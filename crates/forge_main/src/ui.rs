use std::collections::BTreeMap;
use std::fmt::Display;
use std::sync::Arc;

use anyhow::{Context, Result};
use colored::Colorize;
use convert_case::{Case, Casing};
use forge_api::{
    API, AgentId, AppConfig, ChatRequest, ChatResponse, Conversation, ConversationId, Event,
    InterruptionReason, Model, ModelId, Workflow,
};
use forge_display::{MarkdownFormat, TitleFormat};
use forge_domain::{McpConfig, McpServerConfig, Provider, Scope};
use forge_fs::ForgeFS;
use forge_iroh_node::{ForgeIrohNode, IrohConfig};
use forge_spinner::SpinnerManager;
use forge_tracker::ToolCallPayload;
use merge::Merge;
use serde::Deserialize;
use serde_json::Value;
use tokio_stream::StreamExt;
use whoami;

use crate::cli::{Cli, McpCommand, TopLevelCommand, Transport};
use crate::info::Info;
use crate::input::Console;
use crate::model::{Command, ForgeCommandManager};
use crate::select::ForgeSelect;
use crate::state::UIState;
use crate::update::on_update;
use crate::{TRACKER, banner, tracker};

// Event type constants moved to UI layer
pub const EVENT_USER_TASK_INIT: &str = "user_task_init";
pub const EVENT_USER_TASK_UPDATE: &str = "user_task_update";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct PartialEvent {
    pub name: String,
    pub value: Value,
}

impl PartialEvent {
    pub fn new<V: Into<Value>>(name: impl ToString, value: V) -> Self {
        Self { name: name.to_string(), value: value.into() }
    }
}

impl From<PartialEvent> for Event {
    fn from(value: PartialEvent) -> Self {
        Event::new(value.name, Some(value.value))
    }
}

pub struct UI<A, F: Fn() -> A> {
    markdown: MarkdownFormat,
    state: UIState,
    api: Arc<F::Output>,
    new_api: Arc<F>,
    console: Console,
    command: Arc<ForgeCommandManager>,
    cli: Cli,
    spinner: SpinnerManager,
    iroh_node: Option<ForgeIrohNode>,
    #[allow(dead_code)] // The guard is kept alive by being held in the struct
    _guard: forge_tracker::Guard,
}

impl<A: API + 'static, F: Fn() -> A> UI<A, F> {
    /// Writes a line to the console output
    /// Takes anything that implements ToString trait
    fn writeln<T: ToString>(&mut self, content: T) -> anyhow::Result<()> {
        self.spinner.write_ln(content)
    }

    /// Retrieve available models
    async fn get_models(&mut self) -> Result<Vec<Model>> {
        self.spinner.start(Some("Loading"))?;
        let models = self.api.models().await?;
        self.spinner.stop(None)?;
        Ok(models)
    }

    // Handle creating a new conversation
    async fn on_new(&mut self) -> Result<()> {
        self.api = Arc::new((self.new_api)());
        self.init_state(false).await?;
        banner::display()?;
        self.trace_user();
        Ok(())
    }

    async fn active_workflow(&self) -> Result<Workflow> {
        // Read the current workflow to validate the agent
        let workflow = self.api.read_workflow(self.cli.workflow.as_deref()).await?;
        let mut base_workflow = Workflow::default();
        base_workflow.merge(workflow.clone());
        Ok(base_workflow)
    }

    // Set the current mode and update conversation variable
    async fn on_agent_change(&mut self, agent_id: AgentId) -> Result<()> {
        let workflow = self.active_workflow().await?;

        // Convert string to AgentId for validation
        let agent = workflow.get_agent(&AgentId::new(agent_id))?;

        let conversation_id = self.init_conversation().await?;
        if let Some(mut conversation) = self.api.conversation(&conversation_id).await? {
            conversation.set_variable("operating_agent".into(), Value::from(agent.id.as_str()));
            self.api.upsert_conversation(conversation).await?;
        }

        // Reset is_first to true when switching agents
        self.state.is_first = true;
        self.state.operating_agent = agent.id.clone();

        // Update the workflow with the new operating agent.
        self.api
            .update_workflow(self.cli.workflow.as_deref(), |workflow| {
                workflow.variables.insert(
                    "operating_agent".to_string(),
                    Value::from(agent.id.as_str()),
                );
            })
            .await?;

        self.writeln(TitleFormat::action(format!(
            "Switched to agent {}",
            agent.id.as_str().to_case(Case::UpperSnake).bold()
        )))?;

        Ok(())
    }

    fn create_task_event<V: Into<Value>>(
        &self,
        content: Option<V>,
        event_name: &str,
    ) -> anyhow::Result<Event> {
        let operating_agent = &self.state.operating_agent;
        Ok(Event::new(
            format!("{operating_agent}/{event_name}"),
            content,
        ))
    }

    pub fn init(cli: Cli, f: F) -> Result<Self> {
        // Parse CLI arguments first to get flags
        let api = Arc::new(f());
        let env = api.environment();
        let command = Arc::new(ForgeCommandManager::default());
        Ok(Self {
            state: Default::default(),
            api,
            new_api: Arc::new(f),
            console: Console::new(env.clone(), command.clone()),
            cli,
            command,
            spinner: SpinnerManager::new(),
            markdown: MarkdownFormat::new(),
            iroh_node: None,
            _guard: forge_tracker::init_tracing(env.log_path(), TRACKER.clone())?,
        })
    }

    async fn prompt(&self) -> Result<Command> {
        // Prompt the user for input
        self.console.prompt(self.state.clone().into()).await
    }

    pub async fn run(&mut self) {
        match self.run_inner().await {
            Ok(_) => {}
            Err(error) => {
                tracing::error!(error = ?error);
                eprintln!("{}", TitleFormat::error(format!("{error:?}")));
            }
        }
    }

    async fn run_inner(&mut self) -> Result<()> {
        if let Some(mcp) = self.cli.subcommands.clone() {
            return self.handle_subcommands(mcp).await;
        }

        // Check for dispatch flag first
        if let Some(dispatch_json) = self.cli.event.clone() {
            return self.handle_dispatch(dispatch_json).await;
        }

        // Handle direct prompt if provided
        let prompt = self.cli.prompt.clone();
        if let Some(prompt) = prompt {
            self.on_message(Some(prompt)).await?;
            return Ok(());
        }

        // Display the banner in dimmed colors since we're in interactive mode
        banner::display()?;
        self.init_state(true).await?;
        self.trace_user();

        // Get initial input from file or prompt
        let mut command = match &self.cli.command {
            Some(path) => self.console.upload(path).await?,
            None => self.prompt().await?,
        };

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("User interrupted operation with Ctrl+C");
                }
                result = self.on_command(command) => {
                    match result {
                        Ok(exit) => if exit {return Ok(())},
                        Err(error) => {
                            if let Some(conversation_id) = self.state.conversation_id.as_ref()
                                && let Some(conversation) = self.api.conversation(conversation_id).await.ok().flatten() {
                                    TRACKER.set_conversation(conversation).await;
                                }
                            tracker::error(&error);
                            tracing::error!(error = ?error);
                            self.spinner.stop(None)?;
                            eprintln!("{}", TitleFormat::error(format!("{error:?}")));
                        },
                    }
                }
            }

            self.spinner.stop(None)?;

            // Centralized prompt call at the end of the loop
            command = self.prompt().await?;
        }
    }

    async fn handle_subcommands(&mut self, subcommand: TopLevelCommand) -> anyhow::Result<()> {
        match subcommand {
            TopLevelCommand::Mcp(mcp_command) => match mcp_command.command {
                McpCommand::Add(add) => {
                    let name = add.name;
                    let scope: Scope = add.scope.into();
                    // Create the appropriate server type based on transport
                    let server = match add.transport {
                        Transport::Stdio => McpServerConfig::new_stdio(
                            add.command_or_url.clone(),
                            add.args.clone(),
                            Some(parse_env(add.env.clone())),
                        ),
                        Transport::Sse => McpServerConfig::new_sse(add.command_or_url.clone()),
                    };
                    // Command/URL already set in the constructor

                    self.update_mcp_config(&scope, |config| {
                        config.mcp_servers.insert(name.to_string(), server);
                    })
                    .await?;

                    self.writeln(TitleFormat::info(format!("Added MCP server '{name}'")))?;
                }
                McpCommand::List => {
                    let mcp_servers = self.api.read_mcp_config().await?;
                    if mcp_servers.is_empty() {
                        self.writeln(TitleFormat::error("No MCP servers found"))?;
                    }

                    let mut output = String::new();
                    for (name, server) in mcp_servers.mcp_servers {
                        output.push_str(&format!("{name}: {server}"));
                    }
                    self.writeln(output)?;
                }
                McpCommand::Remove(rm) => {
                    let name = rm.name.clone();
                    let scope: Scope = rm.scope.into();

                    self.update_mcp_config(&scope, |config| {
                        config.mcp_servers.remove(name.as_str());
                    })
                    .await?;

                    self.writeln(TitleFormat::info(format!("Removed server: {name}")))?;
                }
                McpCommand::Get(val) => {
                    let name = val.name.clone();
                    let config = self.api.read_mcp_config().await?;
                    let server = config
                        .mcp_servers
                        .get(name.as_str())
                        .ok_or(anyhow::anyhow!("Server not found"))?;

                    let mut output = String::new();
                    output.push_str(&format!("{name}: {server}"));
                    self.writeln(TitleFormat::info(output))?;
                }
                McpCommand::AddJson(add_json) => {
                    let server = serde_json::from_str::<McpServerConfig>(add_json.json.as_str())
                        .context("Failed to parse JSON")?;
                    let scope: Scope = add_json.scope.into();
                    let name = add_json.name.clone();
                    self.update_mcp_config(&scope, |config| {
                        config.mcp_servers.insert(name.clone(), server);
                    })
                    .await?;

                    self.writeln(TitleFormat::info(format!(
                        "Added server: {}",
                        add_json.name
                    )))?;
                }
            },
            TopLevelCommand::Info => {
                // Make sure to init model
                self.on_new().await?;

                self.on_info().await?;
                return Ok(());
            }
        }
        Ok(())
    }

    async fn on_info(&mut self) -> anyhow::Result<()> {
        self.spinner.start(Some("Loading Info"))?;
        let mut info = Info::from(&self.state).extend(Info::from(&self.api.environment()));

        // Add user information if available
        if let Ok(config) = self.api.app_config().await
            && let Some(login_info) = &config.key_info
        {
            info = info.extend(Info::from(login_info));
        }

        // Add usage information
        if let Ok(Some(user_usage)) = self.api.user_usage().await {
            info = info.extend(Info::from(&user_usage));
        }

        self.writeln(info)?;
        self.spinner.stop(None)?;

        Ok(())
    }

    async fn on_command(&mut self, command: Command) -> anyhow::Result<bool> {
        match command {
            Command::Compact => {
                self.spinner.start(Some("Compacting"))?;
                self.on_compaction().await?;
            }
            Command::Dump(format) => {
                self.spinner.start(Some("Dumping"))?;
                self.on_dump(format).await?;
            }
            Command::New => {
                self.on_new().await?;
            }
            Command::Info => {
                self.on_info().await?;
            }
            Command::Message(ref content) => {
                self.spinner.start(None)?;
                self.on_message(Some(content.clone())).await?;
            }
            Command::Forge => {
                self.on_agent_change(AgentId::FORGE).await?;
            }
            Command::Muse => {
                self.on_agent_change(AgentId::MUSE).await?;
            }
            Command::Help => {
                let info = Info::from(self.command.as_ref());
                self.writeln(info)?;
            }
            Command::Tools => {
                self.spinner.start(Some("Loading"))?;
                use crate::tools_display::format_tools;
                let tools = self.api.tools().await?;

                let output = format_tools(&tools);
                self.writeln(output)?;
            }
            Command::Update => {
                on_update(self.api.clone(), None).await;
            }
            Command::Exit => {
                return Ok(true);
            }

            Command::Custom(event) => {
                self.spinner.start(None)?;
                self.on_custom_event(event.into()).await?;
            }
            Command::Model => {
                self.on_model_selection().await?;
            }
            Command::Shell(ref command) => {
                self.api.execute_shell_command_raw(command).await?;
            }
            Command::Agent => {
                // Read the current workflow to validate the agent
                let workflow = self.active_workflow().await?;

                #[derive(Clone)]
                struct Agent {
                    id: AgentId,
                    label: String,
                }

                impl Display for Agent {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(f, "{}", self.label)
                    }
                }
                let n = workflow
                    .agents
                    .iter()
                    .map(|a| a.id.as_str().len())
                    .max()
                    .unwrap_or_default();
                let display_agents = workflow
                    .agents
                    .into_iter()
                    .map(|agent| {
                        let title = &agent.title.unwrap_or("<Missing agent.title>".to_string());
                        {
                            let label = format!(
                                "{:<n$} {}",
                                agent.id.as_str().to_case(Case::UpperSnake).bold(),
                                title.lines().collect::<Vec<_>>().join(" ").dimmed()
                            );
                            Agent { label, id: agent.id.clone() }
                        }
                    })
                    .collect::<Vec<_>>();

                if let Some(selected_agent) = ForgeSelect::select(
                    "select the agent from following list",
                    display_agents.clone(),
                )
                .prompt()?
                {
                    self.on_agent_change(selected_agent.id).await?;
                }
            }
            Command::Login => {
                self.spinner.start(Some("Logging in"))?;
                self.api.logout().await?;
                self.login().await?;
                self.spinner.stop(None)?;
                let config: AppConfig = self.api.app_config().await?;
                tracker::login(
                    config
                        .key_info
                        .and_then(|v| v.auth_provider_id)
                        .unwrap_or_default(),
                );
                let provider = self.api.provider().await?;
                self.state.provider = Some(provider);
            }
            Command::Logout => {
                self.spinner.start(Some("Logging out"))?;
                self.api.logout().await?;
                self.spinner.stop(None)?;
                self.writeln(TitleFormat::info("Logged out"))?;
                // Exit the UI after logout
                return Ok(true);
            }
            Command::Retry => {
                self.spinner.start(None)?;
                self.on_message(None).await?;
            }
        }

        Ok(false)
    }
    async fn on_compaction(&mut self) -> Result<(), anyhow::Error> {
        let conversation_id = self.init_conversation().await?;
        let compaction_result = self.api.compact_conversation(&conversation_id).await?;
        let token_reduction = compaction_result.token_reduction_percentage();
        let message_reduction = compaction_result.message_reduction_percentage();
        let content = TitleFormat::action(format!(
            "Context size reduced by {token_reduction:.1}% (tokens), {message_reduction:.1}% (messages)"
        ));
        self.writeln(content)?;
        Ok(())
    }

    /// Select a model from the available models
    /// Returns Some(ModelId) if a model was selected, or None if selection was
    /// canceled
    async fn select_model(&mut self) -> Result<Option<ModelId>> {
        // Fetch available models
        let models = self
            .get_models()
            .await?
            .into_iter()
            .map(CliModel)
            .collect::<Vec<_>>();

        // Find the index of the current model
        let starting_cursor = self
            .state
            .model
            .as_ref()
            .and_then(|current| models.iter().position(|m| &m.0.id == current))
            .unwrap_or(0);

        // Use the centralized select module
        match ForgeSelect::select("Select a model:", models)
            .with_starting_cursor(starting_cursor)
            .with_help_message("Type a name or use arrow keys to navigate and Enter to select")
            .prompt()?
        {
            Some(model) => Ok(Some(model.0.id)),
            None => Ok(None),
        }
    }

    // Helper method to handle model selection and update the conversation
    async fn on_model_selection(&mut self) -> Result<()> {
        // Select a model
        let model_option = self.select_model().await?;

        // If no model was selected (user canceled), return early
        let model = match model_option {
            Some(model) => model,
            None => return Ok(()),
        };

        self.api
            .update_workflow(self.cli.workflow.as_deref(), |workflow| {
                workflow.model = Some(model.clone());
            })
            .await?;

        // Get the conversation to update
        let conversation_id = self.init_conversation().await?;

        if let Some(mut conversation) = self.api.conversation(&conversation_id).await? {
            // Update the model in the conversation
            conversation.set_model(&model)?;

            // Upsert the updated conversation
            self.api.upsert_conversation(conversation).await?;

            // Update the UI state with the new model
            self.update_model(model.clone());

            self.writeln(TitleFormat::action(format!("Switched to model: {model}")))?;
        }

        Ok(())
    }

    // Handle dispatching events from the CLI
    async fn handle_dispatch(&mut self, json: String) -> Result<()> {
        // Initialize the conversation
        let conversation_id = self.init_conversation().await?;

        // Parse the JSON to determine the event name and value
        let event: PartialEvent = serde_json::from_str(&json)?;

        // Create the chat request with the event
        let chat = ChatRequest::new(event.into(), conversation_id);

        self.on_chat(chat).await
    }

    async fn init_conversation(&mut self) -> Result<ConversationId> {
        match self.state.conversation_id {
            Some(ref id) => Ok(*id),
            None => {
                self.spinner.start(Some("Initializing"))?;

                // Select a model if workflow doesn't have one
                let workflow = self.init_state(false).await?;
                // We need to try and get the conversation ID first before fetching the model
                let id = if let Some(ref path) = self.cli.conversation {
                    let conversation: Conversation =
                        serde_json::from_str(ForgeFS::read_utf8(path.as_os_str()).await?.as_str())
                            .context("Failed to parse Conversation")?;

                    let conversation_id = conversation.id;
                    self.state.conversation_id = Some(conversation_id);
                    self.update_model(conversation.main_model()?);
                    self.api.upsert_conversation(conversation).await?;
                    conversation_id
                } else {
                    let conversation = self.api.init_conversation(workflow).await?;
                    self.state.conversation_id = Some(conversation.id);
                    self.update_model(conversation.main_model()?);
                    conversation.id
                };

                Ok(id)
            }
        }
    }

    /// Initialize the state of the UI
    async fn init_state(&mut self, first: bool) -> Result<Workflow> {
        let provider = self.init_provider().await?;
        let mut workflow = self.api.read_workflow(self.cli.workflow.as_deref()).await?;
        if workflow.model.is_none() {
            workflow.model = Some(
                self.select_model()
                    .await?
                    .ok_or(anyhow::anyhow!("Model selection is required to continue"))?,
            );
        }
        let mut base_workflow = Workflow::default();
        base_workflow.merge(workflow.clone());
        if first {
            // Initialize iroh node on first startup
            self.init_iroh_node().await;
            
            // only call on_update if this is the first initialization
            on_update(self.api.clone(), base_workflow.updates.as_ref()).await;
        }
        self.api
            .write_workflow(self.cli.workflow.as_deref(), &workflow)
            .await?;

        self.command.register_all(&base_workflow);
        self.state = UIState::new(self.api.environment(), base_workflow).provider(provider);

        Ok(workflow)
    }

    /// Initialize iroh P2P node
    async fn init_iroh_node(&mut self) {
        // Create iroh config - enable by default for now
        let config = IrohConfig::new()
            .enabled(true)
            .with_topics(vec!["forge-general".to_string()])
            .with_name(format!("forge-{}", whoami::username()))
            .auto_execute(false); // Disable auto-execute for security

        // Validate config
        if let Err(e) = config.validate() {
            self.writeln(TitleFormat::error(format!("Iroh config validation failed: {}", e))).ok();
            return;
        }

        // Only proceed if enabled
        if !config.enabled {
            return;
        }

        // Create and initialize node
        let (mut node, _message_rx) = ForgeIrohNode::new();
        
        match node.init().await {
            Ok(_) => {
                // Print node information
                node.print_node_info().await;
                
                // Join default topics
                for topic in &config.default_topics {
                    match node.join_topic(topic).await {
                        Ok(_) => {
                            self.writeln(format!("📡 Joined P2P topic: {}", topic)).ok();
                        }
                        Err(e) => {
                            self.writeln(TitleFormat::error(format!("Failed to join topic '{}': {}", topic, e))).ok();
                        }
                    }
                }
                
                // Store the node
                self.iroh_node = Some(node);
                
                self.writeln("✅ Iroh P2P node initialized successfully").ok();
            }
            Err(e) => {
                self.writeln(TitleFormat::error(format!("Failed to initialize iroh node: {}", e))).ok();
            }
        }
    }
    async fn init_provider(&mut self) -> Result<Provider> {
        match self.api.provider().await {
            // Use the forge key if available in the config.
            Ok(provider) => Ok(provider),
            Err(_) => {
                // If no key is available, start the login flow.
                self.login().await?;
                let config: AppConfig = self.api.app_config().await?;
                tracker::login(
                    config
                        .key_info
                        .and_then(|v| v.auth_provider_id)
                        .unwrap_or_default(),
                );
                self.api.provider().await
            }
        }
    }
    async fn login(&mut self) -> Result<()> {
        let auth = self.api.init_login().await?;
        open::that(auth.auth_url.as_str()).ok();
        self.writeln(TitleFormat::info(
            format!("Login here: {}", auth.auth_url).as_str(),
        ))?;
        self.spinner.start(Some("Waiting for login to complete"))?;

        self.api.login(&auth).await?;

        self.spinner.stop(None)?;

        self.writeln(TitleFormat::info("Login completed".to_string().as_str()))?;

        Ok(())
    }

    async fn on_message(&mut self, content: Option<String>) -> Result<()> {
        let conversation_id = self.init_conversation().await?;

        // Create a ChatRequest with the appropriate event type
        let event = if self.state.is_first {
            self.state.is_first = false;
            self.create_task_event(content, EVENT_USER_TASK_INIT)?
        } else {
            self.create_task_event(content, EVENT_USER_TASK_UPDATE)?
        };

        // Create the chat request with the event
        let chat = ChatRequest::new(event, conversation_id);

        self.on_chat(chat).await
    }

    async fn on_chat(&mut self, chat: ChatRequest) -> Result<()> {
        let mut stream = self.api.chat(chat).await?;

        while let Some(message) = stream.next().await {
            match message {
                Ok(message) => self.handle_chat_response(message).await?,
                Err(err) => {
                    self.spinner.stop(None)?;
                    return Err(err);
                }
            }
        }

        self.spinner.stop(None)?;

        Ok(())
    }

    /// Modified version of handle_dump that supports HTML format
    async fn on_dump(&mut self, format: Option<String>) -> Result<()> {
        if let Some(conversation_id) = self.state.conversation_id {
            let conversation = self.api.conversation(&conversation_id).await?;
            if let Some(conversation) = conversation {
                let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
                if let Some(format) = format {
                    if format == "html" {
                        // Export as HTML
                        let html_content = conversation.to_html();
                        let path = format!("{timestamp}-dump.html");
                        tokio::fs::write(path.as_str(), html_content).await?;

                        self.writeln(
                            TitleFormat::action("Conversation HTML dump created".to_string())
                                .sub_title(path.to_string()),
                        )?;

                        open::that(path.as_str()).ok();

                        return Ok(());
                    }
                } else {
                    // Default: Export as JSON
                    let path = format!("{timestamp}-dump.json");
                    let content = serde_json::to_string_pretty(&conversation)?;
                    tokio::fs::write(path.as_str(), content).await?;

                    self.writeln(
                        TitleFormat::action("Conversation JSON dump created".to_string())
                            .sub_title(path.to_string()),
                    )?;

                    open::that(path.as_str()).ok();
                };
            } else {
                return Err(anyhow::anyhow!("Could not create dump"))
                    .context(format!("Conversation: {conversation_id} was not found"));
            }
        } else {
            return Err(anyhow::anyhow!("No conversation initiated yet"))
                .context("Could not create dump");
        }
        Ok(())
    }

    async fn handle_chat_response(&mut self, message: ChatResponse) -> Result<()> {
        match message {
            ChatResponse::Text { mut text, is_complete, is_md } => {
                if is_complete && !text.trim().is_empty() {
                    if is_md {
                        tracing::info!(message = %text, "Agent Response");
                        text = self.markdown.render(&text);
                    }

                    self.writeln(text)?;
                }
            }
            ChatResponse::Summary { content } => {
                if !content.trim().is_empty() {
                    tracing::info!(message = %content, "Agent Completion Response");
                    let rendered = self.markdown.render(&content);
                    self.writeln(rendered)?;
                }
            }
            ChatResponse::ToolCallStart(_) => {
                self.spinner.stop(None)?;
            }
            ChatResponse::ToolCallEnd(toolcall_result) => {
                // Only track toolcall name in case of success else track the error.
                let payload = if toolcall_result.is_error() {
                    let mut r = ToolCallPayload::new(toolcall_result.name.to_string());
                    if let Some(cause) = toolcall_result.output.as_str() {
                        r = r.with_cause(cause.to_string());
                    }
                    r
                } else {
                    ToolCallPayload::new(toolcall_result.name.to_string())
                };
                tracker::tool_call(payload);

                self.spinner.start(None)?;
                if !self.cli.verbose {
                    return Ok(());
                }
            }
            ChatResponse::Usage(mut usage) => {
                // accumulate the cost
                usage.cost = usage
                    .cost
                    .map(|cost| cost + self.state.usage.cost.as_ref().map_or(0.0, |c| *c));
                self.state.usage = usage;
            }
            ChatResponse::RetryAttempt { cause, duration: _ } => {
                if !self.api.environment().retry_config.suppress_retry_errors {
                    self.spinner.start(Some("Retrying"))?;
                    self.writeln(TitleFormat::error(cause.as_str()))?;
                }
            }
            ChatResponse::Interrupt { reason } => {
                self.spinner.stop(None)?;

                let title = match reason {
                    InterruptionReason::MaxRequestPerTurnLimitReached { limit } => {
                        format!("Maximum request ({limit}) per turn achieved")
                    }
                    InterruptionReason::MaxToolFailurePerTurnLimitReached { limit } => {
                        format!("Maximum tool failure limit ({limit}) reached for this turn")
                    }
                };

                self.writeln(TitleFormat::action(title))?;
                self.should_continue().await?;
            }
            ChatResponse::Reasoning { content } => {
                if !content.trim().is_empty() {
                    self.writeln(content.dimmed())?;
                }
            }
        }
        Ok(())
    }

    async fn should_continue(&mut self) -> anyhow::Result<()> {
        let should_continue = ForgeSelect::confirm("Do you want to continue anyway?")
            .with_default(true)
            .prompt()?;

        if should_continue.unwrap_or(false) {
            self.spinner.start(None)?;
            Box::pin(self.on_message(None)).await?;
        }

        Ok(())
    }

    fn update_model(&mut self, model: ModelId) {
        tracker::set_model(model.to_string());
        self.state.model = Some(model);
    }

    async fn on_custom_event(&mut self, event: Event) -> Result<()> {
        let conversation_id = self.init_conversation().await?;
        let chat = ChatRequest::new(event, conversation_id);
        self.on_chat(chat).await
    }

    async fn update_mcp_config(&self, scope: &Scope, f: impl FnOnce(&mut McpConfig)) -> Result<()> {
        let mut config = self.api.read_mcp_config().await?;
        f(&mut config);
        self.api.write_mcp_config(scope, &config).await?;

        Ok(())
    }

    fn trace_user(&self) {
        let api = self.api.clone();
        // NOTE: Spawning required so that we don't block the user while querying user
        // info
        tokio::spawn(async move {
            if let Ok(Some(user_info)) = api.user_info().await {
                tracker::login(user_info.auth_provider_id.into_string());
            }
        });
    }
}

fn parse_env(env: Vec<String>) -> BTreeMap<String, String> {
    env.into_iter()
        .filter_map(|s| {
            let mut parts = s.splitn(2, '=');
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                Some((key.to_string(), value.to_string()))
            } else {
                None
            }
        })
        .collect()
}

struct CliModel(Model);

impl Display for CliModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.id)?;

        let mut info_parts = Vec::new();

        // Add context length if available
        if let Some(limit) = self.0.context_length {
            if limit >= 1_000_000 {
                info_parts.push(format!("{}M", limit / 1_000_000));
            } else if limit >= 1000 {
                info_parts.push(format!("{}k", limit / 1000));
            } else {
                info_parts.push(format!("{limit}"));
            }
        }

        // Add tools support indicator if explicitly supported
        if self.0.tools_supported == Some(true) {
            info_parts.push("🛠️".to_string());
        }

        // Only show brackets if we have info to display
        if !info_parts.is_empty() {
            let info = format!("[ {} ]", info_parts.join(" "));
            write!(f, " {}", info.dimmed())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use console::strip_ansi_codes;
    use forge_domain::{Model, ModelId};
    use pretty_assertions::assert_eq;

    use super::*;

    fn create_model_fixture(
        id: &str,
        context_length: Option<u64>,
        tools_supported: Option<bool>,
    ) -> Model {
        Model {
            id: ModelId::new(id),
            name: None,
            description: None,
            context_length,
            tools_supported,
            supports_parallel_tool_calls: None,
            supports_reasoning: None,
        }
    }

    #[test]
    fn test_cli_model_display_with_context_and_tools() {
        let fixture = create_model_fixture("gpt-4", Some(128000), Some(true));
        let formatted = format!("{}", CliModel(fixture));
        let actual = strip_ansi_codes(&formatted);
        let expected = "gpt-4 [ 128k 🛠️ ]";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cli_model_display_with_large_context() {
        let fixture = create_model_fixture("claude-3", Some(2000000), Some(true));
        let formatted = format!("{}", CliModel(fixture));
        let actual = strip_ansi_codes(&formatted);
        let expected = "claude-3 [ 2M 🛠️ ]";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cli_model_display_with_small_context() {
        let fixture = create_model_fixture("small-model", Some(512), Some(false));
        let formatted = format!("{}", CliModel(fixture));
        let actual = strip_ansi_codes(&formatted);
        let expected = "small-model [ 512 ]";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cli_model_display_with_context_only() {
        let fixture = create_model_fixture("text-model", Some(4096), Some(false));
        let formatted = format!("{}", CliModel(fixture));
        let actual = strip_ansi_codes(&formatted);
        let expected = "text-model [ 4k ]";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cli_model_display_with_tools_only() {
        let fixture = create_model_fixture("tool-model", None, Some(true));
        let formatted = format!("{}", CliModel(fixture));
        let actual = strip_ansi_codes(&formatted);
        let expected = "tool-model [ 🛠️ ]";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cli_model_display_empty_context_and_no_tools() {
        let fixture = create_model_fixture("basic-model", None, Some(false));
        let formatted = format!("{}", CliModel(fixture));
        let actual = strip_ansi_codes(&formatted);
        let expected = "basic-model";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cli_model_display_empty_context_and_none_tools() {
        let fixture = create_model_fixture("unknown-model", None, None);
        let formatted = format!("{}", CliModel(fixture));
        let actual = strip_ansi_codes(&formatted);
        let expected = "unknown-model";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cli_model_display_exact_thousands() {
        let fixture = create_model_fixture("exact-k", Some(8000), Some(true));
        let formatted = format!("{}", CliModel(fixture));
        let actual = strip_ansi_codes(&formatted);
        let expected = "exact-k [ 8k 🛠️ ]";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cli_model_display_exact_millions() {
        let fixture = create_model_fixture("exact-m", Some(1000000), Some(true));
        let formatted = format!("{}", CliModel(fixture));
        let actual = strip_ansi_codes(&formatted);
        let expected = "exact-m [ 1M 🛠️ ]";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cli_model_display_edge_case_999() {
        let fixture = create_model_fixture("edge-999", Some(999), None);
        let formatted = format!("{}", CliModel(fixture));
        let actual = strip_ansi_codes(&formatted);
        let expected = "edge-999 [ 999 ]";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cli_model_display_edge_case_1001() {
        let fixture = create_model_fixture("edge-1001", Some(1001), None);
        let formatted = format!("{}", CliModel(fixture));
        let actual = strip_ansi_codes(&formatted);
        let expected = "edge-1001 [ 1k ]";
        assert_eq!(actual, expected);
    }
}
