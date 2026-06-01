use std::io::Cursor;

use chrono::NaiveDate;

use crate::interfaces::ftp;
use crate::node_mgmt::config::Device;

use super::EncombiCsvError;

pub const PROD_SUFFIX: &str = "_Prod.txt";

// Pick yesterday's production log — the last day for which data is complete.
// Filenames are `YYYY-MM-DD_Prod.txt`, so alphabetical sort is chronological.
fn select_yesterdays_file(filenames: Vec<String>) -> Option<String> {
    let mut files = filenames
        .into_iter()
        .filter(|f| f.ends_with(PROD_SUFFIX))
        .collect::<Vec<String>>();
    files.sort();
    match files.len() {
        0 => None,
        1 => files.pop(),               // only one day present; use it
        n => files.get(n - 2).cloned(), // second-to-last == yesterday
    }
}

// Derive the log date from the filename, e.g. "2026-05-06_Prod.txt" -> 2026-05-06.
pub fn date_from_filename(filename: &str) -> Result<NaiveDate, EncombiCsvError> {
    let base = filename.rsplit('/').next().unwrap_or(filename);
    let date_str = base
        .strip_suffix(PROD_SUFFIX)
        .ok_or_else(|| EncombiCsvError::File(format!("unexpected filename: {filename}")))?;
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|e| EncombiCsvError::File(format!("cannot parse date from {filename}: {e}")))
}

pub fn get_base_url(device: &Device) -> Result<String, EncombiCsvError> {
    device
        .address
        .clone()
        .ok_or_else(|| EncombiCsvError::Address("missing address".into()))?
        .base_url
        .ok_or_else(|| EncombiCsvError::Address("missing base URL".into()))
}

pub fn download_last_day_file(
    device: &Device,
) -> Result<(String, Cursor<Vec<u8>>), EncombiCsvError> {
    let addr = get_base_url(device)?;

    let mut ftp_conn = ftp::FtpConnection::new(&addr)?;
    ftp_conn.connect()?;

    let filename = select_yesterdays_file(ftp_conn.list_files()?)
        .ok_or_else(|| EncombiCsvError::File("no _Prod.txt data files found".into()))?;

    let file = ftp_conn.download_file(&filename)?;
    ftp_conn.disconnect().ok();
    Ok((filename, file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_second_to_last_prod_file() {
        let files = vec![
            "2026-05-04_Prod.txt".to_string(),
            "2026-05-05_Prod.txt".to_string(),
            "2026-05-06_Prod.txt".to_string(),
            "2026-05-06_Sum.txt".to_string(), // must be ignored
            "ProdViewLog.txt".to_string(),    // index file, ignored
        ];
        assert_eq!(
            select_yesterdays_file(files),
            Some("2026-05-05_Prod.txt".to_string())
        );
    }

    #[test]
    fn single_prod_file_is_used() {
        let files = vec![
            "2026-05-06_Prod.txt".to_string(),
            "2026-05-06_Evt.txt".to_string(),
        ];
        assert_eq!(
            select_yesterdays_file(files),
            Some("2026-05-06_Prod.txt".to_string())
        );
    }

    #[test]
    fn no_prod_files() {
        let files = vec!["ProdViewLog.txt".to_string(), "tmysqlinfo.ini".to_string()];
        assert_eq!(select_yesterdays_file(files), None);
    }

    #[test]
    fn date_parsed_from_filename() {
        assert_eq!(
            date_from_filename("2026-05-06_Prod.txt").unwrap(),
            NaiveDate::from_ymd_opt(2026, 5, 6).unwrap()
        );
        assert_eq!(
            date_from_filename("/some/path/2026-12-31_Prod.txt").unwrap(),
            NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()
        );
        assert!(date_from_filename("garbage.txt").is_err());
    }
}
