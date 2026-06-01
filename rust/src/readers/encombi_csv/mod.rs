use std::time::Duration;

use chrono::{NaiveDate, Utc};
use chrono_tz::Tz;
use thiserror::Error;

use crate::data_mgmt::models::{DeviceReading, DeviceRef, Record};
use crate::helpers::backoff_retry;
use crate::interfaces::ftp::FtpConnError;
use crate::node_mgmt::config::{Config, Device, ReadingType};

mod download;
mod driver;
mod parse;
mod timezone;

const READING_TIMEOUT: Option<Duration> = Some(Duration::from_secs(30 * 60));

#[derive(Error, Debug)]
pub enum EncombiCsvError {
    #[error(transparent)]
    FtpConn(#[from] FtpConnError),
    #[error("device address error: {0}")]
    Address(String),
    #[error(transparent)]
    Parse(#[from] parse::ParseError),
}

pub fn run_acquisition(config: &Config, target_date: Option<NaiveDate>) -> Vec<DeviceReading> {
    let mut readings = Vec::new();
    let devices_to_read = select_devices_to_read(config);
    log::info!("Reading from {} ENcombi CSV devices", devices_to_read.len());
    for device in devices_to_read {
        read_device(&device, target_date, &mut readings).ok();
    }
    readings
}

fn read_device(
    device: &Device,
    target_date: Option<NaiveDate>,
    readings: &mut Vec<DeviceReading>,
) -> Result<(), EncombiCsvError> {
    let timezone = timezone::get_timezone(device)?;
    let date = target_date.unwrap_or_else(|| yesterday_in_tz(timezone));

    let records = backoff_retry(
        || read_csv_from_device(device, date, timezone).map_err(backoff::Error::transient),
        READING_TIMEOUT,
    );

    match records {
        Ok(records) => {
            log::trace!("readings: {:?}", &records);
            records.into_iter().for_each(|r| {
                readings.push(DeviceReading {
                    device: DeviceRef::from_device(device),
                    record: r,
                })
            });
        }
        Err(e) => {
            log::error!("error reading CSV from device {:?}: {}", device, e);
        }
    }
    Ok(())
}

fn read_csv_from_device(
    device: &Device,
    date: NaiveDate,
    timezone: Tz,
) -> Result<Vec<Record>, EncombiCsvError> {
    let data_file = download::download_file_for_date(device, date)?;
    log::info!(
        "ENcombi {} timezone: {}",
        download::filename_for_date(date),
        timezone
    );
    let records = parse::parse_csv(data_file, &driver::ENCOMBI_CSV, date, timezone)?;
    Ok(records)
}

// Yesterday in the device's local timezone. The system clock (NTP-synced via the
// `ae-wait-for-time-source` snap dependency) is trusted; the ENcombi controller's
// own clock is not.
fn yesterday_in_tz(tz: Tz) -> NaiveDate {
    (Utc::now().with_timezone(&tz) - chrono::Duration::days(1)).date_naive()
}

fn select_devices_to_read(config: &Config) -> Vec<Device> {
    config
        .devices
        .iter()
        .map(|(k, d)| Device {
            key: k.into(),
            ..d.clone()
        })
        .filter(|d| d.reading_type == ReadingType::EncombiCsv && d.enabled)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use once_cell::sync::Lazy;

    use crate::node_mgmt::config::{Config, config_from_str};

    static CONFIG_WITH_ENCOMBI_CSV: Lazy<Config> = Lazy::new(|| {
        config_from_str(
            r#"
        {
            "devices": {
                "encombi_csv": {
                    "key": "encombi_csv",
                    "name": "ENcombi Controller - CSV backfill",
                    "driver": "encombi_csv",
                    "address": {
                        "base_url": "ftp://User:pwd@172.16.1.101:21/",
                        "timezone": "Africa/Lagos"
                    },
                    "enabled": true,
                    "vendor_id": "encombi-1",
                    "device_model": "gen_control_encombi",
                    "reading_type": "encombi_csv"
                }
            },
            "readings": {},
            "timestamp": "1970-01-01T00:00:00Z"
        }
        "#,
        )
        .unwrap()
    });

    static CONFIG_WITHOUT_ENCOMBI_CSV: Lazy<Config> = Lazy::new(|| {
        config_from_str(
            r#"
        {
            "devices": {
                "sma_stp_1": {
                    "key": "sma_stp_1",
                    "driver": "sma_stp25000",
                    "enabled": true,
                    "device_model": "pv_inv_sma",
                    "vendor_id": "1234567890",
                    "reading_type": "modbustcp",
                    "address": { "host": "mock-sma-stp", "unit_id": 3 }
                }
            },
            "readings": {},
            "timestamp": "1970-01-01T00:00:00Z"
        }
        "#,
        )
        .unwrap()
    });

    #[test]
    fn selects_only_encombi_csv_devices() {
        assert!(select_devices_to_read(&CONFIG_WITHOUT_ENCOMBI_CSV).is_empty());
        let selected = select_devices_to_read(&CONFIG_WITH_ENCOMBI_CSV);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].reading_type, ReadingType::EncombiCsv);
    }
}
