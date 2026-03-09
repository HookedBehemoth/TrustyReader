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
pub mod sdspi_fatfs;

use core::cell::RefCell;

use crate::sdspi_fatfs::FatFs;
use alloc::boxed::Box;
use alloc::string::ToString;
use embassy_executor::Spawner;
use embedded_hal_bus::spi::RefCellDevice;
use embedded_io::{Error, ErrorKind};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig, RtcPinWithResistors};
use esp_hal::rtc_cntl::sleep::{RtcioWakeupSource, WakeupLevel};
use esp_hal::rtc_cntl::{Rtc, SocResetReason, reset_reason, wakeup_cause};
use esp_hal::spi::Mode;
use esp_hal::spi::master::{Config, Spi};
use esp_hal::system::Cpu;
use esp_hal::time::Instant;
use log::info;
use trusty_core::container::book::Book;
use trusty_core::fs::{self, Directory, DirEntry, File, Filesystem};

extern crate alloc;
const MAX_BUFFER_SIZE: usize = 512;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[unsafe(no_mangle)]
pub extern "Rust" fn _esp_println_timestamp() -> u64 {
    Instant::now()
        .duration_since_epoch()
        .as_millis()
}

fn log_heap() {
    let stats = esp_alloc::HEAP.stats();
    info!("{stats}");
}

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
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
    let shared_spi: &'static RefCell<_> = Box::leak(Box::new(RefCell::new(spi)));

    info!("SPI initialized");

    let sdcard_cs = Output::new(peripherals.GPIO12, Level::High, OutputConfig::default());
    let sdcard_spi = RefCellDevice::new(shared_spi, sdcard_cs, delay)
        .expect("Failed to create SPI device for SD card");

    let mut sdcard = FatFs::new(sdcard_spi, delay);

    info!("Setup complete! Starting Benchmark...");
    
    parse_all_books(&mut sdcard).unwrap();

    info!("Benchmark done, entering sleep mode.");

    let mut power_pin = peripherals.GPIO3;
    let wakeup_pins: &mut [(&mut dyn RtcPinWithResistors, WakeupLevel)] =
        &mut [(&mut power_pin, WakeupLevel::Low)];

    let rtcio = RtcioWakeupSource::new(wakeup_pins);
    info!("Sleeping");
    delay.delay_millis(100);
    rtc.sleep_deep(&[&rtcio]);
}

pub fn parse_all_books<FS: fs::Filesystem>(filesystem: &mut FS) -> Result<(), ErrorKind> {
    let root = filesystem.open_directory("/").map_err(|e| e.kind())?;
    let entries = root.list().map_err(|e| e.kind())?;
    
    let mut timings = alloc::vec![];
    for entry in entries {
        if entry.is_directory() {
            continue;
        }
        if !entry.name().ends_with(".epub") {
            continue;
        }
        let start = Instant::now();
        let mut file = filesystem.open_file_entry(&root, &entry, fs::Mode::Read).map_err(|e| e.kind())?;
        log::info!("Parsing book from file: {}", entry.name());
        let book = Book::from_file(entry.name(), filesystem, &mut file).unwrap();
        log::info!("Parsed book: {}", book.title());
        for i in 0..book.chapter_count() {
            log::info!("Parsing chapter {} of {}", i + 1, book.chapter_count());
            if let Some(chapter) = book.chapter(i, &mut file) {
                log::info!("Parsed chapter: {:?}", chapter.title);
            } else {
                log::error!("Failed to parse chapter {}", i + 1);
            };
        }
        let duration = Instant::now() - start;
        log::info!("Finished parsing book: {} in {} ms", book.title(), duration.as_millis());
        timings.push((entry.name().to_string(), duration));
    }
    timings.sort_by(|a, b| a.1.cmp(&b.1));
    for (name, duration) in timings {
        let millis: u64 = duration.as_millis();
        log::info!("Book '{}' parsed in {} ms", name, millis);
    }
    Ok(())
}
