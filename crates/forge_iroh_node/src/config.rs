use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the iroh P2P node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrohConfig {
    /// Enable P2P functionality
    pub enabled: bool,
    /// Default topics to join on startup
    pub default_topics: Vec<String>,
    /// Node nickname/identifier
    pub node_name: String,
    /// Storage path for iroh data
    pub storage_path: Option<PathBuf>,
    /// Secret key for the node (optional, will generate if not provided)
    pub secret_key: Option<String>,
    /// Relay nodes to use for discovery
    pub relay_nodes: Vec<String>,
    /// Auto-execute commands received from P2P
    pub auto_execute_commands: bool,
    /// Maximum message size in bytes
    pub max_message_size: usize,
}

impl Default for IrohConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_topics: vec!["forge-general".to_string()],
            node_name: whoami::username(),
            storage_path: None,
            secret_key: None,
            relay_nodes: vec![],
            auto_execute_commands: false,
            max_message_size: 1024 * 1024, // 1MB
        }
    }
}

impl IrohConfig {
    /// Create a new config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable P2P functionality
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set default topics
    pub fn with_topics(mut self, topics: Vec<String>) -> Self {
        self.default_topics = topics;
        self
    }

    /// Set node name
    pub fn with_name(mut self, name: String) -> Self {
        self.node_name = name;
        self
    }

    /// Set storage path
    pub fn with_storage_path(mut self, path: PathBuf) -> Self {
        self.storage_path = Some(path);
        self
    }

    /// Enable auto-execution of P2P commands
    pub fn auto_execute(mut self, auto_execute: bool) -> Self {
        self.auto_execute_commands = auto_execute;
        self
    }

    /// Set maximum message size
    pub fn with_max_message_size(mut self, size: usize) -> Self {
        self.max_message_size = size;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.node_name.is_empty() {
            return Err("Node name cannot be empty".to_string());
        }

        if self.max_message_size == 0 {
            return Err("Maximum message size must be greater than 0".to_string());
        }

        if self.max_message_size > 10 * 1024 * 1024 {
            return Err("Maximum message size cannot exceed 10MB".to_string());
        }

        Ok(())
    }
}

