use std::sync::Arc;

use anyhow::Result;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

use crate::{ForgeIrohNode, P2PMessage};
use forge_app::{Services, ShellService};

/// P2P message handler that integrates with forge runtime
pub struct P2PMessageHandler<S> {
    services: Arc<S>,
    node: Arc<RwLock<Option<ForgeIrohNode>>>,
    message_receiver: Option<mpsc::UnboundedReceiver<P2PMessage>>,
}

impl<S: Services> P2PMessageHandler<S> {
    /// Create a new P2P message handler
    pub fn new(services: Arc<S>) -> Self {
        let (node, message_receiver) = ForgeIrohNode::new();

        Self {
            services,
            node: Arc::new(RwLock::new(Some(node))),
            message_receiver: Some(message_receiver),
        }
    }

    /// Initialize the P2P node
    pub async fn init(&mut self) -> Result<()> {
        let mut node_guard = self.node.write().await;
        if let Some(ref mut node) = node_guard.as_mut() {
            node.init().await?;
            info!("P2P message handler initialized");
        }
        Ok(())
    }

    /// Start listening for P2P messages and process them
    pub async fn start_listening(&mut self) {
        if let Some(mut receiver) = self.message_receiver.take() {
            let services = self.services.clone();

            tokio::spawn(async move {
                info!("Started P2P message listening");

                while let Some(message) = receiver.recv().await {
                    match Self::process_message(&services, message).await {
                        Ok(_) => debug!("Successfully processed P2P message"),
                        Err(e) => error!("Failed to process P2P message: {}", e),
                    }
                }

                warn!("P2P message listening stopped");
            });
        }
    }

    /// Process a P2P message and forward it to forge
    async fn process_message(services: &Arc<S>, message: P2PMessage) -> Result<()> {
        match message {
            P2PMessage::Command { command, sender, timestamp: _ } => {
                info!("Received P2P command from {}: {}", sender, command);

                // Execute the command through shell service
                let shell_service = services.shell_service();
                let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

                match shell_service.execute(command.clone(), cwd, false).await {
                    Ok(output) => {
                        info!(
                            "P2P command executed successfully: stdout: {}, stderr: {}",
                            output.output.stdout, output.output.stderr
                        );
                        // TODO: Optionally send result back to P2P network
                    }
                    Err(e) => {
                        error!("Failed to execute P2P command '{}': {}", command, e);
                    }
                }
            }
            P2PMessage::Chat { message, sender, timestamp: _ } => {
                info!("Received P2P chat message from {}: {}", sender, message);

                // For chat messages, we could create a new conversation or add to existing one
                // This is a simplified implementation - you might want to expand this
                debug!(
                    "Chat message logged from P2P network: {} ({})",
                    message, sender
                );
            }
            P2PMessage::Status { status, node_id, timestamp: _ } => {
                debug!("Received P2P status update from {}: {}", node_id, status);
            }
        }

        Ok(())
    }

    /// Get the underlying iroh node for direct access
    pub async fn get_node(&self) -> Option<Arc<RwLock<Option<ForgeIrohNode>>>> {
        Some(self.node.clone())
    }

    /// Join a P2P topic
    pub async fn join_topic(&self, topic_name: &str) -> Result<()> {
        let node_guard = self.node.read().await;
        if let Some(ref node) = node_guard.as_ref() {
            let topic_id = node.join_topic(topic_name).await?;
            info!("Joined P2P topic: {} (ID: {})", topic_name, topic_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("P2P node not initialized"))
        }
    }

    /// Send a command through P2P
    pub async fn send_p2p_command(
        &self,
        topic_name: &str,
        command: &str,
        sender: &str,
    ) -> Result<()> {
        let node_guard = self.node.read().await;
        if let Some(ref node) = node_guard.as_ref() {
            // Find topic ID by name
            let topics = node.list_topics().await;
            if let Some((topic_id, _)) = topics.iter().find(|(_, name)| name == topic_name) {
                node.send_command(*topic_id, command, sender).await?;
                info!("Sent P2P command to topic {}: {}", topic_name, command);
            } else {
                return Err(anyhow::anyhow!("Topic not found: {}", topic_name));
            }
        } else {
            return Err(anyhow::anyhow!("P2P node not initialized"));
        }
        Ok(())
    }

    /// Shutdown the P2P handler
    pub async fn shutdown(&self) -> Result<()> {
        let mut node_guard = self.node.write().await;
        if let Some(mut node) = node_guard.take() {
            node.shutdown().await?;
            info!("P2P message handler shutdown");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // Mock services for testing - you would need to implement these properly
    struct MockServices;

    // You would need to implement the Services trait for MockServices
    // This is just a placeholder to show the structure

    #[tokio::test]
    async fn test_p2p_handler_creation() {
        // This test would require proper mock implementations
        // Just showing the structure for now
    }
}
