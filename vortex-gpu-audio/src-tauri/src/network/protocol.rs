use serde::{Serialize, Deserialize};

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageType {
    Spectrum,
    Waveform,
    VuMeter,
    SystemStatus,
    Control,
}

/// Protocol message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub timestamp: u64,
    pub message_type: MessageType,
    pub data: Vec<u8>,
}

impl ProtocolMessage {
    /// Create a new protocol message
    pub fn new(message_type: MessageType, data: Vec<u8>) -> Self {
        Self {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            message_type,
            data,
        }
    }
}
