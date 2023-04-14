use std::{fmt::Display, io, process::Command, thread, time};

use anyhow::anyhow;

use crate::{helpers, interfaces::mqtt};

const TIMEDATECTL_CMD: &str = "/usr/bin/timedatectl";
const TIMEDATECTL_SUBCMD: &str = "show";

fn into_permanent_err<E: Display>(err: E) -> backoff::Error<String> {
    backoff::Error::Permanent(err.to_string())
}

pub fn wait_for_time_source() -> anyhow::Result<()> {
    let check_time_sync = || {
        let timedatectl_output = run_timedatectl_show().map_err(into_permanent_err)?;
        let (rtc_time_0, ntp_sync) = rtc_time_and_ntp_status(&timedatectl_output);

        if ntp_sync {
            // We're good. No need to check the RTC.
            log::info!("NTP synchronized; proceeding");
            return Ok(());
        }

        // In some cases with malfuncitoning RTC, it appears "stuck" at a fixed time, so here we check
        // that it's actually advancing.
        thread::sleep(time::Duration::from_secs(1));
        let timedatectl_output = run_timedatectl_show().map_err(into_permanent_err)?;
        let (rtc_time_1, _) = rtc_time_and_ntp_status(&timedatectl_output);

        // If RTC is advancing then we're probably good. Otherwise we keep retrying.
        if rtc_time_1 != rtc_time_0 {
            log::info!("RTC appears functional; proceeding");
            Ok(())
        } else {
            let err_msg = format!(
                "no time source available; 'timedatectl show' output: {:?}",
                timedatectl_output
            );
            mqtt::publish_log_msg(&err_msg).ok();
            Err(backoff::Error::transient(err_msg))
        }
    };

    match helpers::backoff_retry(check_time_sync, None) {
        Ok(_) => Ok(()),
        Err(backoff::Error::Permanent(e)) => Err(anyhow!("unable to check time sources: {}", e)),
        // In principle transient errors would be retried infinitely, so this should never happen, but needs to be handled...
        Err(backoff::Error::Transient {
            err: e,
            retry_after: _,
        }) => Err(anyhow!(
            "transient error while checking time sources: {}",
            e
        )),
    }
}

fn run_timedatectl_show() -> Result<String, io::Error> {
    let output = Command::new(TIMEDATECTL_CMD)
        .arg(TIMEDATECTL_SUBCMD)
        .output()?;

    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("timedatectl returned exit code {:?}", output.status.code()),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn rtc_time_and_ntp_status(timedatectl_output: &str) -> (Option<String>, bool) {
    let rtc_time = timedatectl_output
        .lines()
        .find(|line| line.starts_with("RTCTimeUSec="))
        .and_then(|line| line.split_once('='))
        .map(|(_, value)| value.to_string());

    let ntp_sync = timedatectl_output.contains("NTPSynchronized=yes");

    (rtc_time, ntp_sync)
}

#[cfg(test)]
mod test {
    use super::*;

    const TIMEDATECTL_NTP_RTC: &str = r#"
Timezone=Etc/UTC
LocalRTC=no
CanNTP=yes
NTP=yes
NTPSynchronized=yes
TimeUSec=Thu 2023-04-13 21:03:24 UTC
RTCTimeUSec=Thu 2023-04-13 21:03:24 UTC
"#;

    const TIMEDATECTL_NO_NTP_RTC_0: &str = r#"
Timezone=Etc/UTC
LocalRTC=no
CanNTP=yes
NTP=yes
NTPSynchronized=no
TimeUSec=Thu 2023-04-13 21:03:24 UTC
RTCTimeUSec=Thu 2023-04-13 21:03:24 UTC
"#;

    const TIMEDATECTL_NO_NTP_RTC_1: &str = r#"
Timezone=Etc/UTC
LocalRTC=no
CanNTP=yes
NTP=yes
NTPSynchronized=no
TimeUSec=Thu 2023-04-13 21:03:25 UTC
RTCTimeUSec=Thu 2023-04-13 21:03:25 UTC
"#;

    #[test]
    fn test_parse_timedatectl_output() {
        let (rtc_time, ntp_sync) = rtc_time_and_ntp_status(TIMEDATECTL_NTP_RTC);
        println!("rtc_time: {:?}", rtc_time);
        assert!(!rtc_time.unwrap().is_empty());
        assert!(ntp_sync);

        let (rtc_time_0, ntp_sync) = rtc_time_and_ntp_status(TIMEDATECTL_NO_NTP_RTC_0);
        assert!(!ntp_sync);

        let (rtc_time_1, ntp_sync) = rtc_time_and_ntp_status(TIMEDATECTL_NO_NTP_RTC_1);
        assert!(rtc_time_0 != rtc_time_1);
        assert!(!ntp_sync);
    }
}
