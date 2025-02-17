use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::SystemTime;

use chrono::Duration;
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
    duration: std::time::Duration,
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

fn run_ntp_query(hostname: &str, port: Option<u16>) -> Result<NtpResult, NtpError> {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Unable to crate UDP socket");
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("Unable to set UDP socket read timeout");
    let sock_wrapper = UdpSocketWrapper(socket);
    let ntp_context = NtpContext::new(StdTimestampGen::default());

    sntpc::get_time(
        format!("{}:{}", hostname, port.unwrap_or(DEFAULT_NTP_PORT)).as_str(),
        &sock_wrapper,
        ntp_context,
    )
    .map_err(map_sntpc_error)
}

pub fn query_epoch(hostname: &str, port: Option<u16>) -> Result<Duration, NtpError> {
    let ntp_result = run_ntp_query(hostname, port)?;
    println!("ntp_result: {:?}", ntp_result);
    match ntp_result.sec() {
        0 => Err(NtpError::InvalidTime),
        _ => Ok(Duration::seconds(ntp_result.sec() as i64)
            + Duration::nanoseconds(
                ntp_result.sec_fraction() as i64 * 1_000_000_000 / u32::MAX as i64,
            )),
    }
}

pub fn query_offset_wrt_systime(hostname: &str, port: Option<u16>) -> Result<Duration, NtpError> {
    let ntp_result = run_ntp_query(hostname, port)?;
    Ok(Duration::microseconds(ntp_result.offset()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ntp_time_from_localhost() {
        let epoch_from_ntp = query_epoch("localhost", None).unwrap();
        let epoch_from_sys = Duration::from_std(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap(),
        )
        .unwrap();

        println!("epoch_from_ntp: {:?}", epoch_from_ntp);
        println!("epoch_from_sys: {:?}", epoch_from_sys);
        assert!((epoch_from_ntp - epoch_from_sys).num_seconds().abs() < 1);
    }

    #[test]
    fn test_ntp_delta_from_localhost() {
        let offset = query_offset_wrt_systime("localhost", None).unwrap();
        println!("offset: {:?}", offset);
        assert!(offset.num_seconds().abs() < 1);
    }
}
