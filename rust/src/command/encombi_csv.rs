use chrono::NaiveDate;
use kvstore::KVDb;

use crate::data_mgmt::payload::{Metadata, blank_metadata, payloads_from_device_readings};
use crate::interfaces::kvpath;
use crate::{data_mgmt, node_mgmt, readers};

const DATA_PROVIDER: &str = "encombi-csv";

pub fn read_encombi_csv(date: Option<NaiveDate>) -> anyhow::Result<()> {
    let kvs = KVDb::new(kvpath::SQLITE_STORE.as_path())?;
    let config = node_mgmt::config::get(&kvs)?;
    let readings = readers::encombi_csv::run_acquisition(&config, date);
    log::info!(
        "Finished ENcombi CSV downloads; obtained {} readings",
        readings.len()
    );
    if !readings.is_empty() {
        log::info!("Publishing readings to MQTT");
        let metadata = Some(Metadata {
            data_provider: Some(DATA_PROVIDER.into()),
            ..blank_metadata()
        });
        data_mgmt::publish::publish_readings(&payloads_from_device_readings(readings, metadata))?;
        log::info!("Finished publishing readings to MQTT");
    }
    Ok(())
}
