use crate::node_mgmt::config;

pub fn run_acquisition(config: &config::Config) {
    ()
}

fn select_devices_to_read(config: &config::Config) -> Vec<config::Device> {
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::node_mgmt::config;

    use once_cell::sync::Lazy;

    static SAMPLE_CONFIG_WITH_HYCON_CSV: Lazy<config::Config> = Lazy::new(|| {
        config::from_str(
            r#"
        {
            "devices": {
                "sma_hycon_csv": {
                    "desc": "SMA hybrid Controller - CSV backfill",
                    "driver": "sma_hycon_csv",
                    "address": {
                        "base_url": "ftp://172.16.1.21/fsc/log/DataFast/",
                        "user": "User",
                        "password": "pwd"
                    },
                    "enabled": true,
                    "vendor_id": "sma-hycon-1",
                    "device_model": "gen_control_sma_hycon",
                    "reading_type": "sma_hycon_csv"
                }
            },
            "readings": {},
            "timestamp": "1970-01-01T00:00:00Z"
        }
        "#,
        )
        .unwrap()
    });

    static SAMPLE_CONFIG_NO_HYCON_CSV: Lazy<config::Config> = Lazy::new(|| {
        config::from_str(
            r#"
        {
            "devices": {
                "sma_stp_1": {
                    "name": "SMA STP-25000",
                    "driver": "sma_stp25000",
                    "enabled": true,
                    "device_model": "pv_inv_sma",
                    "vendor_id": "1234567890",
                    "reading_type": "modbustcp",
                    "address": {
                        "host": "mock-sma-stp",
                        "unit_id": 3
                    }
                }
            },
            "readings": {},
            "timestamp": "1970-01-01T00:00:00Z"
        }
        "#,
        )
        .unwrap()
    });

    #[test]
    fn check_selected_devices() {
        assert!(select_devices_to_read(&SAMPLE_CONFIG_NO_HYCON_CSV).is_empty());
        assert_eq!(
            select_devices_to_read(&SAMPLE_CONFIG_WITH_HYCON_CSV)[0],
            SAMPLE_CONFIG_WITH_HYCON_CSV.devices["sma_hycon_csv"]
        );
    }
}
