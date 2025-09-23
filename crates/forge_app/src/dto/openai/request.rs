use std::vec;

use derive_more::derive::Display;
use derive_setters::Setters;
use serde::{Deserialize, Serialize};

use super::response::{FunctionCall, ToolCall};
use super::tool_choice::{FunctionType, ToolChoice};
use crate::domain::{
    Context, ContextMessage, ModelId, ToolCallFull, ToolCallId, ToolDefinition, ToolName,
    ToolResult, ToolValue,
};
use crate::dto::openai::ReasoningDetail;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Message {
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<ToolName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<ToolCallId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_details: Option<Vec<ReasoningDetail>>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    pub fn cached(self, enable_cache: bool) -> Self {
        let cache_control =
            enable_cache.then_some(CacheControl { type_: CacheControlType::Ephemeral });

        match self {
            MessageContent::Text(text) => {
                if let Some(cc) = cache_control {
                    MessageContent::Parts(vec![ContentPart::Text { text, cache_control: Some(cc) }])
                } else {
                    MessageContent::Text(text)
                }
            }
            MessageContent::Parts(mut parts) => {
                parts.iter_mut().for_each(ContentPart::reset_cache);
                match cache_control {
                    Some(_) => {
                        // cache the last part of the message
                        if let Some(part) = parts.last_mut() {
                            part.cached(enable_cache)
                        }
                        MessageContent::Parts(parts)
                    }
                    None => MessageContent::Parts(parts),
                }
            }
        }
    }

    pub fn is_cached(&self) -> bool {
        match self {
            MessageContent::Text(_) => false,
            MessageContent::Parts(parts) => parts.iter().any(|part| {
                if let ContentPart::Text { cache_control, .. } = part {
                    cache_control.is_some()
                } else {
                    false
                }
            }),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    ImageUrl {
        image_url: ImageUrl,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

impl ContentPart {
    pub fn reset_cache(&mut self) {
        match self {
            ContentPart::Text { cache_control, .. } => {
                *cache_control = None;
            }
            ContentPart::ImageUrl { cache_control, .. } => {
                *cache_control = None;
            }
        }
    }

    pub fn cached(&mut self, enable_cache: bool) {
        let src_cache_control =
            enable_cache.then_some(CacheControl { type_: CacheControlType::Ephemeral });
        match self {
            ContentPart::Text { cache_control, .. } => {
                *cache_control = src_cache_control;
            }
            ContentPart::ImageUrl { cache_control, .. } => {
                *cache_control = src_cache_control;
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub type_: CacheControlType,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CacheControlType {
    Ephemeral,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct FunctionDescription {
    pub description: Option<String>,
    pub name: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Tool {
    // TODO: should be an enum
    pub r#type: FunctionType,
    pub function: FunctionDescription,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct ResponseFormat {
    pub r#type: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Prediction {
    pub r#type: String,
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct ProviderPreferences {
    // Define fields as necessary
}

#[derive(Debug, Deserialize, Serialize, Clone, Setters, Default)]
#[setters(strip_option)]
pub struct Request {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<Message>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repetition_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<std::collections::HashMap<u32, f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_a: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prediction: Option<Prediction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transforms: Option<Vec<Transform>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderPreferences>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<forge_domain::ReasoningConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct StreamOptions {
    pub include_usage: Option<bool>,
}

impl Request {
    pub fn message_count(&self) -> usize {
        self.messages
            .as_ref()
            .map(|messages| messages.len())
            .unwrap_or(0)
    }

    pub fn message_cache_count(&self) -> usize {
        self.messages
            .iter()
            .flatten()
            .flat_map(|a| a.content.as_ref())
            .enumerate()
            .map(|(i, _)| i)
            .max()
            .unwrap_or(0)
    }
}

/// ref: https://openrouter.ai/docs/transforms
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Transform {
    #[default]
    #[serde(rename = "middle-out")]
    MiddleOut,
}

impl From<ToolDefinition> for Tool {
    fn from(value: ToolDefinition) -> Self {
        Tool {
            r#type: FunctionType,
            function: FunctionDescription {
                description: Some(value.description),
                name: value.name.to_string(),
                parameters: {
                    let mut params = serde_json::to_value(value.input_schema).unwrap();
                    // Ensure OpenAI compatibility by adding properties field if missing
                    if let Some(obj) = params.as_object_mut()
                        && obj.get("type") == Some(&serde_json::Value::String("object".to_string()))
                        && !obj.contains_key("properties")
                    {
                        obj.insert(
                            "properties".to_string(),
                            serde_json::Value::Object(serde_json::Map::new()),
                        );
                    }
                    params
                },
            },
        }
    }
}

impl From<Context> for Request {
    fn from(context: Context) -> Self {
        Request {
            messages: {
                let messages = context
                    .messages
                    .into_iter()
                    .map(Message::from)
                    .collect::<Vec<_>>();

                Some(messages)
            },
            tools: {
                let tools = context
                    .tools
                    .into_iter()
                    .map(Tool::from)
                    .collect::<Vec<_>>();
                if tools.is_empty() { None } else { Some(tools) }
            },
            model: None,
            prompt: Default::default(),
            response_format: Default::default(),
            stop: Default::default(),
            stream: Default::default(),
            max_tokens: context.max_tokens.map(|t| t as u32),
            temperature: context.temperature.map(|t| t.value()),
            tool_choice: context.tool_choice.map(|tc| tc.into()),
            seed: Default::default(),
            top_p: context.top_p.map(|t| t.value()),
            top_k: context.top_k.map(|t| t.value()),
            frequency_penalty: Default::default(),
            presence_penalty: Default::default(),
            repetition_penalty: Default::default(),
            logit_bias: Default::default(),
            top_logprobs: Default::default(),
            min_p: Default::default(),
            top_a: Default::default(),
            prediction: Default::default(),
            // Since compaction is support on the client we don't need middle-out transforms any
            // more
            transforms: Default::default(),
            models: Default::default(),
            route: Default::default(),
            provider: Default::default(),
            parallel_tool_calls: Some(false),
            stream_options: Some(StreamOptions { include_usage: Some(true) }),
            session_id: context.conversation_id.map(|id| id.to_string()),
            reasoning: context.reasoning,
            max_completion_tokens: Default::default(),
        }
    }
}

impl From<ToolCallFull> for ToolCall {
    fn from(value: ToolCallFull) -> Self {
        Self {
            id: value.call_id,
            r#type: FunctionType,
            function: FunctionCall {
                arguments: serde_json::to_string(&value.arguments).unwrap(),
                name: Some(value.name),
            },
        }
    }
}

impl From<ContextMessage> for Message {
    fn from(value: ContextMessage) -> Self {
        match value {
            ContextMessage::Text(chat_message) => Message {
                role: chat_message.role.into(),
                content: Some(MessageContent::Text(chat_message.content)),
                name: None,
                tool_call_id: None,
                tool_calls: chat_message
                    .tool_calls
                    .map(|tool_calls| tool_calls.into_iter().map(ToolCall::from).collect()),
                reasoning_details: chat_message.reasoning_details.map(|details| {
                    details
                        .into_iter()
                        .map(|detail| ReasoningDetail {
                            r#type: "reasoning.text".to_string(),
                            text: detail.text,
                            signature: detail.signature,
                        })
                        .collect::<Vec<ReasoningDetail>>()
                }),
            },
            ContextMessage::Tool(tool_result) => Message {
                role: Role::Tool,
                tool_call_id: tool_result.call_id.clone(),
                name: Some(tool_result.name.clone()),
                content: Some(tool_result.into()),
                tool_calls: None,
                reasoning_details: None,
            },
            ContextMessage::Image(img) => {
                let content = vec![ContentPart::ImageUrl {
                    image_url: ImageUrl { url: img.url().clone(), detail: None },
                    cache_control: None,
                }];
                Message {
                    role: Role::User,
                    content: Some(MessageContent::Parts(content)),
                    name: None,
                    tool_call_id: None,
                    tool_calls: None,
                    reasoning_details: None,
                }
            }
        }
    }
}

impl From<ToolResult> for MessageContent {
    fn from(result: ToolResult) -> Self {
        if result.output.values.len() == 1
            && let Some(text) = result.output.as_str()
        {
            return MessageContent::Text(text.to_string());
        }
        let mut parts = Vec::new();
        for value in result.output.values.into_iter() {
            match value {
                ToolValue::Text(text) => {
                    parts.push(ContentPart::Text { text, cache_control: None });
                }
                ToolValue::Image(img) => {
                    let content = ContentPart::ImageUrl {
                        image_url: ImageUrl { url: img.url().clone(), detail: None },
                        cache_control: None,
                    };
                    parts.push(content);
                }
                ToolValue::Empty => {
                    // Handle empty case if needed
                }
            }
        }

        MessageContent::Parts(parts)
    }
}

impl From<forge_domain::Role> for Role {
    fn from(role: forge_domain::Role) -> Self {
        match role {
            forge_domain::Role::System => Role::System,
            forge_domain::Role::User => Role::User,
            forge_domain::Role::Assistant => Role::Assistant,
        }
    }
}

#[derive(Debug, Deserialize, Display, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_cached_text_true() {
        let fixture = MessageContent::Text("hello".to_string());
        let actual = fixture.cached(true);
        let expected = MessageContent::Parts(vec![ContentPart::Text {
            text: "hello".to_string(),
            cache_control: Some(CacheControl { type_: CacheControlType::Ephemeral }),
        }]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cached_text_false() {
        let fixture = MessageContent::Text("hello".to_string());
        let actual = fixture.cached(false);
        let expected = MessageContent::Text("hello".to_string());
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cached_parts_true() {
        let fixture = MessageContent::Parts(vec![
            ContentPart::Text { text: "a".to_string(), cache_control: None },
            ContentPart::ImageUrl {
                image_url: ImageUrl { url: "http://example.com/a.png".to_string(), detail: None },
                cache_control: Some(CacheControl { type_: CacheControlType::Ephemeral }),
            },
        ]);
        let actual = fixture.cached(true);
        let expected = MessageContent::Parts(vec![
            ContentPart::Text { text: "a".to_string(), cache_control: None },
            ContentPart::ImageUrl {
                image_url: ImageUrl { url: "http://example.com/a.png".to_string(), detail: None },
                cache_control: Some(CacheControl { type_: CacheControlType::Ephemeral }),
            },
        ]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cached_parts_multi_false() {
        let fixture = MessageContent::Parts(vec![
            ContentPart::Text {
                text: "a".to_string(),
                cache_control: Some(CacheControl { type_: CacheControlType::Ephemeral }),
            },
            ContentPart::Text {
                text: "b".to_string(),
                cache_control: Some(CacheControl { type_: CacheControlType::Ephemeral }),
            },
            ContentPart::ImageUrl {
                image_url: ImageUrl { url: "http://example.com/a.png".to_string(), detail: None },
                cache_control: Some(CacheControl { type_: CacheControlType::Ephemeral }),
            },
        ]);
        let actual = fixture.cached(false);
        let expected = MessageContent::Parts(vec![
            ContentPart::Text { text: "a".to_string(), cache_control: None },
            ContentPart::Text { text: "b".to_string(), cache_control: None },
            ContentPart::ImageUrl {
                image_url: ImageUrl { url: "http://example.com/a.png".to_string(), detail: None },
                cache_control: None,
            },
        ]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cached_parts_already_true() {
        let fixture = MessageContent::Parts(vec![
            ContentPart::Text {
                text: "a".to_string(),
                cache_control: Some(CacheControl { type_: CacheControlType::Ephemeral }),
            },
            ContentPart::Text { text: "b".to_string(), cache_control: None },
            ContentPart::ImageUrl {
                image_url: ImageUrl { url: "http://example.com/a.png".to_string(), detail: None },
                cache_control: None,
            },
        ]);
        let actual = fixture.cached(true);
        let expected = MessageContent::Parts(vec![
            ContentPart::Text { text: "a".to_string(), cache_control: None },
            ContentPart::Text { text: "b".to_string(), cache_control: None },
            ContentPart::ImageUrl {
                image_url: ImageUrl { url: "http://example.com/a.png".to_string(), detail: None },
                cache_control: Some(CacheControl { type_: CacheControlType::Ephemeral }),
            },
        ]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cached_parts_multi_true() {
        let fixture = MessageContent::Parts(vec![
            ContentPart::Text { text: "a".to_string(), cache_control: None },
            ContentPart::Text { text: "b".to_string(), cache_control: None },
            ContentPart::ImageUrl {
                image_url: ImageUrl { url: "http://example.com/a.png".to_string(), detail: None },
                cache_control: None,
            },
        ]);
        let actual = fixture.cached(true);
        let expected = MessageContent::Parts(vec![
            ContentPart::Text { text: "a".to_string(), cache_control: None },
            ContentPart::Text { text: "b".to_string(), cache_control: None },
            ContentPart::ImageUrl {
                image_url: ImageUrl { url: "http://example.com/a.png".to_string(), detail: None },
                cache_control: Some(CacheControl { type_: CacheControlType::Ephemeral }),
            },
        ]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cached_parts_false() {
        let fixture = MessageContent::Parts(vec![
            ContentPart::Text { text: "a".to_string(), cache_control: None },
            ContentPart::ImageUrl {
                image_url: ImageUrl { url: "http://example.com/a.png".to_string(), detail: None },
                cache_control: None,
            },
        ]);
        let actual = fixture.cached(false);
        let expected = MessageContent::Parts(vec![
            ContentPart::Text { text: "a".to_string(), cache_control: None },
            ContentPart::ImageUrl {
                image_url: ImageUrl { url: "http://example.com/a.png".to_string(), detail: None },
                cache_control: None,
            },
        ]);
        assert_eq!(actual, expected);
    }

    use forge_domain::{
        ContextMessage, Role, TextMessage, ToolCallFull, ToolCallId, ToolName, ToolResult,
    };
    use insta::assert_json_snapshot;

    #[test]
    fn test_user_message_conversion() {
        let user_message = ContextMessage::Text(TextMessage {
            role: Role::User,
            content: "Hello".to_string(),
            tool_calls: None,
            model: ModelId::new("gpt-3.5-turbo").into(),
            reasoning_details: None,
        });
        let router_message = Message::from(user_message);
        assert_json_snapshot!(router_message);
    }

    #[test]
    fn test_message_with_special_chars() {
        let xml_content = r#"Here's some XML content:
<task>
    <id>123</id>
    <description>Test <special> characters</description>
    <data key="value">
        <item>1</item>
        <item>2</item>
    </data>
</task>"#;

        let message = ContextMessage::Text(TextMessage {
            role: Role::User,
            content: xml_content.to_string(),
            tool_calls: None,
            model: ModelId::new("gpt-3.5-turbo").into(),
            reasoning_details: None,
        });
        let router_message = Message::from(message);
        assert_json_snapshot!(router_message);
    }

    #[test]
    fn test_assistant_message_with_tool_call_conversion() {
        let tool_call = ToolCallFull {
            call_id: Some(ToolCallId::new("123")),
            name: ToolName::new("test_tool"),
            arguments: serde_json::json!({"key": "value"}).into(),
        };

        let assistant_message = ContextMessage::Text(TextMessage {
            role: Role::Assistant,
            content: "Using tool".to_string(),
            tool_calls: Some(vec![tool_call]),
            model: ModelId::new("gpt-3.5-turbo").into(),
            reasoning_details: None,
        });
        let router_message = Message::from(assistant_message);
        assert_json_snapshot!(router_message);
    }

    #[test]
    fn test_tool_message_conversion() {
        let tool_result = ToolResult::new(ToolName::new("test_tool"))
            .call_id(ToolCallId::new("123"))
            .success(
                r#"{
               "user": "John",
               "age": 30,
               "address": [{"city": "New York"}, {"city": "San Francisco"}]
            }"#,
            );

        let tool_message = ContextMessage::Tool(tool_result);
        let router_message = Message::from(tool_message);
        assert_json_snapshot!(router_message);
    }

    #[test]
    fn test_tool_message_with_special_chars() {
        let tool_result = ToolResult::new(ToolName::new("html_tool"))
            .call_id(ToolCallId::new("456"))
            .success(
                r#"{
                "html": "<div class=\"container\"><p>Hello <World></p></div>",
                "elements": ["<span>", "<br/>", "<hr>"],
                "attributes": {
                    "style": "color: blue; font-size: 12px;",
                    "data-test": "<test>&value</test>"
                }
            }"#,
            );

        let tool_message = ContextMessage::Tool(tool_result);
        let router_message = Message::from(tool_message);
        assert_json_snapshot!(router_message);
    }

    #[test]
    fn test_tool_message_typescript_code() {
        let tool_result = ToolResult::new(ToolName::new("rust_tool"))
            .call_id(ToolCallId::new("456"))
            .success(r#"{ "code": "fn main<T>(gt: T) {let b = &gt; }"}"#);

        let tool_message = ContextMessage::Tool(tool_result);
        let router_message = Message::from(tool_message);
        assert_json_snapshot!(router_message);
    }

    #[test]
    fn test_transform_display() {
        assert_eq!(
            serde_json::to_string(&Transform::MiddleOut).unwrap(),
            "\"middle-out\""
        );
    }
    #[test]
    fn test_tool_definition_conversion_missing_properties() {
        // Test case where input_schema is an object type but missing properties field
        let fixture = {
            let mut schema = schemars::schema_for!(());
            // Create an object schema without properties field
            schema.schema.object = Some(Box::new(schemars::schema::ObjectValidation {
                max_properties: None,
                min_properties: None,
                required: Default::default(),
                properties: Default::default(), // Empty properties map
                pattern_properties: Default::default(),
                additional_properties: None,
                property_names: None,
            }));
            schema.schema.instance_type = Some(schemars::schema::SingleOrVec::Single(Box::new(
                schemars::schema::InstanceType::Object,
            )));

            ToolDefinition::new("test_tool")
                .description("Test tool")
                .input_schema(schema)
        };

        let actual = Tool::from(fixture);

        let expected = Tool {
            r#type: FunctionType,
            function: FunctionDescription {
                description: Some("Test tool".to_string()),
                name: "test_tool".to_string(),
                parameters: serde_json::json!({
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "properties": {},
                    "title": "Null",
                    "type": "object"
                }),
            },
        };

        assert_eq!(actual, expected);
    }
}
