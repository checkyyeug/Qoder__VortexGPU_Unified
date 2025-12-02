// Network subsystem modules
pub mod discovery;
pub mod websocket;
pub mod output_manager;
pub mod protocol;

pub use discovery::{DeviceDiscovery, NetworkDevice};
pub use websocket::{WebSocketServer, WebSocketMessage};
pub use output_manager::{OutputManager, OutputDevice};
pub use protocol::{ProtocolMessage, MessageType};
