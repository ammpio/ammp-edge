use std::collections::HashMap;
use std::io::{BufRead, BufReader, Cursor};

use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;
use thiserror::Error;

use crate::data_mgmt::models::{Record, RtValue};

use super::driver::Driver;

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
    println!("headers: {:#?}", headers);

    let column_map = map_column_to_driver_field(headers, driver);

    for line in lines.skip(LINES_AFTER_HEADER_BEFORE_DATA) {
        let rec = parse_line(&line?, &column_map, timezone)?;
        records.push(rec);
    }

    println!("{:?}", &records[0..5]);

    Ok(records)
}

fn map_column_to_driver_field(headers: Vec<&str>, driver: &Driver) -> HashMap<usize, String> {
    let mut map = HashMap::new();

    for (field, source) in driver.iter() {
        if let Some(index) = headers.iter().position(|&s| s == source.column) {
            map.insert(index, field.clone());
        }
    }
    map
}

fn parse_line(
    line: &str,
    column_map: &HashMap<usize, String>,
    timezone: Tz,
) -> Result<Record, ParseError> {
    let mut rec = Record::new();
    let values: Vec<&str> = line.split(SEPARATOR).collect();
    println!("values: {:#?}", values);

    rec.set_timestamp(parse_timestamp(
        values
            .first()
            .ok_or(ParseError::FileFormat("timestamp value not present".into()))?,
        timezone,
    )?);

    for (col_num, field) in column_map.iter() {
        let value = values.get(*col_num).ok_or(ParseError::FileFormat(format!(
            "cannot read value for {field}"
        )))?;

        // TODO: handle data type according to driver
        if let Ok(value) = value.parse::<f64>() {
            rec.set_field(field.clone(), RtValue::Float(value));
        }
    }

    Ok(rec)
}

fn parse_timestamp(timestamp: &str, timezone: Tz) -> Result<DateTime<Utc>, ParseError> {
    Ok(timezone
        .datetime_from_str(timestamp, TIMESTAMP_FORMAT)?
        .with_timezone(&Utc))
}
