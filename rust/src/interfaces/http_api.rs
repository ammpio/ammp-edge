use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use backoff::{retry_notify, Error, ExponentialBackoff};
use kvstore::KVDb;
use serde::Deserialize;

use crate::constants::keys;

const REQUEST_TIMEOUT: u64 = 60;

pub fn get_api_root(kvs: &KVDb) -> String {
    const DEFAULT_API_ROOT: &str = "https://edge.ammp.io/api/v0/";
    match kvs.get(keys::HTTP_API_ROOT) {
        Ok(Some(api_root)) => api_root,
        _ => DEFAULT_API_ROOT.to_string(),
    }
}

fn get_ureq_agent() -> Result<ureq::Agent> {
    Ok(ureq::AgentBuilder::new()
        .tls_connector(Arc::new(native_tls::TlsConnector::new()?))
        .timeout(Duration::from_secs(REQUEST_TIMEOUT))
        .build())
}

fn activation_step_1(agent: &ureq::Agent, api_root: &str, node_id: &str) -> Result<String> {
    #[derive(Debug, Deserialize)]
    struct R1 {
        access_key: String,
        message: String,
    }

    let request_step1 = || {
        log::debug!("Doing activation step 1");
        agent
            .get(&format!("{api_root}nodes/{node_id}/activate"))
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
    #[derive(Debug, Deserialize)]
    struct R2 {
        message: String,
    }

    let request_step2 = || {
        log::debug!("Doing activation step 2");
        agent
            .post(&format!("{api_root}nodes/{node_id}/activate"))
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
