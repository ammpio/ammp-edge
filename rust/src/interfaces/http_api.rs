use anyhow::Result;
use kvstore::KVDb;
use serde::{Deserialize, Serialize};

use crate::constants::{defaults, keys, REMOTE_DEFAULTS};
use crate::helpers;

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
    let config = ureq::config::Config::builder()
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .build(),
        )
        .timeout_global(Some(defaults::API_REQUEST_TIMEOUT))
        .build();
    Ok(ureq::Agent::new_with_config(config))
}

fn activation_step_1(agent: &ureq::Agent, api_root: &str, node_id: &str) -> Result<String> {
    let request_step1 = || {
        log::debug!("Doing activation step 1");
        agent
            .get(&format!("{api_root}/nodes/{node_id}/activate"))
            .call()
            .map_err(backoff::Error::transient)
    };

    let resp1: R1 = helpers::backoff_retry(request_step1, None)?
        .body_mut()
        .read_json()?;
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
            .header("Authorization", access_key)
            .send_empty()
            .map_err(backoff::Error::transient)
    };

    let resp2: R2 = helpers::backoff_retry(request_step2, None)?
        .body_mut()
        .read_json()?;
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
        let mut server = mockito::Server::new();
        let api_base_url = server.url();
        let m1 = server
            .mock("GET", &**ACTIVATION_PATH)
            .with_body(serde_json::to_vec(&*SAMPLE_RESP_1).unwrap())
            .expect(1)
            .create();
        let m2 = server
            .mock("POST", &**ACTIVATION_PATH)
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

        let mut server = mockito::Server::new();
        let api_base_url = server.url();
        let m1_error = server
            .mock("GET", &**ACTIVATION_PATH)
            .with_status(400)
            .expect(2)
            .create();

        let m1_success = server
            .mock("GET", &**ACTIVATION_PATH)
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
