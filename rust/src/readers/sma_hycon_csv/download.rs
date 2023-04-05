use std::io::{Cursor, Read};

use zip::result::{ZipError, ZipResult};
use zip::ZipArchive;

use crate::interfaces::ftp;
use crate::node_mgmt::config::Device;

use super::SmaHyconCsvError;

fn select_last_day_zip(filenames: Vec<String>) -> Option<String> {
    filenames
        .iter()
        .filter(|f| f.ends_with(".zip"))
        .max() // The filenames are alphabetical by date so we can take the "largest"
        .cloned()
}

pub fn download_last_day_zip(device: &Device) -> Result<Cursor<Vec<u8>>, SmaHyconCsvError> {
    let addr = &device
        .address
        .clone()
        .ok_or(SmaHyconCsvError::Address("missing address".into()))?
        .base_url
        .ok_or(SmaHyconCsvError::Address("missing base URL".into()))?;

    let mut ftp_conn = ftp::FtpConnection::new(addr);
    ftp_conn.connect()?;

    let filename = select_last_day_zip(ftp_conn.list_files()?)
        .ok_or(SmaHyconCsvError::File("no ZIP files found".into()))?;

    let file = ftp_conn.download_file(&filename)?;
    ftp_conn.disconnect();
    Ok(file)
}

pub fn extract_file_from_zip(cursor: Cursor<Vec<u8>>) -> ZipResult<Cursor<Vec<u8>>> {
    let mut zip_archive = ZipArchive::new(cursor)?;

    for i in 0..zip_archive.len() {
        let mut zip_file = zip_archive.by_index(i)?;

        if zip_file.is_file() && zip_file.name().ends_with(".csv") {
            let mut file_data = Vec::new();
            zip_file.read_to_end(&mut file_data)?;

            return Ok(Cursor::new(file_data));
        }
    }
    // This isn't quite the right error to return if there are no CSV files in the archive
    // but it does the job for now...
    Err(ZipError::FileNotFound)
}
