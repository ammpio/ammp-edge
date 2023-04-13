use kvstore::KVDb;

use crate::data_mgmt::payload::{Metadata, BLANK_METADATA};
use crate::interfaces::kvpath;
use crate::{data_mgmt, node_mgmt, readers};

const DATA_PROVIDER: &str = "sma-hycon-csv";

pub fn read_sma_hycon_csv() -> anyhow::Result<()> {
    let kvs = KVDb::new(kvpath::SQLITE_STORE.as_path())?;
    let config = node_mgmt::config::get(kvs)?;
    let readings = readers::sma_hycon_csv::run_acquisition(&config);
    log::info!(
        "Finished SMA Hycon CSV downloads; obtained {} readings",
        readings.len()
    );
    if !readings.is_empty() {
        log::info!("Publishing readings to MQTT");
        let metadata = Some(Metadata { data_provider: Some(DATA_PROVIDER.into()), ..BLANK_METADATA });
        data_mgmt::publish::publish_readings(readings, metadata)?;
        log::info!("Finished publishing readings to MQTT");
    }
    Ok(())
}
