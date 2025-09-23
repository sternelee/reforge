use forge_domain::{ToolCallArgumentError, ToolName};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid tool call arguments: {0}")]
    CallArgument(ToolCallArgumentError),

    #[error("Tool {0} not found")]
    NotFound(ToolName),

    #[error("Tool '{tool_name}' timed out after {timeout} minutes")]
    CallTimeout { tool_name: ToolName, timeout: u64 },

    #[error(
        "Tool '{name}' is not available. Please try again with one of these tools: [{supported_tools}]"
    )]
    NotAllowed {
        name: ToolName,
        supported_tools: String,
    },

    #[error("Empty tool response")]
    EmptyToolResponse,

    #[error("Authentication still in progress")]
    AuthInProgress,

    #[error("No agent has subscribed to the event: {0}")]
    UnsubscribedEvent(String),
}
