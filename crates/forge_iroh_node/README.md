# Forge Iroh Node

A P2P networking module for Forge that integrates with the iroh library to enable peer-to-peer communication and command execution.

## Features

- **P2P Node Creation**: Create and manage iroh nodes with automatic discovery
- **Gossip Protocol**: Join topics and exchange messages using the gossip protocol
- **Message Types**: Support for command, chat, and status messages
- **Forge Integration**: Forward P2P commands to the forge runtime for execution
- **Configuration**: Flexible configuration for node behavior and topics
- **Error Handling**: Comprehensive error handling and logging

## Usage

### Basic Setup

```rust
use forge_iroh_node::{ForgeIrohNode, P2PMessageHandler, IrohConfig};

// Create configuration
let config = IrohConfig::new()
    .enabled(true)
    .with_topics(vec!["my-topic".to_string()])
    .with_name("my-node".to_string())
    .auto_execute(true);

// Create and initialize node
let (mut node, message_rx) = ForgeIrohNode::new();
node.init().await?;

// Join a topic
let topic_id = node.join_topic("my-topic").await?;
```

### Message Handling

```rust
// Create P2P message handler with forge services
let mut handler = P2PMessageHandler::new(services);
handler.init().await?;
handler.start_listening().await;

// Join topics
handler.join_topic("forge-commands").await?;
```

### Sending Messages

```rust
// Send a command
node.send_command(topic_id, "ls -la", "user1").await?;

// Send a chat message
node.send_chat(topic_id, "Hello, world!", "user1").await?;
```

## Message Types

- **Command**: Execute shell commands through forge
- **Chat**: Simple text messages for communication
- **Status**: Node status updates and heartbeats

## Configuration

The `IrohConfig` struct provides configuration options:

- `enabled`: Enable/disable P2P functionality
- `default_topics`: Topics to join on startup
- `node_name`: Identifier for the node
- `storage_path`: Path for iroh data storage
- `auto_execute_commands`: Whether to automatically execute received commands
- `max_message_size`: Maximum message size in bytes

## Security Considerations

- P2P command execution should be used with caution
- Consider implementing authentication and authorization
- Validate commands before execution
- Monitor for malicious activity

## Dependencies

- `iroh`: Core iroh library for P2P networking
- `iroh-gossip`: Gossip protocol implementation
- `forge_app`: Integration with forge services
- `tokio`: Async runtime support