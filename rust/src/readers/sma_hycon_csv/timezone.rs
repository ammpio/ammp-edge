use chrono::Duration;
use chrono_tz::Tz;

use crate::{node_mgmt::config::Device, interfaces::ntp};

use super::{SmaHyconCsvError, download::get_base_url};

pub fn get_timezone(device: &Device) -> Result<chrono_tz::Tz, SmaHyconCsvError> {
    device
        .address
        .as_ref()
        .ok_or(SmaHyconCsvError::Address("missing device address".into()))?
        .timezone
        .as_ref()
        .ok_or(SmaHyconCsvError::Address("missing timezone".into()))?
        .parse::<Tz>()
        .map_err(|e| SmaHyconCsvError::Address(format!("invalid timezone: {e}")))
}


pub fn get_clock_offset(device: &Device) -> Result<Duration, SmaHyconCsvError> {
    let url = url::Url::parse(&get_base_url(device)?).map_err(|e| SmaHyconCsvError::Address(format!("invalid URL: {e}")))?;
    let hostname = url.host().ok_or(SmaHyconCsvError::Address("missing hostname".into()))?.to_string();
    Ok(ntp::query_offset_wrt_systime(&hostname, None)?)
}
