#![allow(dead_code)]
// This is infuriating, but rust-analyzer seems to arbitrarily think
// that some of these are unused; hence the warning suppression

pub const VALID_PAYLOAD_1: &str = r#"
{
    "name": "Basic config",
    "output": [
      {
        "field": "P_total",
        "device": "sma_stp_1",
        "source": "sma_stp_1[var = \"P_L1\"].value + sma_stp_1[var = \"P_L2\"].value + sma_stp_1[var = \"P_L3\"].value",
        "typecast": "float"
      },
      {
        "field": "genset_P_total",
        "source": "sma_hycon_csv[var = \"genset_P\"].value",
        "typecast": "float"
      }
    ],
    "devices": {
      "logger": {
        "key": "logger",
        "name": "Logger",
        "driver": "sys_generic",
        "enabled": true,
        "device_model": "gateway_ammp",
        "vendor_id": "strato-1",
        "reading_type": "sys"
      },
      "sma_stp_1": {
        "key": "sma_stp_1",
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
        "key": "sma_stp_2",
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
      },
      "sma_hycon_csv": {
        "key": "sma_hycon_csv",
        "name": "SMA Hybrid Controller - CSV backfill",
        "driver": "sma_hycon_csv",
        "address": {
          "base_url": "ftp://testuser:TestPWD123!@localhost:21/fsc/log/DataFast/",
          "timezone": "Europe/Amsterdam"
        },
        "enabled": true,
        "vendor_id": "sma-hycon-1",
        "device_model": "gen_control_sma_hycon",
        "reading_type": "sma_hycon_csv"
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
    "read_roundtime": true,
    "calc_vendor_id": "_asset"
}
"#;

pub const INVALID_PAYLOAD_1: &str = r#"
{
    "name": "Basic config",
    "devices": {
      "logger": {
        "name": "Logger",
        "driver": "sys_generic",
        "enabled": true,
        "device_model": "gateway_ammp",
        "vendor_id": "strato-1",
        "reading_type": "NOT VALID"
      }
    },
    "readings": {
      "comms_lggr_boot_time": {"device": "logger", "var": "boot_time"},
    },
    "timestamp": "2022-08-15T13:03:17Z",
    "read_interval": 15,
    "read_roundtime": true
}
"#;

pub const INVALID_JSON: &str = "blah";

pub const STATUS_INFO_TEST_CONFIG: &str = r#"
{
  "name": "Status info test config",
  "devices": {
    "ems": {
      "name": "EMS Test Device",
      "driver": "ems_test",
      "enabled": true,
      "vendor_id": "ems-test-1",
      "reading_type": "modbustcp",
      "address": {
        "host": "localhost",
        "port": 502,
        "unit_id": 1
      }
    }
  },
  "drivers": {
    "ems_test": {
      "common": {
        "order": "lsr",
        "fncode": 3,
        "words": 2,
        "datatype": "float"
      },
      "status_info_common": {
        "fncode": 3,
        "words": 1,
        "bit_order": "lsb",
        "length_bits": 1,
        "status_level_value_map": [[0, 0], [1, 3]]
      },
      "fields": {
        "P_L1": {
          "unit": "W",
          "register": 122,
          "description": "Phase 1 active power"
        },
        "P_L2": {
          "unit": "W",
          "register": 124,
          "description": "Phase 2 active power"
        },
        "P_L3": {
          "unit": "W",
          "register": 126,
          "description": "Phase 3 active power"
        }
      },
      "status_info": {
        "relay_fault": {
          "content": "Relay Fault Detected",
          "register": 200,
          "start_bit": 2
        },
        "high_temp_warning": {
          "content": "High Temperature Warning",
          "register": 202,
          "start_bit": 9,
          "status_level_value_map": [[0, 0], [1, 2]]
        },
        "alarm_state": {
          "content": "System Alarm",
          "register": 201,
          "start_bit": 4,
          "length_bits": 4,
          "bit_order": "msb"
        }
      }
    }
  },
  "readings": {
    "ems_P_L1": {
      "device": "ems",
      "var": "P_1"
    },
    "ems_P_L2": {
      "device": "ems",
      "var": "P_2"
    },
    "ems_P_L3": {
      "device": "ems",
      "var": "P_3"
    }
  },
  "status_readings": [
    {
      "d": "ems",
      "r": "relay_fault"
    },
    {
      "d": "ems",
      "r": "high_temp_warning"
    },
    {
      "d": "ems",
      "r": "alarm_state"
    }
  ],
  "read_interval": 10
}
"#;
