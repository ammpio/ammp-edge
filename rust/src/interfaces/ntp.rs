use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

use sntpc;
use sntpc::{Error, NtpContext, NtpTimestampGenerator, NtpUdpSocket};
use thiserror::Error;

const DEFAULT_NTP_PORT: u16 = 123;

#[derive(Error, Debug)]
pub enum NtpError {
    #[error("sntpc error: {0}")]
    Client(String),
    #[error("invalid time")]
    InvalidTime,
}

#[derive(Copy, Clone, Default)]
struct StdTimestampGen {
    duration: Duration,
}

impl NtpTimestampGenerator for StdTimestampGen {
    fn init(&mut self) {
        self.duration = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap();
    }

    fn timestamp_sec(&self) -> u64 {
        self.duration.as_secs()
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        self.duration.subsec_micros()
    }
}

#[derive(Debug)]
struct UdpSocketWrapper(UdpSocket);

impl NtpUdpSocket for UdpSocketWrapper {
    fn send_to<T: ToSocketAddrs>(&self, buf: &[u8], addr: T) -> Result<usize, Error> {
        match self.0.send_to(buf, addr) {
            Ok(usize) => Ok(usize),
            Err(_) => Err(Error::Network),
        }
    }

    fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), Error> {
        match self.0.recv_from(buf) {
            Ok((size, addr)) => Ok((size, addr)),
            Err(_) => Err(Error::Network),
        }
    }
}

pub fn ntp_query(hostname: String, port: Option<u16>) -> Result<u32, NtpError> {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Unable to crate UDP socket");
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("Unable to set UDP socket read timeout");

    let sock_wrapper = UdpSocketWrapper(socket);
    let ntp_context = NtpContext::new(StdTimestampGen::default());
    let result = sntpc::get_time(
        format!("{}:{}", hostname, port.unwrap_or(DEFAULT_NTP_PORT)).as_str(),
        sock_wrapper,
        ntp_context,
    );

    let time = result.map_err(map_sntpc_error)?;
    match time.sec() {
        0 => Err(NtpError::InvalidTime),
        _ => Ok(time.sec()),
    }
}

fn map_sntpc_error(e: sntpc::Error) -> NtpError {
    NtpError::Client(format!("{:?}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ntp_time_from_localhost() {
        let epoch_time = ntp_query("localhost".to_string(), None).unwrap();
        assert!(epoch_time > 0);
    }
}
