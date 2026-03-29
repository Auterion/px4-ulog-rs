#![allow(dead_code)]
/// Helper module for building synthetic ULog byte streams in tests.
///
/// ULog binary format: 16-byte header, then messages each with 3-byte header (u16 size + u8 type).
pub struct ULogBuilder {
    buf: Vec<u8>,
}

impl ULogBuilder {
    /// Create a new builder with a valid 16-byte ULog header.
    /// Default timestamp: 112500176 (matches sample.ulg).
    pub fn new() -> Self {
        let mut buf = Vec::new();
        // Magic bytes
        buf.extend_from_slice(&[0x55, 0x4c, 0x6f, 0x67, 0x01, 0x12, 0x35]);
        // Version
        buf.push(0x01);
        // Timestamp (112500176 in LE)
        buf.extend_from_slice(&112500176u64.to_le_bytes());
        ULogBuilder { buf }
    }

    /// Create a builder with a custom timestamp.
    pub fn with_timestamp(timestamp: u64) -> Self {
        let mut b = Self::new();
        b.buf[8..16].copy_from_slice(&timestamp.to_le_bytes());
        b
    }

    /// Write a raw message (3-byte header + payload).
    fn write_message(&mut self, msg_type: u8, payload: &[u8]) -> &mut Self {
        let msg_size = payload.len() as u16;
        self.buf.extend_from_slice(&msg_size.to_le_bytes());
        self.buf.push(msg_type);
        self.buf.extend_from_slice(payload);
        self
    }

    /// Write a minimal valid Flag Bits ('B') message (40 bytes payload).
    pub fn flag_bits(&mut self) -> &mut Self {
        self.write_message(b'B', &[0u8; 40])
    }

    /// Write a Flag Bits message with custom incompat flags.
    pub fn flag_bits_with_incompat(&mut self, incompat: &[u8; 8]) -> &mut Self {
        let mut payload = [0u8; 40];
        payload[8..16].copy_from_slice(incompat);
        self.write_message(b'B', &payload)
    }

    /// Write a Flag Bits message with appended data offset.
    pub fn flag_bits_with_appended(&mut self, offset0: u64) -> &mut Self {
        let mut payload = [0u8; 40];
        // Set DATA_APPENDED incompat flag
        payload[8] = 0x01;
        payload[16..24].copy_from_slice(&offset0.to_le_bytes());
        self.write_message(b'B', &payload)
    }

    /// Write a Format ('F') definition message.
    /// `fields` is a slice of (type_str, field_name) pairs.
    pub fn format(&mut self, name: &str, fields: &[(&str, &str)]) -> &mut Self {
        let field_str: String = fields
            .iter()
            .map(|(t, n)| format!("{} {}", t, n))
            .collect::<Vec<_>>()
            .join(";");
        let payload = format!("{}:{}", name, field_str);
        self.write_message(b'F', payload.as_bytes())
    }

    /// Write an Add Logged Message ('A') subscription.
    pub fn add_logged(&mut self, msg_id: u16, multi_id: u8, name: &str) -> &mut Self {
        let mut payload = Vec::new();
        payload.push(multi_id);
        payload.extend_from_slice(&msg_id.to_le_bytes());
        payload.extend_from_slice(name.as_bytes());
        self.write_message(b'A', &payload)
    }

    /// Write a Data ('D') message with raw payload (must include msg_id as first 2 bytes).
    pub fn data_raw(&mut self, payload: &[u8]) -> &mut Self {
        self.write_message(b'D', payload)
    }

    /// Write a Data ('D') message for a given msg_id with typed field data.
    /// `field_data` should be the raw bytes of the fields (excluding msg_id).
    pub fn data(&mut self, msg_id: u16, field_data: &[u8]) -> &mut Self {
        let mut payload = Vec::new();
        payload.extend_from_slice(&msg_id.to_le_bytes());
        payload.extend_from_slice(field_data);
        self.write_message(b'D', &payload)
    }

    /// Write a Parameter ('P') message with an int32 value.
    pub fn parameter_i32(&mut self, name: &str, value: i32) -> &mut Self {
        let key = format!("int32_t {}", name);
        let mut payload = Vec::new();
        payload.push(key.len() as u8);
        payload.extend_from_slice(key.as_bytes());
        payload.extend_from_slice(&value.to_le_bytes());
        self.write_message(b'P', &payload)
    }

    /// Write a Parameter ('P') message with a float value.
    pub fn parameter_f32(&mut self, name: &str, value: f32) -> &mut Self {
        let key = format!("float {}", name);
        let mut payload = Vec::new();
        payload.push(key.len() as u8);
        payload.extend_from_slice(key.as_bytes());
        payload.extend_from_slice(&value.to_le_bytes());
        self.write_message(b'P', &payload)
    }

    /// Write a Logged String ('L') message.
    pub fn logged_string(&mut self, level: u8, timestamp: u64, msg: &str) -> &mut Self {
        let mut payload = Vec::new();
        payload.push(level);
        payload.extend_from_slice(&timestamp.to_le_bytes());
        payload.extend_from_slice(msg.as_bytes());
        self.write_message(b'L', &payload)
    }

    /// Write an Info ('I') message.
    pub fn info(&mut self, key_type: &str, key_name: &str, value: &[u8]) -> &mut Self {
        let key = format!("{}[{}] {}", key_type, value.len(), key_name);
        let mut payload = Vec::new();
        payload.push(key.len() as u8);
        payload.extend_from_slice(key.as_bytes());
        payload.extend_from_slice(value);
        self.write_message(b'I', &payload)
    }

    /// Write a MultiInfo ('M') message.
    /// `is_continued` indicates whether more fragments follow for this key.
    pub fn multi_info(
        &mut self,
        is_continued: bool,
        key_type: &str,
        key_name: &str,
        value: &[u8],
    ) -> &mut Self {
        let key = format!("{}[{}] {}", key_type, value.len(), key_name);
        let mut payload = Vec::new();
        payload.push(if is_continued { 1u8 } else { 0u8 });
        payload.push(key.len() as u8);
        payload.extend_from_slice(key.as_bytes());
        payload.extend_from_slice(value);
        self.write_message(b'M', &payload)
    }

    /// Write a Dropout ('O') message.
    pub fn dropout(&mut self, duration_ms: u16) -> &mut Self {
        self.write_message(b'O', &duration_ms.to_le_bytes())
    }

    /// Write a Sync ('S') message with the standard magic bytes.
    pub fn sync(&mut self) -> &mut Self {
        self.write_message(b'S', &[0x2F, 0x73, 0x13, 0x20, 0x25, 0x0C, 0xBB, 0x12])
    }

    /// Write a Tagged Logged String ('C') message.
    pub fn tagged_logged_string(
        &mut self,
        level: u8,
        tag: u16,
        timestamp: u64,
        msg: &str,
    ) -> &mut Self {
        let mut payload = Vec::new();
        payload.push(level);
        payload.extend_from_slice(&tag.to_le_bytes());
        payload.extend_from_slice(&timestamp.to_le_bytes());
        payload.extend_from_slice(msg.as_bytes());
        self.write_message(b'C', &payload)
    }

    /// Write a Remove Logged Message ('R') message.
    pub fn remove_logged(&mut self, msg_id: u16) -> &mut Self {
        self.write_message(b'R', &msg_id.to_le_bytes())
    }

    /// Write a Parameter Default ('Q') message.
    pub fn parameter_default_i32(
        &mut self,
        default_types: u8,
        name: &str,
        value: i32,
    ) -> &mut Self {
        let key = format!("int32_t {}", name);
        let mut payload = Vec::new();
        payload.push(default_types);
        payload.push(key.len() as u8);
        payload.extend_from_slice(key.as_bytes());
        payload.extend_from_slice(&value.to_le_bytes());
        self.write_message(b'Q', &payload)
    }

    /// Write a message with an arbitrary/unknown type byte.
    pub fn unknown_message(&mut self, msg_type: u8, payload: &[u8]) -> &mut Self {
        self.write_message(msg_type, payload)
    }

    /// Build a minimal valid ULog stream: header + flag_bits + one format + one subscription + one data message.
    /// Returns (builder, msg_id) for further chaining.
    pub fn minimal_with_data() -> (Self, u16) {
        let mut b = Self::new();
        let msg_id = 0u16;
        let timestamp_val = 1000u64;
        let x_val = 1.5f32;

        b.flag_bits()
            .format("test_topic", &[("uint64_t", "timestamp"), ("float", "x")])
            .add_logged(msg_id, 0, "test_topic");

        // Build data payload: timestamp (8 bytes) + x (4 bytes)
        let mut data_payload = Vec::new();
        data_payload.extend_from_slice(&timestamp_val.to_le_bytes());
        data_payload.extend_from_slice(&x_val.to_le_bytes());
        b.data(msg_id, &data_payload);

        (b, msg_id)
    }

    /// Get the built bytes.
    pub fn build(&self) -> Vec<u8> {
        self.buf.clone()
    }

    /// Get current byte length.
    pub fn len(&self) -> usize {
        self.buf.len()
    }
}
