use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use backoff::{retry_notify, Error, ExponentialBackoff};
use kvstore::KVDb;
use serde::{Deserialize, Serialize};

use crate::constants::{defaults, keys, REMOTE_DEFAULTS};

#[derive(Debug, Deserialize, Serialize)]
struct R1 {
    access_key: String,
    message: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct R2 {
    message: String,
}

pub fn get_api_base_url(kvs: &KVDb) -> String {
    match kvs.get(keys::HTTP_API_BASE_URL) {
        Ok(Some(base_url)) => base_url,
        _ => REMOTE_DEFAULTS
            .get(keys::HTTP_API_BASE_URL)
            .unwrap()
            .to_string(),
    }
}

fn get_ureq_agent() -> Result<ureq::Agent> {
    Ok(ureq::AgentBuilder::new()
        .tls_connector(Arc::new(native_tls::TlsConnector::new()?))
        .timeout(defaults::API_REQUEST_TIMEOUT)
        .build())
}

fn activation_step_1(agent: &ureq::Agent, api_root: &str, node_id: &str) -> Result<String> {
    let request_step1 = || {
        log::debug!("Doing activation step 1");
        agent
            .get(&format!("{api_root}/nodes/{node_id}/activate"))
            .call()
            .map_err(Error::transient)
    };

    let notify = |err, dur: Duration| {
        log::error!("Request error after {:.1}s: {}", dur.as_secs_f32(), err);
    };

    let resp1: R1 =
        retry_notify(ExponentialBackoff::default(), request_step1, notify)?.into_json()?;
    let access_key = resp1.access_key;

    log::debug!(
        "Carried out first step of activation. Access key: {}; Message: {}",
        access_key,
        resp1.message
    );

    Ok(access_key)
}

fn activation_step_2(
    agent: &ureq::Agent,
    api_root: &str,
    node_id: &str,
    access_key: &str,
) -> Result<()> {
    let request_step2 = || {
        log::debug!("Doing activation step 2");
        agent
            .post(&format!("{api_root}/nodes/{node_id}/activate"))
            .set("Authorization", access_key)
            .call()
            .map_err(Error::transient)
    };

    let notify = |err, dur: Duration| {
        log::error!("Request error after {:.1}s: {}", dur.as_secs_f32(), err);
    };

    let resp2: R2 =
        retry_notify(ExponentialBackoff::default(), request_step2, notify)?.into_json()?;
    log::debug!(
        "Carried out second step of activation. Message: {}",
        resp2.message
    );
    Ok(())
}

pub fn activate(api_root: &str, node_id: &str) -> Result<String> {
    let agent = get_ureq_agent()?;
    let access_key = activation_step_1(&agent, api_root, node_id)?;
    activation_step_2(&agent, api_root, node_id, &access_key)?;
    Ok(access_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    use mockito::mock;
    use once_cell::sync::Lazy;

    const SAMPLE_NODE_ID: &str = "abcdef123456";
    const SAMPLE_ACCESS_KEY: &str = "secret";
    static ACTIVATION_PATH: Lazy<String> =
        Lazy::new(|| format!("/nodes/{SAMPLE_NODE_ID}/activate"));
    static SAMPLE_RESP_1: Lazy<R1> = Lazy::new(|| {
        R1 {
        access_key: SAMPLE_ACCESS_KEY.to_string(),
        message: "Activation request approved. Please use provided key to verify access and confirm activation.".to_string(),
    }
    });
    static SAMPLE_RESP_2: Lazy<R2> = Lazy::new(|| R2 {
        message: format!("Node {SAMPLE_NODE_ID} successfully activated"),
    });

    #[test]
    fn test_activation_successful() {
        let api_base_url = mockito::server_url();
        let m1 = mock("GET", &**ACTIVATION_PATH)
            .with_body(serde_json::to_vec(&*SAMPLE_RESP_1).unwrap())
            .expect(1)
            .create();
        let m2 = mock("POST", &**ACTIVATION_PATH)
            .match_header("Authorization", SAMPLE_ACCESS_KEY)
            .with_body(serde_json::to_vec(&*SAMPLE_RESP_2).unwrap())
            .expect(1)
            .create();

        assert_eq!(
            activate(&api_base_url, SAMPLE_NODE_ID).unwrap(),
            SAMPLE_ACCESS_KEY
        );
        m1.assert();
        m2.assert();
    }

    #[test]
    fn test_activation_after_error() {
        use std::thread;

        let api_base_url = mockito::server_url();
        let m1_error = mock("GET", &**ACTIVATION_PATH)
            .with_status(400)
            .expect(2)
            .create();

        let m1_success = mock("GET", &**ACTIVATION_PATH)
            .with_body(serde_json::to_vec(&*SAMPLE_RESP_1).unwrap())
            .expect(1)
            .create();

        let agent = get_ureq_agent().unwrap();
        let activation_thread =
            thread::spawn(move || activation_step_1(&agent, &api_base_url, SAMPLE_NODE_ID));

        let access_key = activation_thread.join().unwrap().unwrap();

        assert_eq!(access_key, SAMPLE_ACCESS_KEY);
        m1_error.assert();
        m1_success.assert();
    }
}
