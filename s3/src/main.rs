#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

// pub mod adc_input;
pub mod eink_display;
pub mod sdspi_fatfs;

use core::cell::RefCell;

// use crate::adc_input::*;
use crate::eink_display::EInkDisplay;
use crate::sdspi_fatfs::FatFs;
use alloc::boxed::Box;
use alloc::vec::Vec;
use embassy_executor::Spawner;
use embassy_time::Instant;
use embedded_hal_bus::spi::RefCellDevice;
use esp_backtrace as _;
use esp_hal::Async;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, RtcPin};
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::rtc_cntl::sleep::{RtcioWakeupSource, WakeupLevel};
use esp_hal::rtc_cntl::{Rtc, SocResetReason, reset_reason, wakeup_cause};
use esp_hal::spi::Mode;
use esp_hal::spi::master::{Config, Spi};
use esp_hal::system::Cpu;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::usb_serial_jtag::{UsbSerialJtag, UsbSerialJtagRx};
use log::info;
use trusty_core::application::Application;
use trusty_core::display::{Display, RefreshMode};
use trusty_core::framebuffer::DisplayBuffers;
use trusty_core::{battery, input};

extern crate alloc;
const MAX_BUFFER_SIZE: usize = 512;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[unsafe(no_mangle)]
pub extern "Rust" fn _esp_println_timestamp() -> u64 {
    esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_millis()
}

fn log_heap() {
    let stats = esp_alloc::HEAP.stats();
    info!("{stats}");
}

fn handle_cmd(input_bytes: &[u8]) {
    let Ok(input) = core::str::from_utf8(input_bytes).map(|cmd| cmd.trim()) else {
        return;
    };
    info!("Handling command: {input}");
    let parts = input.split_whitespace();
    let command = parts.into_iter().next().unwrap_or("");
    if command.eq_ignore_ascii_case("ls") {
        /* ... */
    } else if command.eq_ignore_ascii_case("heap") {
        log_heap();
    } else if command.eq_ignore_ascii_case("help") {
        info!("Available commands:");
        info!("  ls   - List files (not implemented)");
        info!("  heap - Show heap usage statistics");
        info!("  help - Show this help message");
    } else {
        info!("Unknown command: {}", command);
    }
}

#[embassy_executor::task]
async fn reader(mut rx: UsbSerialJtagRx<'static, Async>) {
    let mut rbuf = [0u8; MAX_BUFFER_SIZE];
    let mut cmd_buffer: Vec<u8> = Vec::new();
    cmd_buffer.reserve(0x1000);
    loop {
        let r = embedded_io_async::Read::read(&mut rx, &mut rbuf).await;
        match r {
            Ok(len) => {
                cmd_buffer.extend_from_slice(&rbuf[..len]);
                if rbuf.contains(&b'\r') || rbuf.contains(&b'\n') {
                    // Cut input off at first newline
                    let idx = cmd_buffer
                        .iter()
                        .position(|&c| c == b'\r' || c == b'\n')
                        .unwrap();
                    handle_cmd(&cmd_buffer[..idx]);
                    cmd_buffer.clear();
                }
            }
            #[allow(unreachable_patterns)]
            Err(e) => esp_println::println!("RX Error: {:?}", e),
        }
    }
}

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let mut rtc = Rtc::new(peripherals.LPWR);

    info!("up and runnning!");
    let reason = reset_reason(Cpu::ProCpu).unwrap_or(SocResetReason::ChipPowerOn);
    info!("reset reason: {:?}", reason);
    let wake_reason = wakeup_cause();
    info!("wake reason: {:?}", wake_reason);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 0x10000);
    esp_alloc::heap_allocator!(size: 270000);

    let mut flash = esp_storage::FlashStorage::new(peripherals.FLASH);
    let has_ota = verify_ota(&mut flash).is_some();

    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    let (rx, _tx) = UsbSerialJtag::new(peripherals.USB_DEVICE)
        .into_async()
        .split();

    spawner.spawn(reader(rx)).unwrap();

    info!("Heap initialized");
    log_heap();

    let delay = Delay::new();

    let mut display = {
        // Initialize shared SPI bus
        let spi_cfg = Config::default()
            .with_frequency(esp_hal::time::Rate::from_mhz(40))
            .with_mode(Mode::_0);
        let spi = Spi::new(peripherals.SPI2, spi_cfg)
            .expect("Failed to create EPD SPI")
            .with_sck(peripherals.GPIO11)
            .with_mosi(peripherals.GPIO12);
        let shared_spi: &'static RefCell<_> = Box::leak(Box::new(RefCell::new(spi)));

        info!("Setting up GPIO pins");
        let dc = Output::new(peripherals.GPIO9, Level::High, OutputConfig::default());
        let busy = Input::new(peripherals.GPIO3, InputConfig::default());
        let rst = Output::new(peripherals.GPIO46, Level::High, OutputConfig::default());

        info!("Initializing EPD SPI for E-Ink Display");
        let eink_cs = Output::new(peripherals.GPIO10, Level::High, OutputConfig::default());
        let eink_spi_device = RefCellDevice::new(shared_spi, eink_cs, delay)
            .expect("Failed to create SPI device");

        info!("EPD SPI initialized");

        // Create E-Ink Display instance
        info!("Creating E-Ink Display driver");
        EInkDisplay::new(eink_spi_device, dc, rst, busy, delay)
    };

    let mut display_buffers = Box::new(DisplayBuffers::with_rotation(
        trusty_core::framebuffer::Rotation::Rotate90,
    ));

    // Initialize the display
    display.begin().expect("Failed to initialize display");

    info!("Clearing screen");
    display.display(&mut display_buffers, RefreshMode::Full);

    let sdcard = {
        // Initialize shared SPI bus
        let spi_cfg = Config::default()
            .with_frequency(esp_hal::time::Rate::from_mhz(40))
            .with_mode(Mode::_0);
        let spi = Spi::new(peripherals.SPI3, spi_cfg)
            .expect("Failed to create SD SPI")
            .with_sck(peripherals.GPIO16)
            .with_mosi(peripherals.GPIO17)
            .with_miso(peripherals.GPIO15);
        let shared_spi: &'static RefCell<_> = Box::leak(Box::new(RefCell::new(spi)));

        let sdcard_cs = Output::new(peripherals.GPIO18, Level::High, OutputConfig::default());
        let sdcard_spi = RefCellDevice::new(shared_spi, sdcard_cs, delay)
            .expect("Failed to create SPI device for SD card");

        FatFs::new(sdcard_spi, delay)
    };

    info!("Display complete! Starting Application...");
    let mut application = Application::new(&mut display_buffers, sdcard);

    let up = Input::new(peripherals.GPIO4, InputConfig::default());
    let confirm = Input::new(peripherals.GPIO5, InputConfig::default());
    let down = Input::new(peripherals.GPIO6, InputConfig::default());
    let power = Input::new(unsafe { peripherals.GPIO0.clone_unchecked() }, InputConfig::default());

    let mut power_ts: Option<Instant> = None;
    let mut power_long_fired = false;
    let mut confirm_ts: Option<Instant> = None;
    let mut confirm_long_fired = false;

    let mut button_state = input::ButtonState::default();
    while application.running() {
        embassy_time::Timer::after(embassy_time::Duration::from_millis(10)).await;

        let charge = battery::ChargeState::default();

        let mut buttons = 0;
        if up.is_low() {
            buttons |= 1 << input::Buttons::Up as u8
        };
        if down.is_low() {
            buttons |= 1 << input::Buttons::Down as u8
        }
        if power.is_low() {
            if power_ts.is_none() {
                power_ts = Some(embassy_time::Instant::now());
            } else if !power_long_fired && power_ts.map(|ts| ts.elapsed() > embassy_time::Duration::from_millis(500)).unwrap_or(false) {
                power_long_fired = true;
                buttons |= 1 << input::Buttons::Power as u8;
            }
        } else if let Some(ts) = power_ts.take() {
            if !power_long_fired && ts.elapsed() > embassy_time::Duration::from_millis(50) {
                buttons |= 1 << input::Buttons::Back as u8;
            }
            power_long_fired = false;
        }
        if confirm.is_low() {
            if confirm_ts.is_none() {
                confirm_ts = Some(embassy_time::Instant::now());
            } else if !confirm_long_fired && confirm_ts.map(|ts| ts.elapsed() > embassy_time::Duration::from_millis(500)).unwrap_or(false) {
                confirm_long_fired = true;
                buttons |= 1 << input::Buttons::Left as u8;
            }
        } else if let Some(ts) = confirm_ts.take() {
            if !confirm_long_fired && ts.elapsed() > embassy_time::Duration::from_millis(50) {
                buttons |= 1 << input::Buttons::Confirm as u8;
            }
            confirm_long_fired = false;
        }
        button_state.update(buttons);

        application.update(&button_state, charge);
        application.draw(&mut display);
    }

    if has_ota && application.ota_running() {
        info!("OTA requested; switching boot partition");
        switch_ota(&mut flash);
    }

    info!("Application exiting, entering sleep mode.");

    let mut power_pin = peripherals.GPIO0;
    let wakeup_pins: &mut [(&mut dyn RtcPin, WakeupLevel)] =
        &mut [(&mut power_pin, WakeupLevel::Low)];

    let rtcio = RtcioWakeupSource::new(wakeup_pins);
    info!("Sleeping");
    delay.delay_millis(100);
    rtc.sleep_deep(&[&rtcio]);
}

fn verify_ota(storage: &mut esp_storage::FlashStorage) -> Option<()> {
    let mut buffer = Box::new([0u8; esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN]);

    let mut ota =
        esp_bootloader_esp_idf::ota_updater::OtaUpdater::new(storage, &mut buffer).ok()?;

    let current_state = ota.current_ota_state();
    info!("current image state {:?}", current_state);
    info!(
        "currently selected partition {:?}",
        ota.selected_partition()
    );

    match current_state {
        Ok(esp_bootloader_esp_idf::ota::OtaImageState::PendingVerify) => {
            info!("Verifying OTA partition...");
            ota.set_current_ota_state(esp_bootloader_esp_idf::ota::OtaImageState::Valid)
                .unwrap();
        }
        Ok(state) => info!("OTA partition in state {:?}", state),
        Err(e) => info!("OTA partition verification failed: {:?}", e),
    };

    Some(())
}

fn switch_ota(storage: &mut esp_storage::FlashStorage) -> ! {
    let mut buffer = Box::new([0u8; esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN]);

    let mut ota =
        esp_bootloader_esp_idf::ota_updater::OtaUpdater::new(storage, &mut buffer).unwrap();

    info!("current image state {:?}", ota.current_ota_state());
    info!(
        "currently selected partition {:?}",
        ota.selected_partition()
    );

    ota.activate_next_partition().unwrap();
    ota.set_current_ota_state(esp_bootloader_esp_idf::ota::OtaImageState::New)
        .unwrap();

    info!("Restarting device");
    esp_hal::system::software_reset();
}
