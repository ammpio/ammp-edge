use std::time::Duration;

use thiserror::Error;
use zip::result::ZipError;

use crate::data_mgmt::models::{DeviceReading, Record};
use crate::helpers::backoff_retry;
use crate::interfaces::ftp::FtpConnError;
use crate::interfaces::ntp;
use crate::node_mgmt::{config::Device, config::ReadingType, Config};

mod download;
mod driver;
mod parse;
mod timezone;

const READING_TIMEOUT: Option<Duration> = Some(Duration::from_secs(30 * 60));

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
    #[error(transparent)]
    Ntp(#[from] ntp::NtpError),
}

pub fn run_acquisition(config: &Config) -> Vec<DeviceReading> {
    let mut readings = Vec::new();
    let devices_to_read = select_devices_to_read(config);
    log::info!("Reading from {} devices", devices_to_read.len());
    for device in select_devices_to_read(config) {
        read_device(&device, &mut readings).ok();
    }
    readings
}

fn read_device(device: &Device, readings: &mut Vec<DeviceReading>) -> Result<(), SmaHyconCsvError> {
    // TODO: maybe run this multi-threaded? Only relevant if multiple devices, with some failing
    let records = backoff_retry(
        || read_csv_from_device(device).map_err(backoff::Error::transient),
        READING_TIMEOUT,
    );

    match records {
        Ok(records) => {
            log::trace!("readings: {:?}", &records);
            records.into_iter().for_each(|r| {
                readings.push(DeviceReading {
                    device: device.clone(),
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

fn read_csv_from_device(device: &Device) -> Result<Vec<Record>, SmaHyconCsvError> {
    let (filename, data_file) = download::download_last_day_file(device)?;
    let csv_file = if filename.ends_with(download::ZIP_EXT) {
        download::extract_file_from_zip(data_file)?
    } else {
        data_file
    };

    let timezone = timezone::get_timezone(device)?;
    let clock_offset = timezone::try_get_clock_offset(device);
    log::info!(
        "HyCon timezone: {}; clock offset: {}s",
        timezone,
        clock_offset.num_seconds()
    );
    let records = parse::parse_csv(csv_file, &driver::SMA_HYCON_CSV, timezone, clock_offset)?;
    Ok(records)
}

fn select_devices_to_read(config: &Config) -> Vec<Device> {
    config
        .devices
        .iter()
        .map(|(k, d)| Device {
            key: k.into(),
            ..d.clone()
        })
        .filter(|d| d.reading_type == ReadingType::SmaHyconCsv && d.enabled)
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
                    "key": "sma_hycon_csv",
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
                    "key": "sma_stp_1",
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
                "base_url": "ftp://testuser:TestPWD123!@localhost:21/fsc/log/DataFast/",
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
        let zip_file = download::download_last_day_file(&LOCAL_HYCON_DEVICE)
            .unwrap()
            .1;
        let csv_file = download::extract_file_from_zip(zip_file).unwrap();
        assert!(csv_file.into_inner().starts_with(b"Version"))
    }

    #[test]
    fn test_read_and_parse_csv() {
        let records = read_csv_from_device(&LOCAL_HYCON_DEVICE).unwrap();
        assert!(records.len() >= 8640);
    }
}
