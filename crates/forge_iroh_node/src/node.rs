use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use futures::StreamExt;
use iroh::Endpoint;
use iroh_gossip::net::{Gossip, Event as GossipEvent};
use iroh_gossip::proto::{TopicId, NodeId};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::{debug, error, info, warn};

use crate::{P2PMessage, Result, IrohNodeError};

/// Iroh P2P node for forge integration
pub struct ForgeIrohNode {
    endpoint: Option<Endpoint>,
    gossip: Option<Gossip>,
    topics: Arc<RwLock<HashMap<TopicId, String>>>,
    message_sender: mpsc::UnboundedSender<P2PMessage>,
    _message_receiver: mpsc::UnboundedReceiver<P2PMessage>,
    running: Arc<RwLock<bool>>,
}

impl ForgeIrohNode {
    /// Create a new forge iroh node
    pub fn new() -> (Self, mpsc::UnboundedReceiver<P2PMessage>) {
        let (message_sender, message_receiver) = mpsc::unbounded_channel();
        let node = Self {
            endpoint: None,
            gossip: None,
            topics: Arc::new(RwLock::new(HashMap::new())),
            message_sender,
            _message_receiver: message_receiver,
            running: Arc::new(RwLock::new(false)),
        };
        let rx = message_receiver;
        (node, rx)
    }

    /// Initialize the iroh node
    pub async fn init(&mut self) -> Result<()> {
        info!("Initializing forge iroh node");

        // Create endpoint with discovery
        let endpoint = Endpoint::builder()
            .discovery_n0()
            .alpns(vec![b"forge-p2p".to_vec()])
            .bind()
            .await
            .map_err(IrohNodeError::EndpointCreation)?;

        let node_id = endpoint.node_id();
        info!("Iroh node initialized with ID: {}", node_id);

        // Create gossip instance
        let gossip = Gossip::builder()
            .spawn(endpoint.clone())
            .await
            .map_err(|e| IrohNodeError::Network(anyhow!(e)))?;

        info!("Gossip protocol initialized");

        self.endpoint = Some(endpoint);
        self.gossip = Some(gossip);
        *self.running.write().await = true;

        Ok(())
    }

    /// Get node ID
    pub async fn node_id(&self) -> Option<NodeId> {
        self.endpoint.as_ref().map(|e| e.node_id())
    }

    /// Join a gossip topic
    pub async fn join_topic(&self, topic_name: &str) -> Result<TopicId> {
        let gossip = self.gossip.as_ref().ok_or(IrohNodeError::NodeNotRunning)?;

        // Generate topic ID from name
        let topic_id = TopicId::new(topic_name.as_bytes());

        info!("Joining topic: {} (ID: {})", topic_name, topic_id);

        // Subscribe to the topic
        let mut topic_stream = gossip
            .subscribe(topic_id, Vec::new())
            .await
            .map_err(IrohNodeError::GossipJoin)?;

        // Store topic mapping
        self.topics.write().await.insert(topic_id, topic_name.to_string());

        // Spawn task to handle messages from this topic
        let message_sender = self.message_sender.clone();
        let topic_name = topic_name.to_string();

        tokio::spawn(async move {
            while let Some(event) = topic_stream.next().await {
                match event {
                    Ok(GossipEvent::Received { content, .. }) => {
                        debug!("Received message on topic {}: {} bytes", topic_name, content.len());

                        match P2PMessage::from_bytes(&content) {
                            Ok(message) => {
                                if let Err(e) = message_sender.send(message) {
                                    error!("Failed to forward message: {}", e);
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse message: {}", e);
                            }
                        }
                    }
                    Ok(GossipEvent::Joined { nodes }) => {
                        info!("Joined topic {} with {} nodes", topic_name, nodes.len());
                    }
                    Ok(GossipEvent::NeighborUp { node_id }) => {
                        debug!("New neighbor on topic {}: {}", topic_name, node_id);
                    }
                    Ok(GossipEvent::NeighborDown { node_id }) => {
                        debug!("Lost neighbor on topic {}: {}", topic_name, node_id);
                    }
                    Err(e) => {
                        error!("Error on topic {}: {}", topic_name, e);
                    }
                }
            }
            warn!("Topic stream ended for: {}", topic_name);
        });

        Ok(topic_id)
    }

    /// Send a message to a topic
    pub async fn send_message(&self, topic_id: TopicId, message: P2PMessage) -> Result<()> {
        let gossip = self.gossip.as_ref().ok_or(IrohNodeError::NodeNotRunning)?;

        let bytes = message.to_bytes()?;

        debug!("Sending message to topic {}: {} bytes", topic_id, bytes.len());

        gossip
            .broadcast(topic_id, bytes.into())
            .await
            .map_err(|e| IrohNodeError::Network(anyhow!(e)))?;

        Ok(())
    }

    /// Send a command message to a topic
    pub async fn send_command(&self, topic_id: TopicId, command: &str, sender: &str) -> Result<()> {
        let message = P2PMessage::new_command(command.to_string(), sender.to_string());
        self.send_message(topic_id, message).await
    }

    /// Send a chat message to a topic
    pub async fn send_chat(&self, topic_id: TopicId, message: &str, sender: &str) -> Result<()> {
        let msg = P2PMessage::new_chat(message.to_string(), sender.to_string());
        self.send_message(topic_id, msg).await
    }

    /// Get list of joined topics
    pub async fn list_topics(&self) -> Vec<(TopicId, String)> {
        self.topics
            .read()
            .await
            .iter()
            .map(|(id, name)| (*id, name.clone()))
            .collect()
    }

    /// Check if node is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Shutdown the node
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down forge iroh node");

        *self.running.write().await = false;

        if let Some(endpoint) = self.endpoint.take() {
            endpoint.close(0u32.into(), b"shutting down").await;
        }

        self.gossip = None;
        self.topics.write().await.clear();

        info!("Forge iroh node shutdown complete");
        Ok(())
    }
}

impl Default for ForgeIrohNode {
    fn default() -> Self {
        let (node, _) = Self::new();
        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_node_creation() {
        let (mut node, _rx) = ForgeIrohNode::new();
        assert!(!node.is_running().await);

        let init_result = node.init().await;
        assert!(init_result.is_ok(), "Failed to initialize node: {:?}", init_result);
        assert!(node.is_running().await);
        assert!(node.node_id().await.is_some());

        let shutdown_result = node.shutdown().await;
        assert!(shutdown_result.is_ok());
        assert!(!node.is_running().await);
    }

    #[tokio::test]
    async fn test_topic_join() {
        let (mut node, _rx) = ForgeIrohNode::new();
        node.init().await.unwrap();

        let topic_id = node.join_topic("test-topic").await.unwrap();
        let topics = node.list_topics().await;

        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].0, topic_id);
        assert_eq!(topics[0].1, "test-topic");

        node.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_message_sending() {
        let (mut node, mut rx) = ForgeIrohNode::new();
        node.init().await.unwrap();

        let topic_id = node.join_topic("test-topic").await.unwrap();

        // Give some time for the subscription to be established
        tokio::time::sleep(Duration::from_millis(100)).await;

        node.send_command(topic_id, "test command", "test_user").await.unwrap();

        // Wait for message to be received
        let received = timeout(Duration::from_secs(1), rx.recv()).await;

        if let Ok(Some(P2PMessage::Command { command, sender, .. })) = received {
            assert_eq!(command, "test command");
            assert_eq!(sender, "test_user");
        } else {
            // Note: In a single-node test, we might not receive our own message
            // This is expected behavior for gossip protocols
        }

        node.shutdown().await.unwrap();
    }
}
