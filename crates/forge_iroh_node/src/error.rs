use thiserror::Error;

#[derive(Error, Debug)]
pub enum IrohNodeError {
    #[error("Failed to create iroh endpoint: {0}")]
    EndpointCreation(anyhow::Error),

    #[error("Failed to join gossip topic: {0}")]
    GossipJoin(anyhow::Error),

    #[error("Message serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Network error: {0}")]
    Network(#[from] anyhow::Error),

    #[error("Node is not running")]
    NodeNotRunning,

    #[error("Topic not joined")]
    TopicNotJoined,
}

pub type Result<T> = std::result::Result<T, IrohNodeError>;
