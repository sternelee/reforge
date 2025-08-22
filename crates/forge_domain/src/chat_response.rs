use std::time::Duration;

use crate::{Metrics, ToolCallFull, ToolResult, Usage};
use serde::Serialize;

/// Events that are emitted by the agent for external consumption. This includes
/// events for all internal state changes.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ChatResponse {
    TaskMessage { text: String, is_md: bool },
    TaskReasoning { content: String },
    TaskComplete { metrics: Metrics },
    ToolCallStart(ToolCallFull),
    ToolCallEnd(ToolResult),
    Usage(Usage),
    RetryAttempt { cause: Cause, duration: Duration },
    Interrupt { reason: InterruptionReason },
}

#[derive(Debug, Clone, Serialize)]
pub enum InterruptionReason {
    MaxToolFailurePerTurnLimitReached { limit: u64 },
    MaxRequestPerTurnLimitReached { limit: u64 },
}

#[derive(Clone, Serialize)]
pub struct Cause(String);

impl Cause {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Debug for Cause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl From<&anyhow::Error> for Cause {
    fn from(value: &anyhow::Error) -> Self {
        Self(format!("{value:?}"))
    }
}
