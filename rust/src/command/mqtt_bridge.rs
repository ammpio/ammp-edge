use std::sync::Arc;

use anyhow::Result;
use convoy::{
    Bridge, BridgeConfig, BrokerConfig, CacheConfig, CacheManager, ForwardRule, SubscribeRule,
    TlsConfig,
};
use kvstore::AsyncKVDb;

use crate::constants::{REMOTE_DEFAULTS, keys};
use crate::helpers::base_path;
use crate::interfaces::kvpath;
use crate::interfaces::mqtt::MQTT_BRIDGE_HOST_PORT;

/// Start the MQTT bridge
///
/// This function configures and starts the MQTT bridge using the Convoy library.
/// It creates a bridge configuration with local and remote broker settings,
/// and a cache manager for persistent storage. The bridge is then created and
/// started, and the event loop is run until the bridge is stopped.
///
/// Returns a Result indicating success or failure.
pub async fn mqtt_bridge() -> Result<()> {
    let kvs = AsyncKVDb::new(kvpath::SQLITE_STORE.as_path()).await?;
    let node_id: String = kvs
        .get(keys::NODE_ID)
        .await?
        .ok_or_else(|| anyhow::anyhow!("'node_id' not set in KV store; aborting"))?;
    let access_key: String = kvs
        .get(keys::ACCESS_KEY)
        .await?
        .ok_or_else(|| anyhow::anyhow!("'access_key' not set in KV store; aborting"))?;

    let bridge_config = BridgeConfig {
        local: BrokerConfig {
            addr: MQTT_BRIDGE_HOST_PORT.clone(),
            client_id: "convoy".to_string(),
            keep_alive_secs: 3600,
            clean_session: false,
            max_inflight: 100,
            username: None,
            password: None,
            tls: None,
        },
        remote: BrokerConfig {
            addr: REMOTE_DEFAULTS
                .get(keys::MQTT_BRIDGE_PROD)
                .unwrap()
                .to_string(),
            client_id: format!("{}-brg", &node_id),
            keep_alive_secs: 90,
            clean_session: false,
            max_inflight: 2,
            username: Some(node_id.clone()),
            password: Some(access_key),
            tls: Some(TlsConfig {
                ca_file: Some(base_path::ROOT_DIR.join("resources/certs/ca.crt")),
                client_cert: None,
                client_password: None,
                danger_accept_invalid_certs: false,
            }),
        },
        state_topic: format!("a/{node_id}/bridge_state"),
        state_online_payload: "1".to_string(),
        state_offline_payload: "0".to_string(),
        forward: vec![ForwardRule {
            local_filter: "u/#".to_string(),
            remote_prefix: format!("a/{node_id}/"),
            qos: 1,
        }],
        subscribe: vec![SubscribeRule {
            remote_filter: format!("a/{node_id}/d/#"),
            remote_prefix: format!("a/{node_id}/"),
            qos: 1,
        }],
    };

    // Configure cache
    let cache_config = CacheConfig {
        sqlite_path: base_path::DATA_DIR.join("convoy/cache.db"),
        flush_batch: 10,
        flush_interval_ms: 100,
        ..Default::default()
    };

    // Create cache manager
    let cache = Arc::new(CacheManager::new(cache_config)?);

    log::info!("Creating MQTT bridge...");

    // Create and run bridge
    let bridge = Bridge::new(bridge_config, cache).await?;

    log::info!("MQTT bridge created, starting event loop...");

    bridge.run().await?;

    Ok(())
}
