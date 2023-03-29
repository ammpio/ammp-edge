pub const PAYLOAD_1: &str = r#"
{
    "name": "Basic config",
    "devices": {
      "logger": {
        "name": "Logger",
        "driver": "sys_generic",
        "enabled": true,
        "device_model": "gateway_ammp",
        "vendor_id": "strato-1",
        "reading_type": "sys"
      },
      "sma_stp_1": {
        "name": "SMA STP-25000 (good)",
        "driver": "sma_stp25000",
        "enabled": true,
        "device_model": "pv_inv_sma",
        "vendor_id": "1234567890",
        "reading_type": "modbustcp",
        "address": {
          "host": "mock-sma-stp",
          "unit_id": 3
        }
      },
      "sma_stp_2": {
        "name": "SMA STP-25000 (bad)",
        "driver": "sma_stp25000",
        "enabled": true,
        "device_model": "pv_inv_sma",
        "vendor_id": "000",
        "reading_type": "modbustcp",
        "address": {
          "host": "mock-sma-stp",
          "unit_id": 100
        }
      }
    },
    "readings": {
      "comms_lggr_boot_time": {"device": "logger", "var": "boot_time"},
      "comms_lggr_cpu_load": {"device": "logger", "var": "cpu_load"},
      "comms_lggr_disk_usage": {"device": "logger", "var": "disk_usage"},
      "comms_lggr_mem_usage": {"device": "logger", "var": "memory_usage"},
      "pv_P_1": {"device": "sma_stp_1", "var": "P_total"},
      "pv_E_1": {"device": "sma_stp_1", "var": "total_yield"},
      "pv_P_2": {"device": "sma_stp_2", "var": "P_total"}
    },
    "timestamp": "2022-08-15T13:03:17Z",
    "read_interval": 15,
    "read_roundtime": true
}
"#;

pub const BAD_PAYLOAD: &str = "blah";
