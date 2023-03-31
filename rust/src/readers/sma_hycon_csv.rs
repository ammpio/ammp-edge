#![allow(unused)]
use std::io::Cursor;
use std::str;

use thiserror::Error;

use crate::interfaces::ftp::{self, FtpConnError};
use crate::node_mgmt::{config::Device, config::ReadingType, Config};

#[derive(Error, Debug)]
pub enum SmaHyconCsvError {
    #[error(transparent)]
    FtpConn(#[from] FtpConnError),
    #[error("device address error: {0}")]
    Address(String),
    #[error("file error: {0}")]
    File(String),
}

pub fn run_acquisition(config: &Config) {
    ()
}

fn download_last_day_zip(device: &Device) -> Result<Vec<u8>, SmaHyconCsvError> {
    let addr = &device
        .address
        .clone()
        .ok_or(SmaHyconCsvError::Address("missing address".into()))?
        .base_url
        .ok_or(SmaHyconCsvError::Address("missing base URL".into()))?;

    let mut ftp_conn = ftp::FtpConnection::new(addr);
    ftp_conn.connect()?;

    let filename = select_last_day_zip(ftp_conn.list_files()?).ok_or(
        SmaHyconCsvError::File("no zip files found".into()),
    )?;

    Ok(ftp_conn
        .download_file(&filename)
        .unwrap()
        .into_inner())
}

fn select_last_day_zip(filenames: Vec<String>) -> Option<String> {
    filenames
        .iter()
        .filter(|f| f.ends_with(".zip"))
        .max()
        .cloned()
}

fn select_devices_to_read(config: &Config) -> Vec<Device> {
    config
        .devices
        .values()
        .filter(|d| d.reading_type == ReadingType::SmaHyconCsv)
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
                "timezone": "Europe/Amsterdam"
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
        let csv = download_last_day_csv(&LOCAL_HYCON_DEVICE);
        assert!(csv.unwrap().starts_with(b"Version"))
    }

    // #[test]
    // fn test_csv_download() {
    //     let csv = download_latest_csv(&LOCAL_HYCON_DEVICE);
    //     assert!(csv.unwrap().starts_with("Version"))
    // }
}
