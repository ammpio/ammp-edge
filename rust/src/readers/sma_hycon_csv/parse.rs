use std::collections::HashMap;
use std::io::{BufRead, BufReader, Cursor};

use chrono::{DateTime, TimeZone, Utc, Duration};
use chrono_tz::Tz;
use thiserror::Error;

use crate::data_mgmt::models::{Record, RtValue};

use super::driver::{Driver, DriverField};

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("file format error: {0}")]
    FileFormat(String),
    #[error(transparent)]
    FileRead(#[from] std::io::Error),
    #[error(transparent)]
    Chrono(#[from] chrono::ParseError),
}

const LINES_BEFORE_HEADERS: usize = 5;
const LINES_AFTER_HEADER_BEFORE_DATA: usize = 2;
const SEPARATOR: &str = ";";
const TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

pub fn parse_csv(
    csv: Cursor<Vec<u8>>,
    driver: &Driver,
    timezone: Tz,
    clock_offset: Duration,
) -> Result<Vec<Record>, ParseError> {
    let mut records = vec![];
    let mut reader = BufReader::new(csv).lines();
    let lines = reader.by_ref();

    let headers_line = lines
        .nth(LINES_BEFORE_HEADERS)
        .ok_or(ParseError::FileFormat(
            "cannot read header line; insufficient fize size".into(),
        ))??;

    let headers = headers_line.split(SEPARATOR).collect();
    let column_map = map_column_to_driver_field(headers, driver);

    for line in lines.skip(LINES_AFTER_HEADER_BEFORE_DATA) {
        match parse_line(&line?, &column_map, timezone, clock_offset) {
            Ok(rec) => records.push(rec),
            Err(e) => log::warn!("error parsing CSV line: {:?}", e),
        }
    }

    Ok(records)
}

fn map_column_to_driver_field(headers: Vec<&str>, driver: &Driver) -> HashMap<usize, DriverField> {
    let mut map = HashMap::new();

    for field in driver.fields.iter() {
        if let Some(index) = headers.iter().position(|&s| s == field.column) {
            map.insert(index, field.clone());
        }
    }
    map
}

fn parse_line(
    line: &str,
    column_map: &HashMap<usize, DriverField>,
    timezone: Tz,
    clock_offset: Duration,
) -> Result<Record, ParseError> {
    let mut rec = Record::new();
    let values: Vec<&str> = line.split(SEPARATOR).collect();

    rec.set_timestamp(parse_timestamp(
        values
            .first()
            .ok_or(ParseError::FileFormat("timestamp value not present".into()))?,
        timezone,
        clock_offset,
    )?);

    for (col_num, field) in column_map.iter() {
        let value = values.get(*col_num).ok_or(ParseError::FileFormat(format!(
            "cannot read value for {}", field.name
        )))?;

        // TODO: handle non-float data type according to driver
        // Also break this functionality out into dedicated module
        if let Ok(value) = value.parse::<f64>() {
            let ret_value = if let Some(mult) = field.multiplier {
                RtValue::Float(value * mult)
            } else {
                RtValue::Float(value)
            };
            rec.set_field(field.name.clone(), ret_value);
        }
    }

    Ok(rec)
}

fn parse_timestamp(timestamp: &str, timezone: Tz, clock_offset: Duration) -> Result<DateTime<Utc>, ParseError> {
    Ok(timezone
        .datetime_from_str(timestamp, TIMESTAMP_FORMAT)?
        .with_timezone(&Utc) - clock_offset
    )
}
