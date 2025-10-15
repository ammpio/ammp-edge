use tracing_subscriber::EnvFilter;

use crate::constants::{defaults, envvars};

/// Initialize tracing subscriber with compact format
///
/// Respects LOG_LEVEL env var, or RUST_LOG if set.
/// Defaults to "info" level if neither is set.
///
/// Timestamps are disabled since journald adds its own timestamps in production.
pub fn init_tracing() {
    let log_level =
        std::env::var(envvars::LOG_LEVEL).unwrap_or_else(|_| defaults::LOG_LEVEL.to_string());

    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&log_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .compact()
        .init();
}
