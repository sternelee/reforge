use serde::{Deserialize, Serialize};

/// Message types that can be sent over P2P network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum P2PMessage {
    /// Command message to be forwarded to forge
    Command {
        command: String,
        sender: String,
        timestamp: u64,
    },
    /// Chat message for communication
    Chat {
        message: String,
        sender: String,
        timestamp: u64,
    },
    /// Status update message
    Status {
        status: String,
        node_id: String,
        timestamp: u64,
    },
}

impl P2PMessage {
    pub fn new_command(command: String, sender: String) -> Self {
        Self::Command {
            command,
            sender,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn new_chat(message: String, sender: String) -> Self {
        Self::Chat {
            message,
            sender,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn new_status(status: String, node_id: String) -> Self {
        Self::Status {
            status,
            node_id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

