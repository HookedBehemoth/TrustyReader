use embedded_sdmmc::{RawVolume, SdCard, VolumeManager};
use trusty_core::{fs, io};

/// Dummy time source for embedded-sdmmc (RTC requires too much power)
pub struct DummyTimeSource;

impl embedded_sdmmc::TimeSource for DummyTimeSource {
    fn get_timestamp(&self) -> embedded_sdmmc::Timestamp {
        embedded_sdmmc::Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

pub struct SdSpiFilesystem<SPI, Delay> 
where
    SPI: embedded_hal::spi::SpiDevice<u8>,
    Delay: embedded_hal::delay::DelayNs,
{
    volume_mgr: VolumeManager<SdCard<SPI, Delay>, DummyTimeSource>,
    volume: RawVolume,
}

impl<SPI, Delay> SdSpiFilesystem<SPI, Delay>
where
    SPI: embedded_hal::spi::SpiDevice<u8>,
    Delay: embedded_hal::delay::DelayNs,
{
    pub fn new_with_volume(spi: SPI, delay: Delay) -> fs::Result<Self> {
        let sdcard = SdCard::new(spi, delay);
        let volume_mgr = VolumeManager::new(sdcard, DummyTimeSource);
        let volume = volume_mgr.open_raw_volume(embedded_sdmmc::VolumeIdx(0))
            .map_err(|_| fs::Error::IoFailure)?;
        Ok(SdSpiFilesystem {
            volume_mgr,
            volume,
        })
    }

    fn components(path: &str) -> impl Iterator<Item=&str> {
        path.split('/').filter(|s| !s.is_empty())
    }
}

impl<SPI, Delay> trusty_core::fs::Filesystem<SdSpiFile<'_, SPI, Delay>>
for SdSpiFilesystem<SPI, Delay>
where
    SPI: embedded_hal::spi::SpiDevice<u8>,
    Delay: embedded_hal::delay::DelayNs,
{
    fn create_dir_all(&mut self, path: &str) -> fs::Result<()> {
        let volume = self.volume.to_volume(&self.volume_mgr);
        let mut dir = volume.open_root_dir().map_err(|_| fs::Error::IoFailure)?;

        for comp in Self::components(path) {
            // Ignore error if directory already exists
            let _ = dir.make_dir_in_dir(comp);
            dir.change_dir(comp).map_err(|_| fs::Error::IoFailure)?;
        }

        Ok(())
    }

    fn exists(&mut self, path: &str) -> fs::Result<bool> {
        let volume = self.volume.to_volume(&self.volume_mgr);
        let mut dir = volume.open_root_dir().map_err(|_| fs::Error::IoFailure)?;
        let mut components = Self::components(path).peekable();
        while let Some(comp) = components.next() {
            let entry = match dir.find_directory_entry(comp) {
                Ok(e) => e,
                Err(embedded_sdmmc::Error::NotFound) => return Ok(false),
                Err(_) => return Err(fs::Error::IoFailure),
            };
            if !entry.attributes.is_directory() {
                return Ok(components.peek().is_none());
            }
            if components.peek().is_some() {
                dir.change_dir(entry.name).map_err(|_| fs::Error::IoFailure)?;
            }
        }
        Ok(true)
    }

    fn open(&mut self, path: &str) -> fs::Result<SdSpiFile<'_, SPI, Delay>> {
        let volume = self.volume.to_volume(&self.volume_mgr);
        let mut dir = volume.open_root_dir().map_err(|_| fs::Error::IoFailure)?;
        let mut components = Self::components(path).peekable();
        while let Some(comp) = components.next() {
            let entry = match dir.find_directory_entry(comp) {
                Ok(e) => e,
                Err(embedded_sdmmc::Error::NotFound) => return Err(fs::Error::NotFound),
                Err(_) => return Err(fs::Error::IoFailure),
            };
            if !entry.attributes.is_directory() {
                if components.peek().is_some() {
                    return Err(fs::Error::NotFound);
                }
                let size = entry.size;
                let file = dir.open_file_in_dir(
                    entry.name, embedded_sdmmc::Mode::ReadOnly)
                    .map_err(|_| fs::Error::IoFailure)?;
                return Ok(SdSpiFile {
                    file,
                    size,
                });
            }
            if components.peek().is_some() {
                dir.change_dir(entry.name).map_err(|_| fs::Error::IoFailure)?;
            }
        }
        Err(fs::Error::NotFound)
    }
}

struct SdSpiFile<'a, SPI, Delay>
where
    SPI: embedded_hal::spi::SpiDevice<u8>,
    Delay: embedded_hal::delay::DelayNs,
{
    file: embedded_sdmmc::File<'a, SdCard<SPI, Delay>, DummyTimeSource, 4, 4, 1>,
    size: u32,
}

impl<SPI, Delay> io::Stream
for SdSpiFile<'_, SPI, Delay>
where
    SPI: embedded_hal::spi::SpiDevice<u8>,
    Delay: embedded_hal::delay::DelayNs,
{
    fn size(&self) -> usize {
        self.size as usize
    }

    fn seek(&mut self, pos: usize) -> core::result::Result<(), ()> {
        self.file
            .seek_from_start(pos as u32)
            .map_err(|_| ())
    }
}

impl<SPI, Delay> io::Read
for SdSpiFile<'_, SPI, Delay>
where
    SPI: embedded_hal::spi::SpiDevice<u8>,
    Delay: embedded_hal::delay::DelayNs,
{
    fn read(&mut self, buf: &mut [u8]) -> core::result::Result<usize, ()> {
        self.file.read(buf).map_err(|_| ())
    }
}
