use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, SystemTime};

use sntpc::{self, NtpResult};
use sntpc::{NtpContext, NtpTimestampGenerator, NtpUdpSocket};
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
        self.duration = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
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
    fn send_to<T: ToSocketAddrs>(&self, buf: &[u8], addr: T) -> Result<usize, sntpc::Error> {
        match self.0.send_to(buf, addr) {
            Ok(usize) => Ok(usize),
            Err(_) => Err(sntpc::Error::Network),
        }
    }

    fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), sntpc::Error> {
        match self.0.recv_from(buf) {
            Ok((size, addr)) => Ok((size, addr)),
            Err(_) => Err(sntpc::Error::Network),
        }
    }
}

fn map_sntpc_error(e: sntpc::Error) -> NtpError {
    NtpError::Client(format!("{:?}", e))
}

fn run_ntp_query(hostname: String, port: Option<u16>) -> Result<NtpResult, NtpError> {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Unable to crate UDP socket");
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("Unable to set UDP socket read timeout");
    let sock_wrapper = UdpSocketWrapper(socket);
    let ntp_context = NtpContext::new(StdTimestampGen::default());

    sntpc::get_time(
        format!("{}:{}", hostname, port.unwrap_or(DEFAULT_NTP_PORT)).as_str(),
        sock_wrapper,
        ntp_context,
    )
    .map_err(map_sntpc_error)
}

pub fn query_epoch(hostname: String, port: Option<u16>) -> Result<Duration, NtpError> {
    let ntp_result = run_ntp_query(hostname, port)?;
    match ntp_result.sec() {
        0 => Err(NtpError::InvalidTime),
        _ => Ok(Duration::from_secs_f64(
            ntp_result.sec() as f64 + ntp_result.sec_fraction() as f64 / u32::MAX as f64,
        )),
    }
}

pub fn query_delta_wrt_systime_micros(
    hostname: String,
    port: Option<u16>,
) -> Result<i64, NtpError> {
    let ntp_result = run_ntp_query(hostname, port)?;
    // This returns offset in microseconds
    Ok(ntp_result.offset())
}

pub fn query_delta_wrt_systime_secs(hostname: String, port: Option<u16>) -> Result<i64, NtpError> {
    let offset_micros = query_delta_wrt_systime_micros(hostname, port)?;
    Ok(offset_micros / 1_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ntp_time_from_localhost() {
        let epoch_from_ntp = query_epoch("localhost".to_string(), None).unwrap();
        let epoch_from_sys = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();

        println!("epoch_from_ntp: {:?}", epoch_from_ntp);
        println!("epoch_from_sys: {:?}", epoch_from_sys);
        assert!((epoch_from_ntp.as_millis() - epoch_from_sys.as_millis()) < 1000);
    }

    #[test]
    fn test_ntp_delta_from_localhost() {
        let delta = query_delta_wrt_systime_micros("localhost".to_string(), None).unwrap();
        println!("delta: {:?}", delta);
        assert!(delta < 1_000_000);
    }
}
