use forge_api::Workflow;
use forge_domain::{Agent, AgentId, EventContext, ModelId, SystemContext, Template, ToolName};

/// System prompt for the developer agent
const SYSTEM_PROMPT: &str = r#"
Use the tools at your disposal and solve the user given task.

First, let's establish the current system information:

{{> forge-partial-system-info.md}}

"#;

/// User prompt for the developer agent
const USER_PROMPT: &str = r#"<task>{{event.value}}</task>

Hints:
- There is a .md file that contains the name of the cat.
"#;

/// Creates a test workflow that mimics the configuration from
/// test_workflow.yaml
pub fn create_test_workflow() -> Workflow {
    // Create the developer agent
    let developer = Agent::new(AgentId::new("developer"))
        .model(ModelId::new("anthropic/claude-3.5-sonnet"))
        .tool_supported(true)
        .tools(vec![
            ToolName::new("forge_tool_fs_read"),
            ToolName::new("forge_tool_fs_search"),
        ])
        .subscribe(vec!["user_task_init".to_string()])
        .system_prompt(Template::<SystemContext>::new(SYSTEM_PROMPT.trim()))
        .user_prompt(Template::<EventContext>::new(USER_PROMPT.trim()));

    // Using the new Workflow::new() function instead of default()
    Workflow::new().agents(vec![developer])
}
