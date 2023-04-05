use kvstore::KVDb;

use crate::interfaces::kvpath;
use crate::{node_mgmt, readers};

pub fn read_sma_hycon_csv() -> anyhow::Result<()> {
    let kvs = KVDb::new(kvpath::SQLITE_STORE.as_path())?;
    let config = node_mgmt::config::get(kvs)?;
    let readings = readers::sma_hycon_csv::run_acquisition(&config);
    let num_readings: usize = readings.iter().map(|r| r.records.len()).sum();
    log::info!(
        "Finished SMA Hycon CSV downloads; obtained {} records from {} devices",
        num_readings,
        readings.len()
    );
    Ok(())
}
