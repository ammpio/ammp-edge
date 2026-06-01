use std::io::{BufRead, BufReader, Cursor};

use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc};
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
// The ENcombi production log records time-only timestamps; the date is taken from the filename.
const TIME_FORMAT: &str = "%H:%M:%S";

pub fn parse_csv(
    csv: Cursor<Vec<u8>>,
    driver: &Driver,
    date: NaiveDate,
    timezone: Tz,
    clock_offset: Duration,
) -> Result<Vec<Record>, ParseError> {
    let mut records = vec![];
    for line in BufReader::new(csv).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match parse_line(&line, driver, date, timezone, clock_offset) {
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
    clock_offset: Duration,
) -> Result<Record, ParseError> {
    let values: Vec<&str> = line.split(SEPARATOR).collect();
    let mut rec = Record::new();

    let time_str = values
        .first()
        .ok_or_else(|| ParseError::FileFormat("timestamp value not present".into()))?;
    rec.set_timestamp(parse_timestamp(time_str, date, timezone, clock_offset)?);

    for field in driver.fields.iter() {
        if let Some(raw) = values.get(field.index)
            && let Ok(parsed) = raw.trim().parse::<f64>()
        {
            // Sign-split: a negative value on a column with `negative_name`
            // (e.g. Mains -> grid_out_P when exporting) is absolutised and
            // emitted under the alternate field name.
            let (out_name, signed) = match field.negative_name {
                Some(neg) if parsed < 0.0 => (neg, -parsed),
                _ => (field.name, parsed),
            };
            let final_value = match field.multiplier {
                Some(mult) => signed * mult,
                None => signed,
            };
            rec.set_field(out_name.to_string(), RtValue::Float(final_value));
        }
    }
    Ok(rec)
}

fn parse_timestamp(
    time_str: &str,
    date: NaiveDate,
    timezone: Tz,
    clock_offset: Duration,
) -> Result<DateTime<Utc>, ParseError> {
    let time = NaiveTime::parse_from_str(time_str.trim(), TIME_FORMAT)?;
    let naive = NaiveDateTime::new(date, time);
    Ok(naive
        .and_local_timezone(timezone)
        .single()
        .ok_or_else(|| ParseError::FileFormat("ambiguous timestamp".into()))?
        .with_timezone(&Utc)
        - clock_offset)
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
        let records = parse_csv(
            csv,
            &ENCOMBI_CSV,
            date,
            chrono_tz::Africa::Lagos,
            Duration::zero(),
        )
        .unwrap();
        assert_eq!(records.len(), 1);
        let r = &records[0];

        // kW columns scaled to W
        assert_eq!(field_f64(r, "pvinv_P_total"), 0.0);
        assert_eq!(field_f64(r, "genset_P"), 0.0);
        assert_eq!(field_f64(r, "grid_in_P"), 373_500.0);
        assert_eq!(field_f64(r, "load_P"), 373_500.0);
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
        let records = parse_csv(
            csv,
            &ENCOMBI_CSV,
            date,
            chrono_tz::Africa::Lagos,
            Duration::zero(),
        )
        .unwrap();
        // 00:01:16 local (Africa/Lagos, UTC+1) on 2026-05-06 -> 2026-05-05T23:01:16Z
        let ts = records[0].get_timestamp().unwrap();
        assert_eq!(ts.to_rfc3339(), "2026-05-05T23:01:16+00:00");
    }

    #[test]
    fn parse_daytime_pv_row() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let csv = Cursor::new(DAY_ROW.as_bytes().to_vec());
        let records = parse_csv(
            csv,
            &ENCOMBI_CSV,
            date,
            chrono_tz::Africa::Lagos,
            Duration::zero(),
        )
        .unwrap();
        assert_eq!(field_f64(&records[0], "pvinv_P_total"), 227_100.0);
        assert_eq!(field_f64(&records[0], "pv_capacity"), 350_000.0);
    }

    #[test]
    fn clock_offset_is_subtracted() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let csv = Cursor::new(NIGHT_ROW.as_bytes().to_vec());
        // 10s clock offset should shift the resulting UTC timestamp back by 10s
        let records = parse_csv(
            csv,
            &ENCOMBI_CSV,
            date,
            chrono_tz::Africa::Lagos,
            Duration::seconds(10),
        )
        .unwrap();
        assert_eq!(
            records[0].get_timestamp().unwrap().to_rfc3339(),
            "2026-05-05T23:01:06+00:00"
        );
    }

    #[test]
    fn negative_mains_emits_grid_out_p() {
        // PV exporting heavily: Mains = -120.5 kW (controller exporting to grid).
        // Per the discovery doc, this should be emitted as grid_out_P = 120_500 W
        // (absolutised), and no grid_in_P should be set on the record.
        let row = "12:00:00\t500.0\t0.0\t-120.5\t379.5\t550.0\t550.0\t0.0\t0.0\t0.0\t0\t1\t0\t1";
        let date = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let csv = Cursor::new(row.as_bytes().to_vec());
        let records = parse_csv(
            csv,
            &ENCOMBI_CSV,
            date,
            chrono_tz::Africa::Lagos,
            Duration::zero(),
        )
        .unwrap();
        assert_eq!(records.len(), 1);
        let r = &records[0];
        assert_eq!(field_f64(r, "grid_out_P"), 120_500.0);
        assert!(
            r.get_field("grid_in_P").is_none(),
            "grid_in_P must not be set on an export row"
        );
        // sanity: positive PV / Load fields unaffected by the sign-split
        assert_eq!(field_f64(r, "pvinv_P_total"), 500_000.0);
        assert_eq!(field_f64(r, "load_P"), 379_500.0);
    }
}
