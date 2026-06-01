use chrono_tz::Tz;

use crate::node_mgmt::config::Device;

use super::EncombiCsvError;

pub fn get_timezone(device: &Device) -> Result<Tz, EncombiCsvError> {
    device
        .address
        .as_ref()
        .ok_or_else(|| EncombiCsvError::Address("missing device address".into()))?
        .timezone
        .as_ref()
        .ok_or_else(|| EncombiCsvError::Address("missing timezone".into()))?
        .parse::<Tz>()
        .map_err(|e| EncombiCsvError::Address(format!("invalid timezone: {e}")))
}
