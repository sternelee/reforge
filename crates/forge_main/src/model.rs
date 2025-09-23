use std::sync::{Arc, Mutex};

use forge_api::{Model, Workflow};
use forge_domain::Agent;
use strum::{EnumProperty, IntoEnumIterator};
use strum_macros::{EnumIter, EnumProperty};

use crate::info::Info;
use crate::ui::PartialEvent;

/// Result of agent command registration
#[derive(Debug, Clone)]
pub struct AgentCommandRegistrationResult {
    pub registered_count: usize,
    pub skipped_conflicts: Vec<String>,
}

fn humanize_context_length(length: u64) -> String {
    if length >= 1_000_000 {
        format!("{:.1}M context", length as f64 / 1_000_000.0)
    } else if length >= 1_000 {
        format!("{:.1}K context", length as f64 / 1_000.0)
    } else {
        format!("{length} context")
    }
}

impl From<&[Model]> for Info {
    fn from(models: &[Model]) -> Self {
        let mut info = Info::new();

        for model in models.iter() {
            if let Some(context_length) = model.context_length {
                info = info.add_key_value(&model.id, humanize_context_length(context_length));
            } else {
                info = info.add_key(&model.id);
            }
        }

        info
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeCommand {
    pub name: String,
    pub description: String,
    pub value: Option<String>,
}

impl From<&Workflow> for ForgeCommandManager {
    fn from(value: &Workflow) -> Self {
        let cmd = ForgeCommandManager::default();
        cmd.register_all(value);
        cmd
    }
}

#[derive(Debug)]
pub struct ForgeCommandManager {
    commands: Arc<Mutex<Vec<ForgeCommand>>>,
}

impl Default for ForgeCommandManager {
    fn default() -> Self {
        let commands = Self::default_commands();
        ForgeCommandManager { commands: Arc::new(Mutex::new(commands)) }
    }
}

impl ForgeCommandManager {
    /// Sanitizes agent ID to create a valid command name
    /// Replaces spaces and special characters with hyphens
    fn sanitize_agent_id(agent_id: &str) -> String {
        agent_id
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<&str>>()
            .join("-")
    }

    /// Checks if a command name conflicts with built-in commands
    fn is_reserved_command(name: &str) -> bool {
        matches!(
            name,
            "/agent"
                | "/forge"
                | "/muse"
                | "/sage"
                | "/help"
                | "/compact"
                | "/new"
                | "/info"
                | "/usage"
                | "/exit"
                | "/update"
                | "/dump"
                | "/model"
                | "/tools"
                | "/login"
                | "/logout"
                | "/retry"
                | "/conversations"
                | "/list"
        )
    }

    fn default_commands() -> Vec<ForgeCommand> {
        Command::iter()
            .filter(|command| !matches!(command, Command::Message(_)))
            .filter(|command| !matches!(command, Command::Custom(_)))
            .filter(|command| !matches!(command, Command::Shell(_)))
            .filter(|command| !matches!(command, Command::AgentSwitch(_)))
            .map(|command| ForgeCommand {
                name: command.name().to_string(),
                description: command.usage().to_string(),
                value: None,
            })
            .collect::<Vec<_>>()
    }

    /// Registers multiple commands to the manager.
    pub fn register_all(&self, workflow: &Workflow) {
        let mut guard = self.commands.lock().unwrap();
        let mut commands = Self::default_commands();

        commands.sort_by(|a, b| a.name.cmp(&b.name));

        commands.extend(workflow.commands.clone().into_iter().map(|cmd| {
            let name = format!("/{}", cmd.name);
            let description = format!("⚙ {}", cmd.description);
            let value = cmd.prompt.clone();

            ForgeCommand { name, description, value }
        }));

        *guard = commands;
    }

    /// Registers agent commands to the manager.
    /// Returns information about the registration process.
    pub fn register_agent_commands(&self, agents: Vec<Agent>) -> AgentCommandRegistrationResult {
        let mut guard = self.commands.lock().unwrap();
        let mut result =
            AgentCommandRegistrationResult { registered_count: 0, skipped_conflicts: Vec::new() };

        // Remove existing agent commands (commands starting with "/agent-")
        guard.retain(|cmd| !cmd.name.starts_with("/agent-"));

        // Add new agent commands
        for agent in agents {
            let agent_id_str = agent.id.as_str();
            let sanitized_id = Self::sanitize_agent_id(agent_id_str);
            let command_name = format!("/agent-{}", sanitized_id);

            // Skip if it would conflict with reserved commands
            if Self::is_reserved_command(&command_name) {
                result.skipped_conflicts.push(command_name);
                continue;
            }

            let default_title = agent_id_str.to_string();
            let title = agent.title.as_ref().unwrap_or(&default_title);
            let description = format!("🤖 Switch to {} agent", title);

            guard.push(ForgeCommand {
                name: command_name,
                description,
                value: Some(agent_id_str.to_string()),
            });

            result.registered_count += 1;
        }

        // Sort commands for consistent completion behavior
        guard.sort_by(|a, b| a.name.cmp(&b.name));

        result
    }

    /// Finds a command by name.
    fn find(&self, command: &str) -> Option<ForgeCommand> {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.name == command)
            .cloned()
    }

    /// Lists all registered commands.
    pub fn list(&self) -> Vec<ForgeCommand> {
        self.commands.lock().unwrap().clone()
    }

    /// Extracts the command value from the input parts
    ///
    /// # Arguments
    /// * `command` - The command for which to extract the value
    /// * `parts` - The parts of the command input after the command name
    ///
    /// # Returns
    /// * `Option<String>` - The extracted value, if any
    fn extract_command_value(&self, command: &ForgeCommand, parts: &[&str]) -> Option<String> {
        // Unit tests implemented in the test module below

        // Try to get value provided in the command
        let value_provided = if !parts.is_empty() {
            Some(parts.join(" "))
        } else {
            None
        };

        // Try to get default value from command definition
        let value_default = self
            .commands
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.name == command.name)
            .and_then(|cmd| cmd.value.clone());

        // Use provided value if non-empty, otherwise use default
        match value_provided {
            Some(value) if !value.trim().is_empty() => Some(value),
            _ => value_default,
        }
    }

    pub fn parse(&self, input: &str) -> anyhow::Result<Command> {
        // Check if it's a shell command (starts with !)
        if input.trim().starts_with("!") {
            return Ok(Command::Shell(
                input
                    .strip_prefix("!")
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            ));
        }

        let mut tokens = input.trim().split_ascii_whitespace();
        let command = tokens.next().unwrap();
        let parameters = tokens.collect::<Vec<_>>();

        // Check if it's a system command (starts with /)
        let is_command = command.starts_with("/");
        if !is_command {
            return Ok(Command::Message(input.to_string()));
        }

        // TODO: Can leverage Clap to parse commands and provide correct error messages
        match command {
            "/compact" => Ok(Command::Compact),
            "/new" => Ok(Command::New),
            "/info" => Ok(Command::Info),
            "/usage" => Ok(Command::Usage),
            "/exit" => Ok(Command::Exit),
            "/update" => Ok(Command::Update),
            "/dump" => {
                if !parameters.is_empty() && parameters[0] == "html" {
                    Ok(Command::Dump(Some("html".to_string())))
                } else {
                    Ok(Command::Dump(None))
                }
            }
            "/act" | "/forge" => Ok(Command::Forge),
            "/plan" | "/muse" => Ok(Command::Muse),
            "/sage" => Ok(Command::Sage),
            "/help" => Ok(Command::Help),
            "/model" => Ok(Command::Model),
            "/tools" => Ok(Command::Tools),
            "/agent" => Ok(Command::Agent),
            "/login" => Ok(Command::Login),
            "/logout" => Ok(Command::Logout),
            "/retry" => Ok(Command::Retry),
            "/provider" => Ok(Command::Provider),
            "/conversations" | "/list" => Ok(Command::Conversations),
            text => {
                let parts = text.split_ascii_whitespace().collect::<Vec<&str>>();

                if let Some(command) = parts.first() {
                    // Check if it's an agent command pattern (/agent-*)
                    if command.starts_with("/agent-") {
                        if let Some(found_command) = self.find(command) {
                            // Extract the agent ID from the command value
                            if let Some(agent_id) = &found_command.value {
                                return Ok(Command::AgentSwitch(agent_id.clone()));
                            }
                        }
                        return Err(anyhow::anyhow!("{} is not a valid agent command", command));
                    }

                    // Handle custom workflow commands
                    if let Some(command) = self.find(command) {
                        let value = self.extract_command_value(&command, &parts[1..]);

                        Ok(Command::Custom(PartialEvent::new(
                            command.name.clone().strip_prefix('/').unwrap().to_string(),
                            value.unwrap_or_default(),
                        )))
                    } else {
                        Err(anyhow::anyhow!("{} is not valid", command))
                    }
                } else {
                    Err(anyhow::anyhow!("Invalid Command Format."))
                }
            }
        }
    }
}

/// Represents user input types in the chat application.
///
/// This enum encapsulates all forms of input including:
/// - System commands (starting with '/')
/// - Regular chat messages
/// - File content
#[derive(Debug, Clone, PartialEq, Eq, EnumProperty, EnumIter)]
pub enum Command {
    /// Compact the conversation context. This can be triggered with the
    /// '/compact' command.
    #[strum(props(usage = "Compact the conversation context"))]
    Compact,
    /// Start a new conversation while preserving history.
    /// This can be triggered with the '/new' command.
    #[strum(props(usage = "Start a new conversation"))]
    New,
    /// A regular text message from the user to be processed by the chat system.
    /// Any input that doesn't start with '/' is treated as a message.
    #[strum(props(usage = "Send a regular message"))]
    Message(String),
    /// Display system environment information.
    /// This can be triggered with the '/info' command.
    #[strum(props(usage = "Display system information"))]
    Info,
    /// Display usage information (tokens & requests).
    #[strum(props(usage = "Shows usage information (tokens & requests)"))]
    Usage,
    /// Exit the application without any further action.
    #[strum(props(usage = "Exit the application"))]
    Exit,
    /// Updates the forge version
    #[strum(props(usage = "Updates to the latest compatible version of forge"))]
    Update,
    /// Switch to "forge" agent.
    /// This can be triggered with the '/forge' command.
    #[strum(props(usage = "Enable implementation mode with code changes"))]
    Forge,
    /// Switch to "muse" agent.
    /// This can be triggered with the '/must' command.
    #[strum(props(usage = "Enable planning mode without code changes"))]
    Muse,
    /// Switch to "sage" agent.
    /// This can be triggered with the '/sage' command.
    #[strum(props(
        usage = "Enable research mode for systematic codebase exploration and analysis"
    ))]
    Sage,
    /// Switch to "help" mode.
    /// This can be triggered with the '/help' command.
    #[strum(props(usage = "Enable help mode for tool questions"))]
    Help,
    /// Dumps the current conversation into a json file or html file
    #[strum(props(usage = "Save conversation as JSON or HTML (use /dump html for HTML format)"))]
    Dump(Option<String>),
    /// Switch or select the active model
    /// This can be triggered with the '/model' command.
    #[strum(props(usage = "Switch to a different model"))]
    Model,
    /// List all available tools with their descriptions and schema
    /// This can be triggered with the '/tools' command.
    #[strum(props(usage = "List all available tools with their descriptions and schema"))]
    Tools,
    /// Handles custom command defined in workflow file.
    Custom(PartialEvent),
    /// Executes a native shell command.
    /// This can be triggered with commands starting with '!' character.
    #[strum(props(usage = "Execute a native shell command"))]
    Shell(String),

    /// Allows user to switch the operating agent.
    #[strum(props(usage = "Switch to an agent interactively"))]
    Agent,

    /// Log into the default provider.
    #[strum(props(usage = "Log into the Forge provider"))]
    Login,

    /// Logs out of the current session.
    #[strum(props(usage = "Logout of the current session"))]
    Logout,

    /// Retry without modifying model context
    #[strum(props(usage = "Retry the last command"))]
    Retry,

    /// Select and configure a provider
    #[strum(props(usage = "Select and configure AI provider from supported list"))]
    Provider,
    /// List all conversations for the active workspace
    #[strum(props(usage = "List all conversations for the active workspace"))]
    Conversations,

    /// Switch directly to a specific agent by ID
    #[strum(props(usage = "Switch directly to a specific agent"))]
    AgentSwitch(String),
}

impl Command {
    pub fn name(&self) -> &str {
        match self {
            Command::Compact => "/compact",
            Command::New => "/new",
            Command::Message(_) => "/message",
            Command::Update => "/update",
            Command::Info => "/info",
            Command::Usage => "/usage",
            Command::Exit => "/exit",
            Command::Forge => "/forge",
            Command::Muse => "/muse",
            Command::Sage => "/sage",
            Command::Help => "/help",
            Command::Dump(_) => "/dump",
            Command::Model => "/model",
            Command::Tools => "/tools",
            Command::Custom(event) => &event.name,
            Command::Shell(_) => "!shell",
            Command::Agent => "/agent",
            Command::Login => "/login",
            Command::Logout => "/logout",
            Command::Retry => "/retry",
            Command::Provider => "/provider",
            Command::Conversations => "/conversations",
            Command::AgentSwitch(agent_id) => agent_id,
        }
    }

    /// Returns the usage description for the command.
    pub fn usage(&self) -> &str {
        self.get_str("usage").unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_command_value_with_provided_value() {
        // Setup
        let cmd_manager = ForgeCommandManager::default();
        let command = ForgeCommand {
            name: String::from("/test"),
            description: String::from("Test command"),
            value: None,
        };
        let parts = vec!["arg1", "arg2"];

        // Execute
        let result = cmd_manager.extract_command_value(&command, &parts);

        // Verify
        assert_eq!(result, Some(String::from("arg1 arg2")));
    }

    #[test]
    fn test_extract_command_value_with_empty_parts_default_value() {
        // Setup
        let cmd_manager = ForgeCommandManager {
            commands: Arc::new(Mutex::new(vec![ForgeCommand {
                name: String::from("/test"),
                description: String::from("Test command"),
                value: Some(String::from("default_value")),
            }])),
        };
        let command = ForgeCommand {
            name: String::from("/test"),
            description: String::from("Test command"),
            value: None,
        };
        let parts: Vec<&str> = vec![];

        // Execute
        let result = cmd_manager.extract_command_value(&command, &parts);

        // Verify
        assert_eq!(result, Some(String::from("default_value")));
    }

    #[test]
    fn test_extract_command_value_with_empty_string_parts() {
        // Setup
        let cmd_manager = ForgeCommandManager {
            commands: Arc::new(Mutex::new(vec![ForgeCommand {
                name: String::from("/test"),
                description: String::from("Test command"),
                value: Some(String::from("default_value")),
            }])),
        };
        let command = ForgeCommand {
            name: String::from("/test"),
            description: String::from("Test command"),
            value: None,
        };
        let parts = vec![""];

        // Execute
        let result = cmd_manager.extract_command_value(&command, &parts);

        // Verify - should use default as the provided value is empty
        assert_eq!(result, Some(String::from("default_value")));
    }

    #[test]
    fn test_extract_command_value_with_whitespace_parts() {
        // Setup
        let cmd_manager = ForgeCommandManager {
            commands: Arc::new(Mutex::new(vec![ForgeCommand {
                name: String::from("/test"),
                description: String::from("Test command"),
                value: Some(String::from("default_value")),
            }])),
        };
        let command = ForgeCommand {
            name: String::from("/test"),
            description: String::from("Test command"),
            value: None,
        };
        let parts = vec!["  "];

        // Execute
        let result = cmd_manager.extract_command_value(&command, &parts);

        // Verify - should use default as the provided value is just whitespace
        assert_eq!(result, Some(String::from("default_value")));
    }

    #[test]
    fn test_extract_command_value_no_default_no_provided() {
        // Setup
        let cmd_manager = ForgeCommandManager {
            commands: Arc::new(Mutex::new(vec![ForgeCommand {
                name: String::from("/test"),
                description: String::from("Test command"),
                value: None,
            }])),
        };
        let command = ForgeCommand {
            name: String::from("/test"),
            description: String::from("Test command"),
            value: None,
        };
        let parts: Vec<&str> = vec![];

        // Execute
        let result = cmd_manager.extract_command_value(&command, &parts);

        // Verify - should be None as there's no default and no provided value
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_command_value_provided_overrides_default() {
        // Setup
        let cmd_manager = ForgeCommandManager {
            commands: Arc::new(Mutex::new(vec![ForgeCommand {
                name: String::from("/test"),
                description: String::from("Test command"),
                value: Some(String::from("default_value")),
            }])),
        };
        let command = ForgeCommand {
            name: String::from("/test"),
            description: String::from("Test command"),
            value: None,
        };
        let parts = vec!["provided_value"];

        // Execute
        let result = cmd_manager.extract_command_value(&command, &parts);

        // Verify - provided value should override default
        assert_eq!(result, Some(String::from("provided_value")));
    }
    #[test]
    fn test_parse_shell_command() {
        // Setup
        let cmd_manager = ForgeCommandManager::default();

        // Execute
        let result = cmd_manager.parse("!ls -la").unwrap();

        // Verify
        match result {
            Command::Shell(cmd) => assert_eq!(cmd, "ls -la"),
            _ => panic!("Expected Shell command, got {result:?}"),
        }
    }

    #[test]
    fn test_parse_shell_command_empty() {
        // Setup
        let cmd_manager = ForgeCommandManager::default();

        // Execute
        let result = cmd_manager.parse("!").unwrap();

        // Verify
        match result {
            Command::Shell(cmd) => assert_eq!(cmd, ""),
            _ => panic!("Expected Shell command, got {result:?}"),
        }
    }

    #[test]
    fn test_parse_shell_command_with_whitespace() {
        // Setup
        let cmd_manager = ForgeCommandManager::default();

        // Execute
        let result = cmd_manager.parse("!   echo 'test'   ").unwrap();

        // Verify
        match result {
            Command::Shell(cmd) => assert_eq!(cmd, "echo 'test'"),
            _ => panic!("Expected Shell command, got {result:?}"),
        }
    }

    #[test]
    fn test_shell_command_not_in_default_commands() {
        // Setup
        let manager = ForgeCommandManager::default();
        let commands = manager.list();

        // The shell command should not be included
        let contains_shell = commands.iter().any(|cmd| cmd.name == "!shell");
        assert!(
            !contains_shell,
            "Shell command should not be in default commands"
        );
    }
    #[test]
    fn test_parse_list_command() {
        // Setup
        let cmd_manager = ForgeCommandManager::default();

        // Execute
        let result = cmd_manager.parse("/conversations").unwrap();

        // Verify
        match result {
            Command::Conversations => {
                // Command parsed correctly
            }
            _ => panic!("Expected List command, got {result:?}"),
        }
    }

    #[test]
    fn test_list_command_in_default_commands() {
        // Setup
        let manager = ForgeCommandManager::default();
        let commands = manager.list();

        // The list command should be included
        let contains_list = commands.iter().any(|cmd| cmd.name == "/conversations");
        assert!(
            contains_list,
            "Conversations command should be in default commands"
        );
    }

    #[test]
    fn test_sanitize_agent_id_basic() {
        // Test basic sanitization
        let fixture = "test-agent";
        let actual = ForgeCommandManager::sanitize_agent_id(fixture);
        let expected = "test-agent";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_sanitize_agent_id_with_spaces() {
        // Test space replacement
        let fixture = "test agent name";
        let actual = ForgeCommandManager::sanitize_agent_id(fixture);
        let expected = "test-agent-name";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_sanitize_agent_id_with_special_chars() {
        // Test special character replacement
        let fixture = "test@agent#name!";
        let actual = ForgeCommandManager::sanitize_agent_id(fixture);
        let expected = "test-agent-name";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_sanitize_agent_id_uppercase() {
        // Test uppercase conversion
        let fixture = "TestAgent";
        let actual = ForgeCommandManager::sanitize_agent_id(fixture);
        let expected = "testagent";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_is_reserved_command() {
        // Test reserved commands
        assert!(ForgeCommandManager::is_reserved_command("/agent"));
        assert!(ForgeCommandManager::is_reserved_command("/forge"));
        assert!(ForgeCommandManager::is_reserved_command("/muse"));
        assert!(!ForgeCommandManager::is_reserved_command("/agent-custom"));
        assert!(!ForgeCommandManager::is_reserved_command("/custom"));
    }

    #[test]
    fn test_register_agent_commands() {
        use forge_domain::Agent;

        // Setup
        let fixture = ForgeCommandManager::default();
        let agents = vec![
            Agent::new("test-agent").title("Test Agent".to_string()),
            Agent::new("another").title("Another Agent".to_string()),
        ];

        // Execute
        let result = fixture.register_agent_commands(agents);

        // Verify result
        assert_eq!(result.registered_count, 2);
        assert_eq!(result.skipped_conflicts.len(), 0);

        // Verify
        let commands = fixture.list();
        let agent_commands: Vec<_> = commands
            .iter()
            .filter(|cmd| cmd.name.starts_with("/agent-"))
            .collect();

        assert_eq!(agent_commands.len(), 2);
        assert!(
            agent_commands
                .iter()
                .any(|cmd| cmd.name == "/agent-test-agent")
        );
        assert!(
            agent_commands
                .iter()
                .any(|cmd| cmd.name == "/agent-another")
        );
    }

    #[test]
    fn test_parse_agent_switch_command() {
        use forge_domain::Agent;

        // Setup
        let fixture = ForgeCommandManager::default();
        let agents = vec![Agent::new("test-agent").title("Test Agent".to_string())];
        let _result = fixture.register_agent_commands(agents);

        // Execute
        let actual = fixture.parse("/agent-test-agent").unwrap();

        // Verify
        match actual {
            Command::AgentSwitch(agent_id) => assert_eq!(agent_id, "test-agent"),
            _ => panic!("Expected AgentSwitch command, got {actual:?}"),
        }
    }

    #[test]
    fn test_parse_invalid_agent_command() {
        // Setup
        let fixture = ForgeCommandManager::default();

        // Execute
        let result = fixture.parse("/agent-nonexistent");

        // Verify
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not a valid agent command")
        );
    }
}
