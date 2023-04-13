use kvstore::KVDb;

use crate::interfaces::kvpath;
use crate::{data_mgmt, node_mgmt, readers};

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
        data_mgmt::publish::publish_readings(readings)?;
        log::info!("Finished publishing readings to MQTT");
    }
    Ok(())
}
