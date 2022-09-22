use anyhow::Result;

use crate::interfaces::mqtt;

pub fn mqtt_pub_meta() -> Result<()> {
    mqtt::publish_one(
        "u/meta/snap".into(),
        "abc".as_bytes().into(),
        None,
        None,
    )
}
