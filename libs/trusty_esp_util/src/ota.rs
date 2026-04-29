use esp_storage;
use alloc::boxed::Box;
use log::info;


pub fn verify_ota(storage: &mut esp_storage::FlashStorage) -> Option<()> {
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

pub fn switch_ota(storage: &mut esp_storage::FlashStorage) -> ! {
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
