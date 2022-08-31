class MockResponses:
    SLAVE_IDS = [3]
    REGISTER_MAP = {
        30051: 0,  # Device class (2 registers); 8081 = Solar Inverters
        30052: 8081,
        30053: 0,  # Device type (2 registers); 9197 = STP 24000TL-US-10
        30054: 9197,
        30057: 18838,  # Serial number (2 registers); 1234567890
        30058: 722,
        30775: 0,  # Total output power (int32); 12,345 W
        30776: 12345,
    }
    DEFAULT_RESPONSE = 0
