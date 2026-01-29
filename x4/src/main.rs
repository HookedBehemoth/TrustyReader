#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

pub mod adc_input;
pub mod eink_display;
pub mod sdspi_fs;

use core::cell::RefCell;

use crate::adc_input::*;
use crate::eink_display::EInkDisplay;
use crate::sdspi_fs::SdSpiFilesystem;
use alloc::boxed::Box;
use alloc::vec::Vec;
use embassy_executor::Spawner;
use embedded_hal_bus::spi::RefCellDevice;
use esp_backtrace as _;
use esp_hal::Async;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, RtcPinWithResistors};
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

extern crate alloc;
const MAX_BUFFER_SIZE: usize = 512;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

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

    let mut flash = esp_storage::FlashStorage::new(peripherals.FLASH);
    verify_ota(&mut flash);

    let mut rtc = Rtc::new(peripherals.LPWR);

    info!("up and runnning!");
    let reason = reset_reason(Cpu::ProCpu).unwrap_or(SocResetReason::ChipPowerOn);
    info!("reset reason: {:?}", reason);
    let wake_reason = wakeup_cause();
    info!("wake reason: {:?}", wake_reason);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 0x10000);
    esp_alloc::heap_allocator!(size: 290000);

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

    // Initialize shared SPI bus
    let spi_cfg = Config::default()
        .with_frequency(esp_hal::time::Rate::from_mhz(40))
        .with_mode(Mode::_0);
    let spi = Spi::new(peripherals.SPI2, spi_cfg)
        .expect("Failed to create SPI")
        .with_sck(peripherals.GPIO8)
        .with_mosi(peripherals.GPIO10)
        .with_miso(peripherals.GPIO7);
    let shared_spi = RefCell::new(spi);

    info!("Setting up GPIO pins");
    let dc = Output::new(peripherals.GPIO4, Level::High, OutputConfig::default());
    let busy = Input::new(peripherals.GPIO6, InputConfig::default());
    let rst = Output::new(peripherals.GPIO5, Level::High, OutputConfig::default());

    info!("Initializing SPI for E-Ink Display");
    let eink_cs = Output::new(peripherals.GPIO21, Level::High, OutputConfig::default());
    let eink_spi_device = RefCellDevice::new(&shared_spi, eink_cs, delay.clone())
        .expect("Failed to create SPI device");

    info!("SPI initialized");

    let mut display_buffers = Box::new(DisplayBuffers::default());

    // Create E-Ink Display instance
    info!("Creating E-Ink Display driver");
    let mut display = EInkDisplay::new(eink_spi_device, dc, rst, busy, delay);

    // Initialize the display
    display.begin().expect("Failed to initialize display");

    info!("Clearing screen");
    display.display(&mut display_buffers, RefreshMode::Full);

    let mut button_state = GpioButtonState::new(
        peripherals.GPIO1,
        peripherals.GPIO2,
        unsafe { peripherals.GPIO3.clone_unchecked() },
        peripherals.ADC1,
    );

    let sdcard_cs = Output::new(peripherals.GPIO12, Level::High, OutputConfig::default());
    let sdcard_spi = RefCellDevice::new(&shared_spi, sdcard_cs, delay)
        .expect("Failed to create SPI device for SD card");

    let sdcard = SdSpiFilesystem::new_with_volume(sdcard_spi, delay)
        .expect("Failed to create SD SPI filesystem");

    info!("Display complete! Starting rotation demo...");
    let mut application = Application::new(&mut display_buffers, sdcard);

    while application.running() {
        embassy_time::Timer::after(embassy_time::Duration::from_millis(10)).await;

        button_state.update();
        let buttons = button_state.get_buttons();
        application.update(&buttons);
        application.draw(&mut display);
    }

    if application.ota_running()
    {
        info!("OTA requested; switching boot partition");
        switch_ota(&mut flash);
    }

    info!("Application exiting, entering sleep mode.");

    let mut power_pin = peripherals.GPIO3;
    let wakeup_pins: &mut [(&mut dyn RtcPinWithResistors, WakeupLevel)] =
        &mut [(&mut power_pin, WakeupLevel::Low)];

    let rtcio = RtcioWakeupSource::new(wakeup_pins);
    info!("Sleeping");
    delay.delay_millis(100);
    rtc.sleep_deep(&[&rtcio]);
}

fn verify_ota(storage: &mut esp_storage::FlashStorage) {
    let mut buffer = [0u8; esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN];

    let mut ota =
        esp_bootloader_esp_idf::ota_updater::OtaUpdater::new(storage, &mut buffer).unwrap();

    let current_state = ota.current_ota_state();
    info!("current image state {:?}", current_state);
    info!("currently selected partition {:?}", ota.selected_partition());

    match current_state {
        Ok(esp_bootloader_esp_idf::ota::OtaImageState::PendingVerify) => {
            info!("Verifying OTA partition...");
            ota.set_current_ota_state(esp_bootloader_esp_idf::ota::OtaImageState::Valid)
                .unwrap();
        },
        Ok(state) => info!("OTA partition in state {:?}", state),
        Err(e) => info!("OTA partition verification failed: {:?}", e),
    }
}

fn switch_ota(storage: &mut esp_storage::FlashStorage) -> ! {
    let mut buffer = [0u8; esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN];

    let mut ota =
        esp_bootloader_esp_idf::ota_updater::OtaUpdater::new(storage, &mut buffer).unwrap();

    info!("current image state {:?}", ota.current_ota_state());
    info!("currently selected partition {:?}", ota.selected_partition());

    ota.activate_next_partition().unwrap();
    ota.set_current_ota_state(esp_bootloader_esp_idf::ota::OtaImageState::New)
        .unwrap();

    info!("Restarting device");
    esp_hal::system::software_reset();
}
