use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use backoff::{retry_notify, Error, ExponentialBackoff};
use serde::Deserialize;

use crate::node_mgmt::generate_node_id;
use crate::interfaces::kvstore;
use crate::interfaces::keys;

const REQUEST_TIMEOUT: u64 = 60;
const REMOTE_API_ROOT: &str = "https://edge.stage.ammp.io/api/v0/";

fn activate(node_id: &str) -> Result<String> {
    #[derive(Debug, Deserialize)]
    struct R1 {
        access_key: String,
        message: String,
    }

    #[derive(Debug, Deserialize)]
    struct R2 {
        message: String,
    }

    let agent = ureq::AgentBuilder::new()
        .tls_connector(Arc::new(native_tls::TlsConnector::new()?))
        .timeout(Duration::from_secs(REQUEST_TIMEOUT))
        .build();

    let request_step1 = || {
        log::debug!("Doing activation step 1");
        agent
            .get(&format!("{REMOTE_API_ROOT}nodes/{node_id}/activate"))
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

    let request_step2 = || {
        log::debug!("Doing activation step 2");
        agent
            .post(&format!("{REMOTE_API_ROOT}nodes/{node_id}/activate"))
            .set("Authorization", &access_key)
            .call()
            .map_err(Error::transient)
    };

    let resp2: R2 =
        retry_notify(ExponentialBackoff::default(), request_step2, notify)?.into_json()?;
    log::debug!(
        "Carried out second step of activation. Message: {}",
        resp2.message
    );

    Ok(access_key)
}

pub fn init() -> Result<()> {
    let node_id = generate_node_id();
    log::info!("Node ID is {}", node_id);

    let access_key = activate(&node_id)?;
    kvstore::set_many([(keys::NODE_ID, &node_id), (keys::ACCESS_KEY, &access_key)].to_vec())?;
    log::info!("Activation successfully completed");

    Ok(())
}
