use once_cell::sync::Lazy;
use std::collections::HashMap;

use super::keys;

pub static REMOTE_DEFAULTS: Lazy<HashMap<&str, &str>> =
    Lazy::new(|| HashMap::from([(keys::HTTP_API_BASE_URL, "https://edge.ammp.io/api/v0")]));
