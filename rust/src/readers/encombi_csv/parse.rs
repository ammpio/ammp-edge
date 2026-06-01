use std::io::{BufRead, BufReader, Cursor};

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
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

const SEPARATOR: char = '\t';
// The ENcombi production log records time-only timestamps; the date is the file's date.
const TIME_FORMAT: &str = "%H:%M:%S";

pub fn parse_csv(
    csv: Cursor<Vec<u8>>,
    driver: &Driver,
    date: NaiveDate,
    timezone: Tz,
) -> Result<Vec<Record>, ParseError> {
    let mut records = vec![];
    for line in BufReader::new(csv).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match parse_line(&line, driver, date, timezone) {
            Ok(rec) => records.push(rec),
            Err(e) => log::warn!("error parsing ENcombi CSV line: {:?}", e),
        }
    }
    Ok(records)
}

fn parse_line(
    line: &str,
    driver: &Driver,
    date: NaiveDate,
    timezone: Tz,
) -> Result<Record, ParseError> {
    let values: Vec<&str> = line.split(SEPARATOR).collect();
    let mut rec = Record::new();

    let time_str = values
        .first()
        .ok_or_else(|| ParseError::FileFormat("timestamp value not present".into()))?;
    rec.set_timestamp(parse_timestamp(time_str, date, timezone)?);

    for field in driver.fields.iter() {
        if let Some(raw) = values.get(field.index)
            && let Ok(parsed) = raw.trim().parse::<f64>()
        {
            let value = match field.multiplier {
                Some(mult) => parsed * mult,
                None => parsed,
            };
            rec.set_field(field.name.to_string(), RtValue::Float(value));
        }
    }
    Ok(rec)
}

fn parse_timestamp(
    time_str: &str,
    date: NaiveDate,
    timezone: Tz,
) -> Result<DateTime<Utc>, ParseError> {
    let time = NaiveTime::parse_from_str(time_str.trim(), TIME_FORMAT)?;
    let naive = NaiveDateTime::new(date, time);
    Ok(naive
        .and_local_timezone(timezone)
        .single()
        .ok_or_else(|| ParseError::FileFormat("ambiguous timestamp".into()))?
        .with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::readers::encombi_csv::driver::ENCOMBI_CSV;

    // Real nighttime sample row (PV/genset off, mains carrying load) from a 2026-05-06_Prod.txt
    const NIGHT_ROW: &str =
        "00:01:16\t0.0\t0.0\t373.5\t373.5\t0.0\t17.5\t0.0\t0.0\t0.0\t0\t1\t0\t1";
    // Daytime sample row (PV producing, no genset)
    const DAY_ROW: &str =
        "12:28:53\t227.1\t0.0\t151.5\t378.5\t350.0\t350.0\t0.0\t0.0\t0.0\t0\t1\t0\t1";

    fn field_f64(rec: &Record, name: &str) -> f64 {
        match rec.get_field(name) {
            Some(RtValue::Float(v)) => *v,
            other => panic!("field {name} missing or not float: {other:?}"),
        }
    }

    #[test]
    fn parse_night_row_fields_and_scaling() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let csv = Cursor::new(NIGHT_ROW.as_bytes().to_vec());
        let records = parse_csv(csv, &ENCOMBI_CSV, date, chrono_tz::Africa::Lagos).unwrap();
        assert_eq!(records.len(), 1);
        let r = &records[0];

        // kW columns scaled to W
        assert_eq!(field_f64(r, "pvinv_P_total"), 0.0);
        assert_eq!(field_f64(r, "genset_P"), 0.0);
        assert_eq!(field_f64(r, "grid_in_P"), 373_500.0);
        assert_eq!(field_f64(r, "grid_out_P"), 373_500.0);
        assert_eq!(field_f64(r, "pv_reference"), 17_500.0);
        // pass-through columns
        assert_eq!(field_f64(r, "irradiance"), 0.0);
        assert_eq!(field_f64(r, "start_active"), 1.0);
        assert_eq!(field_f64(r, "mains_breaker_on"), 1.0);
    }

    #[test]
    fn timestamp_uses_filename_date_and_local_tz() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let csv = Cursor::new(NIGHT_ROW.as_bytes().to_vec());
        let records = parse_csv(csv, &ENCOMBI_CSV, date, chrono_tz::Africa::Lagos).unwrap();
        // 00:01:16 local (Africa/Lagos, UTC+1) on 2026-05-06 -> 2026-05-05T23:01:16Z
        let ts = records[0].get_timestamp().unwrap();
        assert_eq!(ts.to_rfc3339(), "2026-05-05T23:01:16+00:00");
    }

    #[test]
    fn parse_daytime_pv_row() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let csv = Cursor::new(DAY_ROW.as_bytes().to_vec());
        let records = parse_csv(csv, &ENCOMBI_CSV, date, chrono_tz::Africa::Lagos).unwrap();
        assert_eq!(field_f64(&records[0], "pvinv_P_total"), 227_100.0);
        assert_eq!(field_f64(&records[0], "pv_capacity"), 350_000.0);
        assert_eq!(field_f64(&records[0], "grid_in_P"), 151_500.0);
        assert_eq!(field_f64(&records[0], "grid_out_P"), 378_500.0);
    }

    #[test]
    fn negative_mains_keeps_sign() {
        // PV exporting heavily: Mains = -120.5 kW. With the sign-split mechanism removed,
        // the parser preserves the sign so downstream (mqtt-listener / data API) can
        // interpret it as export.
        let row = "12:00:00\t500.0\t0.0\t-120.5\t379.5\t550.0\t550.0\t0.0\t0.0\t0.0\t0\t1\t0\t1";
        let date = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let csv = Cursor::new(row.as_bytes().to_vec());
        let records = parse_csv(csv, &ENCOMBI_CSV, date, chrono_tz::Africa::Lagos).unwrap();
        assert_eq!(records.len(), 1);
        let r = &records[0];
        assert_eq!(field_f64(r, "grid_in_P"), -120_500.0);
        assert_eq!(field_f64(r, "pvinv_P_total"), 500_000.0);
        assert_eq!(field_f64(r, "grid_out_P"), 379_500.0);
    }
}
