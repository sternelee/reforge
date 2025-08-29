use std::time::Duration;

use crate::{ToolCallFull, ToolResult, Usage};

#[derive(Debug, Clone, PartialEq)]
pub enum ChatResponseContent {
    Title(TitleFormat),
    PlainText(String),
    Markdown(String),
}

impl From<ChatResponseContent> for ChatResponse {
    fn from(content: ChatResponseContent) -> Self {
        ChatResponse::TaskMessage { content }
    }
}

impl From<TitleFormat> for ChatResponse {
    fn from(title: TitleFormat) -> Self {
        ChatResponse::TaskMessage { content: ChatResponseContent::Title(title) }
    }
}

impl From<TitleFormat> for ChatResponseContent {
    fn from(title: TitleFormat) -> Self {
        ChatResponseContent::Title(title)
    }
}
impl ChatResponseContent {
    pub fn contains(&self, needle: &str) -> bool {
        self.as_str().contains(needle)
    }

    pub fn as_str(&self) -> &str {
        match self {
            ChatResponseContent::PlainText(text) | ChatResponseContent::Markdown(text) => text,
            ChatResponseContent::Title(_) => "",
        }
    }
}

/// Events that are emitted by the agent for external consumption. This includes
/// events for all internal state changes.
#[derive(Debug, Clone)]
pub enum ChatResponse {
    TaskMessage { content: ChatResponseContent },
    TaskReasoning { content: String },
    TaskComplete,
    ToolCallStart(ToolCallFull),
    ToolCallEnd(ToolResult),
    Usage(Usage),
    RetryAttempt { cause: Cause, duration: Duration },
    Interrupt { reason: InterruptionReason },
}

#[derive(Debug, Clone)]
pub enum InterruptionReason {
    MaxToolFailurePerTurnLimitReached { limit: u64 },
    MaxRequestPerTurnLimitReached { limit: u64 },
}

#[derive(Clone)]
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

#[derive(Clone, Debug, PartialEq)]
pub enum Category {
    Action,
    Info,
    Debug,
    Error,
    Completion,
}

#[derive(Clone, derive_setters::Setters, Debug, PartialEq)]
#[setters(into, strip_option)]
pub struct TitleFormat {
    pub title: String,
    pub sub_title: Option<String>,
    pub category: Category,
}

pub trait TitleExt {
    fn title_fmt(&self) -> TitleFormat;
}

impl<T> TitleExt for T
where
    T: Into<TitleFormat> + Clone,
{
    fn title_fmt(&self) -> TitleFormat {
        self.clone().into()
    }
}

impl TitleFormat {
    /// Create a status for executing a tool
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            title: message.into(),
            sub_title: None,
            category: Category::Info,
        }
    }

    /// Create a status for executing a tool
    pub fn action(message: impl Into<String>) -> Self {
        Self {
            title: message.into(),
            sub_title: None,
            category: Category::Action,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            title: message.into(),
            sub_title: None,
            category: Category::Error,
        }
    }

    pub fn debug(message: impl Into<String>) -> Self {
        Self {
            title: message.into(),
            sub_title: None,
            category: Category::Debug,
        }
    }

    pub fn completion(message: impl Into<String>) -> Self {
        Self {
            title: message.into(),
            sub_title: None,
            category: Category::Completion,
        }
    }
}
