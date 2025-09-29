use std::time::Duration as StdDuration;

use chrono::Duration;
use chrono_tz::Tz;

use crate::helpers::backoff_retry;
use crate::interfaces::ntp;
use crate::node_mgmt::config::Device;

use super::{SmaHyconCsvError, download::get_base_url};

const NTP_RETRY_TIMEOUT: Option<StdDuration> = Some(StdDuration::from_secs(15));

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
    let url = url::Url::parse(&get_base_url(device)?)
        .map_err(|e| SmaHyconCsvError::Address(format!("invalid URL: {e}")))?;
    let hostname = url
        .host()
        .ok_or(SmaHyconCsvError::Address("missing hostname".into()))?
        .to_string();
    Ok(ntp::query_offset_wrt_systime(&hostname, None)?)
}

pub fn try_get_clock_offset(device: &Device) -> Duration {
    backoff_retry(
        || get_clock_offset(device).map_err(backoff::Error::transient),
        NTP_RETRY_TIMEOUT,
    )
    .unwrap_or_else(|err| {
        log::warn!(
            "error '{}'\nwhile getting NTP clock offset for device {:?}\nassuming zero offset",
            err,
            device
        );
        Duration::zero()
    })
}
