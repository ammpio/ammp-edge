class EmsMockResponses:
    SLAVE_ID = 1
    REGISTER_MAP = {
        122: 0xcc5a,  # 32-bit float with LSR -> 325218.8125
        123: 0x489e,
        124: 0xf330,
        125: 0x48aa,
        126: 0xe441,
        127: 0x489f,
    }
    DEFAULT_RESPONSE = 0


class SmaStpMockResponses:
    SLAVE_ID = 3
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
