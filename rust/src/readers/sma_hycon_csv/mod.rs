use chrono_tz::Tz;
use thiserror::Error;
use zip::result::ZipError;

use crate::data_mgmt::models::{DeviceReading, Record};
use crate::interfaces::ftp::FtpConnError;
use crate::node_mgmt::{config::Device, config::ReadingType, Config};

mod download;
mod driver;
mod parse;

#[derive(Error, Debug)]
pub enum SmaHyconCsvError {
    #[error(transparent)]
    FtpConn(#[from] FtpConnError),
    #[error("device address error: {0}")]
    Address(String),
    #[error(transparent)]
    Zip(#[from] ZipError),
    #[error("file error: {0}")]
    File(String),
    #[error(transparent)]
    Parse(#[from] parse::ParseError),
}

pub fn run_acquisition(config: &Config) -> Vec<DeviceReading> {
    let mut readings = Vec::new();
    let devices_to_read = select_devices_to_read(config);
    log::info!("Reading from {} devices", devices_to_read.len());
    for device in select_devices_to_read(config) {
        match read_csv_from_device(&device) {
            Ok(records) => {
                log::trace!("Readings: {:#?}", &records);
                records.into_iter().for_each(|r| {
                    readings.push(DeviceReading {
                        device: device.clone(),
                        record: r,
                    })
                });
            }
            Err(e) => {
                log::error!("Error reading CSV from device {:?}: {:#?}", device, e);
            }
        }
    }
    readings
}

fn read_csv_from_device(device: &Device) -> Result<Vec<Record>, SmaHyconCsvError> {
    let zip_file = download::download_last_day_zip(device)?;
    let csv_file = download::extract_file_from_zip(zip_file)?;

    let timezone_str = device
        .address
        .as_ref()
        .ok_or(SmaHyconCsvError::Address("missing device address".into()))?
        .timezone
        .as_ref()
        .ok_or(SmaHyconCsvError::Address("missing timezone".into()))?;
    let timezone = timezone_str
        .parse::<Tz>()
        .map_err(|e| SmaHyconCsvError::Address(format!("invalid timezone: {e}")))?;
    let records = parse::parse_csv(csv_file, &driver::SMA_HYCON_CSV, timezone)?;
    Ok(records)
}

fn select_devices_to_read(config: &Config) -> Vec<Device> {
    config
        .devices
        .values()
        .filter(|d| d.reading_type == ReadingType::SmaHyconCsv && d.enabled)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use once_cell::sync::Lazy;
    use std::str::FromStr;

    use crate::node_mgmt::config::{Config, Device};

    static SAMPLE_CONFIG_WITH_HYCON_CSV: Lazy<Config> = Lazy::new(|| {
        Config::from_str(
            r#"
        {
            "devices": {
                "sma_hycon_csv": {
                    "driver": "sma_hycon_csv",
                    "address": {
                        "base_url": "ftp://User:pwd@172.16.1.21:900/fsc/log/DataFast/"
                    },
                    "enabled": true,
                    "vendor_id": "sma-hycon-1",
                    "device_model": "gen_control_sma_hycon",
                    "reading_type": "sma_hycon_csv"
                }
            },
            "readings": {},
            "timestamp": "1970-01-01T00:00:00Z"
        }
        "#,
        )
        .unwrap()
    });

    static SAMPLE_CONFIG_NO_HYCON_CSV: Lazy<Config> = Lazy::new(|| {
        Config::from_str(
            r#"
        {
            "devices": {
                "sma_stp_1": {
                    "name": "SMA STP-25000",
                    "driver": "sma_stp25000",
                    "enabled": true,
                    "device_model": "pv_inv_sma",
                    "vendor_id": "1234567890",
                    "reading_type": "modbustcp",
                    "address": {
                        "host": "mock-sma-stp",
                        "unit_id": 3
                    }
                }
            },
            "readings": {},
            "timestamp": "1970-01-01T00:00:00Z"
        }
        "#,
        )
        .unwrap()
    });

    static LOCAL_HYCON_DEVICE: Lazy<Device> = Lazy::new(|| {
        serde_json::from_str::<Device>(
            r#"
        {
            "driver": "sma_hycon_csv",
            "address": {
                "base_url": "ftp://testuser:testpwd@localhost:21/fsc/log/DataFast/",
                "timezone": "Africa/Johannesburg"
            },
            "enabled": true,
            "vendor_id": "sma-hycon-1",
            "device_model": "gen_control_sma_hycon",
            "reading_type": "sma_hycon_csv"
        }
        "#,
        )
        .unwrap()
    });

    #[test]
    fn check_selected_devices() {
        assert!(select_devices_to_read(&SAMPLE_CONFIG_NO_HYCON_CSV).is_empty());
        assert_eq!(
            select_devices_to_read(&SAMPLE_CONFIG_WITH_HYCON_CSV)[0],
            SAMPLE_CONFIG_WITH_HYCON_CSV.devices["sma_hycon_csv"]
        );
    }

    #[test]
    fn test_csv_download() {
        let zip_file = download::download_last_day_zip(&LOCAL_HYCON_DEVICE).unwrap();
        let csv_file = download::extract_file_from_zip(zip_file).unwrap();
        assert!(csv_file.into_inner().starts_with(b"Version"))
    }

    #[test]
    fn test_read_and_parse_csv() {
        let records = read_csv_from_device(&LOCAL_HYCON_DEVICE).unwrap();
        assert!(records.len() >= 8640);
    }
}
