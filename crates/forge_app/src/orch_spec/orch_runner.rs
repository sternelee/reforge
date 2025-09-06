use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use forge_domain::{
    ChatCompletionMessage, ChatResponse, Conversation, ConversationId, ToolCallFull, ToolResult,
};
use handlebars::{Handlebars, no_escape};
use rust_embed::Embed;
use tokio::sync::Mutex;

pub use super::orch_setup::TestContext;
use crate::AgentService;
use crate::orch::Orchestrator;

#[derive(Embed)]
#[folder = "../../templates/"]
struct Templates;

pub struct Runner {
    hb: Handlebars<'static>,
    // History of all the updates made to the conversation
    conversation_history: Mutex<Vec<Conversation>>,

    // Tool call requests and the mock responses
    test_tool_calls: Mutex<VecDeque<(ToolCallFull, ToolResult)>>,

    // Mock completions from the LLM (Each value is produced as an event in the stream)
    test_completions: Mutex<VecDeque<ChatCompletionMessage>>,
}

impl Runner {
    fn new(setup: &TestContext) -> Self {
        let mut hb = Handlebars::new();
        hb.set_strict_mode(true);
        hb.register_escape_fn(no_escape);

        // Register all partial templates
        hb.register_embed_templates::<Templates>().unwrap();
        for (name, tpl) in &setup.templates {
            hb.register_template_string(name, tpl).unwrap();
        }

        Self {
            hb,
            conversation_history: Mutex::new(Vec::new()),
            test_tool_calls: Mutex::new(VecDeque::from(setup.mock_tool_call_responses.clone())),
            test_completions: Mutex::new(VecDeque::from(setup.mock_assistant_responses.clone())),
        }
    }

    // Returns the conversation history
    async fn get_history(&self) -> Vec<Conversation> {
        self.conversation_history.lock().await.clone()
    }

    pub async fn run(setup: &mut TestContext) -> anyhow::Result<()> {
        const LIMIT: usize = 1024;
        let (tx, mut rx) = tokio::sync::mpsc::channel::<anyhow::Result<ChatResponse>>(LIMIT);
        let agents = setup.agents.clone();
        let services = Arc::new(Runner::new(setup));
        let conversation = Conversation::new(
            ConversationId::generate(),
            setup.workflow.clone(),
            Default::default(),
            agents,
        );

        let orch = Orchestrator::new(
            services.clone(),
            setup.env.clone(),
            conversation,
            setup.current_time,
            vec![], // empty custom_instructions
        )
        .sender(tx)
        .files(setup.files.clone());

        let (mut orch, runner) = (orch, services);
        let event = setup.event.clone();
        let mut chat_responses = Vec::new();
        let result = orch.chat(event).await;
        tokio::time::timeout(
            Duration::from_secs(1),
            rx.recv_many(&mut chat_responses, LIMIT),
        )
        .await?;
        setup.output.chat_responses.extend(chat_responses);
        setup
            .output
            .conversation_history
            .extend(runner.get_history().await);

        result
    }
}

#[async_trait::async_trait]
impl AgentService for Runner {
    async fn chat_agent(
        &self,
        _id: &forge_domain::ModelId,
        context: forge_domain::Context,
    ) -> forge_domain::ResultStream<ChatCompletionMessage, anyhow::Error> {
        let mut responses = self.test_completions.lock().await;
        if let Some(message) = responses.pop_front() {
            Ok(Box::pin(tokio_stream::iter(std::iter::once(Ok(message)))))
        } else {
            let total_messages = context.messages.len();
            let last_message = context.messages.last();
            panic!(
                "No mock response found. Total Messages: {total_messages}. Last Message: {last_message:#?}"
            )
        }
    }

    async fn call(
        &self,
        _: &forge_domain::Agent,
        _: &forge_domain::ToolCallContext,
        test_call: forge_domain::ToolCallFull,
    ) -> forge_domain::ToolResult {
        let mut guard = self.test_tool_calls.lock().await;
        for (id, (call, result)) in guard.iter().enumerate() {
            if call.call_id == test_call.call_id {
                let result = result.clone();
                guard.remove(id);
                return result;
            }
        }
        panic!("Tool call not found")
    }

    async fn render(
        &self,
        template: &str,
        object: &(impl serde::Serialize + Sync),
    ) -> anyhow::Result<String> {
        Ok(self.hb.render_template(template, object)?)
    }

    async fn update(&self, conversation: Conversation) -> anyhow::Result<()> {
        self.conversation_history.lock().await.push(conversation);
        Ok(())
    }
}
