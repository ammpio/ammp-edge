#![allow(unused)]
use std::collections::HashMap;
use std::io::Cursor;

use chrono::offset::Utc;
use chrono::DateTime;
use csv;
use serde_json::Value;
use thiserror::Error;
use zip::result::ZipError;

use crate::interfaces::ftp::FtpConnError;
use crate::node_mgmt::{config::Device, config::ReadingType, Config};

mod download_handler;
mod driver;

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
    Csv(#[from] csv::Error),
    #[error(transparent)]
    Chrono(#[from] chrono::ParseError),
}

type CsvRecord = HashMap<String, Value>;

pub fn run_acquisition(config: &Config) {
    ()
}

fn read_csv_from_device(device: &Device) -> Result<Vec<Record>, SmaHyconCsvError> {
    let zip_file = download_handler::download_last_day_zip(device)?;
    let csv_file = download_handler::extract_file_from_zip(zip_file)?;
    let records = parse_csv(csv_file)?;
    println!("{:?}", records);
    // let csv = str::from_utf8(&csv_file).unwrap();
    Ok(records)
}

#[derive(Debug)]
struct Record {
    timestamp: DateTime<Utc>,
    values: HashMap<String, f64>,
}

fn parse_csv(csv: Cursor<Vec<u8>>) -> Result<Vec<Record>, SmaHyconCsvError> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b';')
        .flexible(true)
        .from_reader(csv);

    let headers = rdr.headers()?.clone();
    let mut records: Vec<Record> = Vec::new();

    for result in rdr.records() {
        let record = result?;
        let mut map = HashMap::new();
        let mut timestamp = None;

        for (i, field) in record.iter().enumerate() {
            if i == 0 {
                timestamp = Some(DateTime::parse_from_rfc3339(field)?.with_timezone(&Utc));
            } else if let Ok(value) = field.parse::<f64>() {
                map.insert(headers[i].to_owned(), value);
            }
        }

        if let Some(timestamp) = timestamp {
            records.push(Record {
                timestamp,
                values: map,
            });
        }
    }

    println!("{:?}", records);

    Ok(records)
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
        let zip_file = download_handler::download_last_day_zip(&LOCAL_HYCON_DEVICE).unwrap();
        let csv_file = download_handler::extract_file_from_zip(zip_file).unwrap();
        assert!(csv_file.into_inner().starts_with(b"Version"))
    }

    #[test]
    fn test_driver() {
        println!("{:?}", driver::SMA_HYCON_CSV["grid_out_P"]);
        // assert!(csv.unwrap().starts_with("Version"))
    }
}