use std::io::Cursor;

use native_tls::TlsConnector;
use suppaftp::{FtpError, NativeTlsConnector, NativeTlsFtpStream};
use thiserror::Error;

const DEFAULT_FTP_PORT: u16 = 21;

#[derive(Error, Debug)]
pub enum FtpConnError {
    #[error(transparent)]
    FtpError(#[from] FtpError),
    #[error("path {0} is not valid")]
    PathError(String),
    #[error("not connected")]
    NotConnected,
    #[error(transparent)]
    UrlParse(#[from] url::ParseError),
    #[error("address error: {0}")]
    Address(String),
}

pub struct FtpConnection {
    host: String,
    port: u16,
    user: String,
    password: String,
    secure: bool,
    base_path: String,
    ftp_stream: Option<NativeTlsFtpStream>,
}

impl FtpConnection {
    pub fn new(url: &str) -> Result<Self, FtpConnError> {
        let url = url::Url::parse(url)?;
        let host = url
            .host_str()
            .ok_or_else(|| FtpConnError::Address("hostname missing from URL".into()))?
            .to_string();
        let port = url.port().unwrap_or(DEFAULT_FTP_PORT);
        let user = url.username().to_string();
        let password = url.password().unwrap_or("").to_string();
        let secure = url.scheme() == "ftps";
        let base_path = url
            .path()
            .strip_prefix('/')
            .unwrap_or_else(|| url.path())
            .to_owned();
        Ok(FtpConnection {
            host,
            port,
            user,
            password,
            secure,
            base_path,
            ftp_stream: None,
        })
    }

    pub fn connect(&mut self) -> Result<(), FtpConnError> {
        let addr = &format!("{}:{}", self.host, self.port);
        let mut ftp_stream;
        if self.secure {
            let ctx = NativeTlsConnector::from(
                TlsConnector::builder()
                    .danger_accept_invalid_certs(true)
                    .danger_accept_invalid_hostnames(true)
                    .build()
                    .unwrap(),
            );
            ftp_stream = NativeTlsFtpStream::connect_secure_implicit(addr, ctx, &self.host)?;
        } else {
            ftp_stream = NativeTlsFtpStream::connect(addr)?;
        }

        ftp_stream.login(&self.user, &self.password)?;
        ftp_stream.set_passive_nat_workaround(true);
        ftp_stream.cwd(&self.base_path)?;
        self.ftp_stream = Some(ftp_stream);
        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<(), FtpConnError> {
        if let Some(ftp_stream) = self.ftp_stream.as_mut() {
            ftp_stream.quit()?;
        }
        Ok(())
    }

    pub fn list_files(&mut self) -> Result<Vec<String>, FtpConnError> {
        self.ftp_stream
            .as_mut()
            .ok_or(FtpConnError::NotConnected)?
            .nlst(None)
            .map_err(Into::into)
    }

    pub fn download_file(&mut self, filename: &str) -> Result<Cursor<Vec<u8>>, FtpConnError> {
        self.ftp_stream
            .as_mut()
            .ok_or(FtpConnError::NotConnected)?
            .retr_as_buffer(filename)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const BASE_URL: &str = "ftp://testuser:TestPWD123!@localhost:21/fsc/log/DataFast";
    const HOST: &str = "localhost";
    const PORT: u16 = 21;
    const USER: &str = "testuser";
    const PASSWORD: &str = "TestPWD123!";
    const CSV_DIR: &str = "fsc/log/DataFast";
    const CSV_FILENAME: &str = "LogDataFast_2023-03-10.csv";
    const SECURE: bool = false;
    const CSV_FILE_START: &[u8] = b"Version";
    const NUM_FILES_IN_CSV_DIR: usize = 3;

    #[test]
    fn test_url_parse() {
        let conn = FtpConnection::new(BASE_URL).unwrap();
        assert_eq!(conn.host, HOST);
        assert_eq!(conn.port, PORT);
        assert_eq!(conn.user, USER);
        assert_eq!(conn.password, PASSWORD);
        assert_eq!(conn.secure, SECURE);
        assert_eq!(conn.base_path, CSV_DIR);
    }

    #[test]
    fn test_connect() {
        let mut conn = FtpConnection::new(BASE_URL).unwrap();
        assert!(conn.connect().is_ok());
        assert!(conn.disconnect().is_ok());
    }

    #[test]
    fn test_download_file() {
        let mut conn = FtpConnection::new(BASE_URL).unwrap();
        assert!(conn.connect().is_ok());
        let cursor = conn.download_file(CSV_FILENAME).unwrap();
        assert!(cursor.into_inner().starts_with(CSV_FILE_START));
        assert!(conn.disconnect().is_ok());
    }

    #[test]
    fn test_get_directory_listing() {
        let mut conn = FtpConnection::new(BASE_URL).unwrap();
        assert!(conn.connect().is_ok());
        assert!(conn.list_files().unwrap().len() == NUM_FILES_IN_CSV_DIR);
        assert!(conn.disconnect().is_ok());
    }
}
