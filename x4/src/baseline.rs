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
use embedded_io::{Error, ErrorKind, Read};
use embedded_xml as xml;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::rtc_cntl::{SocResetReason, reset_reason, wakeup_cause};
use esp_hal::spi::Mode;
use esp_hal::spi::master::{Config, Spi};
use esp_hal::system::Cpu;
use esp_hal::time::Instant;
use log::info;
use trusty_core::container::image;
use trusty_core::fs::{self, Directory, DirEntry};
use embedded_zip as zip;

extern crate alloc;

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

    log::warn!("Benchmark done, spinning.");

    loop {
        delay.delay_millis(1000u32);
    }
}

pub fn parse_all_books<FS: fs::Filesystem>(filesystem: &mut FS) -> Result<(), ErrorKind> {
    let root = filesystem.open_directory("/").map_err(|e| e.kind())?;
    let dir_entries = root.list().map_err(|e| e.kind())?;

    let mut read_timings = alloc::vec![];
    let mut zip_timings = alloc::vec![];
    let mut xml_timings = alloc::vec![];
    let mut image_timings = alloc::vec![];

    for dir_entry in dir_entries {
        if dir_entry.is_directory() {
            continue;
        }
        if !dir_entry.name().ends_with("ohler.epub") {
            continue;
        }

        let mut file = filesystem
            .open_file_entry(&root, &dir_entry, fs::Mode::Read)
            .map_err(|e| e.kind())?;
        log::info!("Parsing book from file: {}", dir_entry.name());

        // --- 1. Raw file read (baseline throughput) ---
        let mut buf = [0u8; 4096];
        let start = Instant::now();
        while let Ok(n) = file.read(&mut buf) {
            if n == 0 {
                break;
            }
        }
        let duration = Instant::now() - start;
        let mbps = (dir_entry.size() as f64) / (duration.as_micros() as f64);
        log::warn!(
            "[read] {} in {} ms ({:.2} MB/s)",
            dir_entry.name(),
            duration.as_millis(),
            mbps
        );
        read_timings.push((dir_entry.name().to_string(), duration, mbps));

        // --- 2. ZIP central directory parsing ---
        let start = Instant::now();
        let entries = embedded_zip::parse_zip(&mut file).map_err(|_| ErrorKind::Other)?;
        let duration = Instant::now() - start;
        log::warn!(
            "[zip] {} central directory ({} entries) in {} ms",
            dir_entry.name(),
            entries.len(),
            duration.as_millis()
        );
        zip_timings.push((dir_entry.name().to_string(), duration));

        log_heap();

        // --- 3. XML reading overhead (XHTML entries) ---
        for entry in entries.iter() {
            let is_xhtml = entry.name.ends_with(".xhtml") || entry.name.ends_with(".html") || entry.name.ends_with(".xml");
            if !is_xhtml {
                continue;
            }

            let start = Instant::now();
            let reader = zip::ZipEntryReader::new(&mut file, entry).map_err(|_| ErrorKind::Other)?;
            let Ok(mut parser) = xml::Reader::new(reader, entry.size as _, 4096) else {
                log::error!("Failed to create XML reader for {}", entry.name);
                continue;
            };
            let mut event_count: u32 = 0;
            loop {
                match parser.next_event() {
                    Ok(xml::Event::EndOfFile) => break,
                    Ok(_) => event_count += 1,
                    Err(_) => {
                        log::error!("XML parse error in {}", entry.name);
                        break;
                    }
                }
            }
            let duration = Instant::now() - start;
            log::warn!(
                "[xml] {} ({} bytes, {} events) in {} ms",
                entry.name,
                entry.size,
                event_count,
                duration.as_millis()
            );
            xml_timings.push((entry.name.clone(), entry.size, event_count, duration));
        }

        // --- 4. Image parsing overhead ---
        for entry in &entries {
            let Some(format) = image::Format::guess_from_filename(&entry.name) else {
                continue;
            };

            // Landscape
            let start = Instant::now();
            let mut reader = zip::ZipEntryReader::new(&mut file, entry).map_err(|_| ErrorKind::Other)?;
            match image::decode(format, &mut reader, entry.size, 800, 480) {
                Ok(img) => {
                    let duration = Instant::now() - start;
                    log::warn!(
                        "[img] {} {:?} {}x{} (landscape) in {} ms",
                        entry.name,
                        format,
                        img.width,
                        img.height,
                        duration.as_millis()
                    );
                    image_timings.push((entry.name.clone(), format, (img.width, img.height), duration));
                }
                Err(e) => {
                    log::error!("Failed to decode image '{}': {}", entry.name, e);
                }
            }

            // Portrait
            let start = Instant::now();
            let mut reader = zip::ZipEntryReader::new(&mut file, entry).map_err(|_| ErrorKind::Other)?;
            match image::decode(format, &mut reader, entry.size, 480, 800) {
                Ok(img) => {
                    let duration = Instant::now() - start;
                    log::warn!(
                        "[img] {} {:?} {}x{} (portrait) in {} ms",
                        entry.name,
                        format,
                        img.width,
                        img.height,
                        duration.as_millis()
                    );
                    image_timings.push((entry.name.clone(), format, (img.width, img.height), duration));
                }
                Err(e) => {
                    log::error!("Failed to decode image '{}' (portrait): {}", entry.name, e);
                }
            }
        }
    }

    // --- Summary ---
    log::warn!("=== SUMMARY ===");

    read_timings.sort_by(|a, b| a.1.cmp(&b.1));
    for (name, duration, mbps) in &read_timings {
        log::warn!("[read] '{}' in {} ms ({:.2} MB/s)", name, duration.as_millis(), mbps);
    }

    zip_timings.sort_by(|a, b| a.1.cmp(&b.1));
    for (name, duration) in &zip_timings {
        log::warn!("[zip]  '{}' in {} ms", name, duration.as_millis());
    }

    xml_timings.sort_by(|a, b| a.3.cmp(&b.3));
    for (name, size, events, duration) in &xml_timings {
        log::warn!(
            "[xml]  '{}' ({} bytes, {} events) in {} ms",
            name,
            size,
            events,
            duration.as_millis()
        );
    }

    image_timings.sort_by(|a, b| a.3.cmp(&b.3));
    for (name, format, (w, h), duration) in &image_timings {
        log::warn!(
            "[img]  '{}' {:?} {}x{} in {} ms",
            name,
            format,
            w,
            h,
            duration.as_millis()
        );
    }

    Ok(())
}
