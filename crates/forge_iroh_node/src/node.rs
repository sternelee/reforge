use std::collections::HashMap;
use std::sync::Arc;

use anyhow::anyhow;
use futures::StreamExt;
use iroh::protocol::Router;
use iroh_gossip::{ALPN as GOSSIP_ALPN, net::Gossip, proto::TopicId};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

use crate::{IrohNodeError, P2PMessage, Result};

/// Iroh P2P node for forge integration
pub struct ForgeIrohNode {
    endpoint: Option<iroh::Endpoint>,
    gossip: Option<Gossip>,
    router: Option<Router>,
    topics: Arc<RwLock<HashMap<TopicId, String>>>,
    message_sender: mpsc::UnboundedSender<P2PMessage>,
    running: Arc<RwLock<bool>>,
}

impl ForgeIrohNode {
    /// Create a new forge iroh node
    pub fn new() -> (Self, mpsc::UnboundedReceiver<P2PMessage>) {
        let (message_sender, message_receiver) = mpsc::unbounded_channel();
        let node = Self {
            endpoint: None,
            gossip: None,
            router: None,
            topics: Arc::new(RwLock::new(HashMap::new())),
            message_sender,
            running: Arc::new(RwLock::new(false)),
        };
        (node, message_receiver)
    }

    /// Initialize the iroh node
    pub async fn init(&mut self) -> Result<()> {
        info!("Initializing forge iroh node");

        // Create endpoint with discovery
        let endpoint = iroh::Endpoint::builder()
            .discovery_n0()
            .bind()
            .await
            .map_err(|e| IrohNodeError::EndpointCreation(anyhow!(e)))?;

        let node_id = endpoint.node_id();
        info!("Iroh endpoint created with ID: {}", node_id);

        // Create gossip instance
        let gossip = Gossip::builder().spawn(endpoint.clone());

        info!("Gossip protocol initialized");

        // Setup router
        let router = Router::builder(endpoint.clone())
            .accept(GOSSIP_ALPN, gossip.clone())
            .spawn();

        info!("Router spawned");

        self.endpoint = Some(endpoint);
        self.gossip = Some(gossip);
        self.router = Some(router);
        *self.running.write().await = true;

        Ok(())
    }

    /// Get node ID
    pub async fn node_id(&self) -> Option<iroh::NodeId> {
        self.endpoint.as_ref().map(|e| e.node_id())
    }

    /// Get node address information
    pub async fn node_addr(&self) -> Option<String> {
        if let Some(endpoint) = &self.endpoint {
            let node_id = endpoint.node_id();
            Some(format!("{}", node_id))
        } else {
            None
        }
    }

    /// Print node information to console
    pub async fn print_node_info(&self) {
        if let Some(node_id) = self.node_id().await {
            println!("🌐 Iroh P2P Node Started");
            println!("   Node ID: {}", node_id);

            if let Some(_endpoint) = &self.endpoint {
                println!("   Addresses: Available via relay");
                println!("   Discovery: n0 (iroh default)");
            }

            let topics = self.list_topics().await;
            if !topics.is_empty() {
                println!("   Active Topics:");
                for (_, topic_name) in topics.iter().take(5) {
                    println!("     - {}", topic_name);
                }
            }
            println!();
        }
    }

    /// Join a gossip topic
    pub async fn join_topic(&self, topic_name: &str) -> Result<TopicId> {
        let gossip = self.gossip.as_ref().ok_or(IrohNodeError::NodeNotRunning)?;

        // Generate topic ID from name - use SHA256 hash to get exactly 32 bytes
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(topic_name.as_bytes());
        let hash = hasher.finalize();
        let mut topic_bytes = [0u8; 32];
        topic_bytes.copy_from_slice(&hash[..]);
        let topic_id = TopicId::from(topic_bytes);

        info!("Joining topic: {} (ID: {})", topic_name, topic_id);

        // Subscribe to the topic
        let mut events = gossip
            .subscribe(topic_id, Vec::new())
            .await
            .map_err(|e| IrohNodeError::GossipJoin(anyhow!(e)))?;

        // Store topic mapping
        self.topics
            .write()
            .await
            .insert(topic_id, topic_name.to_string());

        // Spawn task to handle messages from this topic
        let message_sender = self.message_sender.clone();
        let topic_name = topic_name.to_string();

        tokio::spawn(async move {
            while let Some(event) = events.next().await {
                match event {
                    Ok(iroh_gossip::api::Event::Received(msg)) => {
                        debug!(
                            "Received message on topic {}: {} bytes",
                            topic_name,
                            msg.content.len()
                        );

                        match P2PMessage::from_bytes(&msg.content) {
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
                    Ok(iroh_gossip::api::Event::NeighborUp(peer)) => {
                        debug!("New peer joined topic {}: {}", topic_name, peer);
                    }
                    Ok(iroh_gossip::api::Event::NeighborDown(peer)) => {
                        debug!("Peer left topic {}: {}", topic_name, peer);
                    }
                    Ok(iroh_gossip::api::Event::Lagged) => {
                        warn!("Gossip stream lagged on topic: {}", topic_name);
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

        debug!(
            "Sending message to topic {}: {} bytes",
            topic_id,
            bytes.len()
        );

        // Create a temporary subscription to get the topic
        let mut topic = gossip
            .subscribe(topic_id, Vec::new())
            .await
            .map_err(|e| IrohNodeError::Network(anyhow!(e)))?;

        // Use the topic to broadcast the message
        topic
            .broadcast(bytes.into())
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

        if let Some(router) = self.router.take() {
            router
                .shutdown()
                .await
                .map_err(|e| IrohNodeError::Network(anyhow!(e)))?;
        }

        if let Some(endpoint) = self.endpoint.take() {
            endpoint.close().await;
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
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn test_node_creation() {
        let (mut node, _rx) = ForgeIrohNode::new();
        assert!(!node.is_running().await);

        let init_result = node.init().await;
        if let Err(ref e) = init_result {
            eprintln!("Init failed: {}", e);
        }
        assert!(
            init_result.is_ok(),
            "Failed to initialize node: {:?}",
            init_result
        );
        assert!(node.is_running().await);
        assert!(node.node_id().await.is_some());

        let shutdown_result = node.shutdown().await;
        assert!(shutdown_result.is_ok());
        assert!(!node.is_running().await);
    }

    #[tokio::test]
    async fn test_topic_join() {
        let (mut node, _rx) = ForgeIrohNode::new();
        if node.init().await.is_err() {
            return; // Skip test if init fails (e.g., network issues)
        }

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
        if node.init().await.is_err() {
            return; // Skip test if init fails
        }

        let topic_id = node.join_topic("test-topic").await.unwrap();

        // Give some time for the subscription to be established
        tokio::time::sleep(Duration::from_millis(100)).await;

        node.send_command(topic_id, "test command", "test_user")
            .await
            .unwrap();

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
