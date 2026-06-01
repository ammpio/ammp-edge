use std::io::Cursor;

use chrono::NaiveDate;

use crate::interfaces::ftp;
use crate::node_mgmt::config::Device;

use super::EncombiCsvError;

const PROD_SUFFIX: &str = "_Prod.txt";

pub fn filename_for_date(date: NaiveDate) -> String {
    format!("{}{}", date.format("%Y-%m-%d"), PROD_SUFFIX)
}

pub fn get_base_url(device: &Device) -> Result<String, EncombiCsvError> {
    device
        .address
        .clone()
        .ok_or_else(|| EncombiCsvError::Address("missing address".into()))?
        .base_url
        .ok_or_else(|| EncombiCsvError::Address("missing base URL".into()))
}

pub fn download_file_for_date(
    device: &Device,
    date: NaiveDate,
) -> Result<Cursor<Vec<u8>>, EncombiCsvError> {
    let addr = get_base_url(device)?;
    let filename = filename_for_date(date);

    let mut ftp_conn = ftp::FtpConnection::new(&addr)?;
    ftp_conn.connect()?;
    let file = ftp_conn.download_file(&filename)?;
    ftp_conn.disconnect().ok();
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_formatting() {
        assert_eq!(
            filename_for_date(NaiveDate::from_ymd_opt(2026, 5, 6).unwrap()),
            "2026-05-06_Prod.txt"
        );
        assert_eq!(
            filename_for_date(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            "2026-12-31_Prod.txt"
        );
    }
}
