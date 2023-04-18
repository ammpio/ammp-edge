use std::io::{Cursor, Read};

use zip::result::{ZipError, ZipResult};
use zip::ZipArchive;

use crate::interfaces::ftp;
use crate::node_mgmt::config::Device;

use super::SmaHyconCsvError;

pub const CSV_EXT: &str = ".csv";
pub const ZIP_EXT: &str = ".zip";

fn select_yesterdays_file(filenames: Vec<String>) -> Option<String> {
    // Note that yesterday is the last day for which data will be complete
    let mut filenames = filenames
        .iter()
        .filter(|f| f.ends_with(ZIP_EXT) || f.ends_with(CSV_EXT))
        .cloned() // Convert the filtered references to owned strings
        .collect::<Vec<String>>(); // Collect the owned strings into a vector
    filenames.sort(); // Sort the vector in alphabetical order
    filenames.get(filenames.len() - 2).cloned() // Get the second-to-last filename
}

pub fn get_base_url(device: &Device) -> Result<String, SmaHyconCsvError> {
    device
        .address
        .clone()
        .ok_or(SmaHyconCsvError::Address("missing address".into()))?
        .base_url
        .ok_or(SmaHyconCsvError::Address("missing base URL".into()))
}

pub fn download_last_day_file(
    device: &Device,
) -> Result<(String, Cursor<Vec<u8>>), SmaHyconCsvError> {
    let addr = get_base_url(device)?;

    let mut ftp_conn = ftp::FtpConnection::new(&addr)?;
    ftp_conn.connect()?;

    let filename = select_yesterdays_file(ftp_conn.list_files()?).ok_or(SmaHyconCsvError::File(
        "no ZIP or CSV data files found".into(),
    ))?;

    let file = ftp_conn.download_file(&filename)?;
    ftp_conn.disconnect().ok();
    Ok((filename, file))
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
