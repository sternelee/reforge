use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use console::style;
use forge_domain::{
    Agent, AgentInput, ChatResponse, ChatResponseContent, ToolCallContext, ToolCallFull,
    ToolDefinition, ToolName, ToolOutput, ToolResult, Tools, ToolsDiscriminants,
};
use futures::future::join_all;
use strum::IntoEnumIterator;
use tokio::time::timeout;

use crate::agent_executor::AgentExecutor;
use crate::dto::ToolsOverview;
use crate::error::Error;
use crate::mcp_executor::McpExecutor;
use crate::tool_executor::ToolExecutor;
use crate::{EnvironmentService, McpService, Services};

pub struct ToolRegistry<S> {
    tool_executor: ToolExecutor<S>,
    agent_executor: AgentExecutor<S>,
    mcp_executor: McpExecutor<S>,
    tool_timeout: Duration,
}

impl<S: Services> ToolRegistry<S> {
    pub fn new(services: Arc<S>) -> Self {
        Self {
            tool_executor: ToolExecutor::new(services.clone()),
            agent_executor: AgentExecutor::new(services.clone()),
            mcp_executor: McpExecutor::new(services.clone()),
            tool_timeout: Duration::from_secs(services.get_environment().tool_timeout),
        }
    }

    async fn call_with_timeout<F, Fut>(
        &self,
        tool_name: &ToolName,
        future: F,
    ) -> anyhow::Result<ToolOutput>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<ToolOutput>>,
    {
        timeout(self.tool_timeout, future())
            .await
            .context(Error::CallTimeout {
                timeout: self.tool_timeout.as_secs() / 60,
                tool_name: tool_name.clone(),
            })?
    }

    async fn call_inner(
        &self,
        agent: &Agent,
        input: ToolCallFull,
        context: &ToolCallContext,
    ) -> anyhow::Result<ToolOutput> {
        Self::validate_tool_call(agent, &input.name)?;

        tracing::info!(tool_name = %input.name, arguments = %input.arguments.clone().into_string(), "Executing tool call");
        let tool_name = input.name.clone();

        // First, try to call a Forge tool
        if Tools::contains(&input.name) {
            self.call_with_timeout(&tool_name, || self.tool_executor.execute(input, context))
                .await
        } else if self.agent_executor.contains_tool(&input.name).await? {
            // Handle agent delegation tool calls
            let agent_input = AgentInput::try_from(&input)?;
            let executor = self.agent_executor.clone();
            // NOTE: Agents should not timeout
            let outputs = join_all(
                agent_input
                    .tasks
                    .into_iter()
                    .map(|task| executor.execute(input.name.to_string(), task, context)),
            )
            .await
            .into_iter()
            .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(ToolOutput::from(outputs.into_iter()))
        } else if self.mcp_executor.contains_tool(&input.name).await? {
            let output = self
                .call_with_timeout(&tool_name, || self.mcp_executor.execute(input, context))
                .await?;
            let text = output
                .values
                .iter()
                .filter_map(|output| output.as_str())
                .fold(String::new(), |mut a, b| {
                    a.push('\n');
                    a.push_str(b);
                    a
                });
            if !text.trim().is_empty() {
                let text = style(text).cyan().dim().to_string();
                context
                    .send(ChatResponse::TaskMessage {
                        content: ChatResponseContent::PlainText(text),
                    })
                    .await?;
            }
            Ok(output)
        } else {
            Err(Error::NotFound(input.name).into())
        }
    }

    pub async fn call(
        &self,
        agent: &Agent,
        context: &ToolCallContext,
        call: ToolCallFull,
    ) -> ToolResult {
        let call_id = call.call_id.clone();
        let tool_name = call.name.clone();
        let output = self.call_inner(agent, call, context).await;

        ToolResult::new(tool_name).call_id(call_id).output(output)
    }

    pub async fn list(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        Ok(self.tools_overview().await?.into())
    }
    pub async fn tools_overview(&self) -> anyhow::Result<ToolsOverview> {
        let mcp_tools = self.mcp_executor.services.list().await?;
        let agent_tools = self.agent_executor.agent_definitions().await?;

        let system_tools = Tools::iter()
            .map(|tool| tool.definition())
            .collect::<Vec<_>>();

        Ok(ToolsOverview::new()
            .system(system_tools)
            .agents(agent_tools)
            .mcp(mcp_tools))
    }
}

impl<S> ToolRegistry<S> {
    /// Validates if a tool is supported by both the agent and the system.
    ///
    /// # Validation Process
    /// Verifies the tool is supported by the agent specified in the context
    fn validate_tool_call(agent: &Agent, tool_name: &ToolName) -> Result<(), Error> {
        let agent_tools: Vec<_> = agent
            .tools
            .iter()
            .flat_map(|tools| tools.iter())
            .map(|tool| tool.as_str())
            .collect();

        if !agent_tools.contains(&tool_name.as_str())
            && *tool_name != ToolsDiscriminants::AttemptCompletion.name()
        {
            tracing::error!(tool_name = %tool_name, "No tool with name");

            return Err(Error::NotAllowed {
                name: tool_name.clone(),
                supported_tools: agent_tools.join(", "),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use forge_domain::{Agent, AgentId, ToolName, Tools, ToolsDiscriminants};
    use pretty_assertions::assert_eq;

    use crate::tool_registry::ToolRegistry;

    fn agent() -> Agent {
        // only allow read and search tools for this agent
        Agent::new(AgentId::new("test_agent"))
            .tools(vec![ToolName::new("read"), ToolName::new("search")])
    }

    #[tokio::test]
    async fn test_restricted_tool_call() {
        let result = ToolRegistry::<()>::validate_tool_call(
            &agent(),
            &ToolName::new(Tools::Read(Default::default())),
        );
        assert!(result.is_ok(), "Tool call should be valid");
    }

    #[tokio::test]
    async fn test_restricted_tool_call_err() {
        let error = ToolRegistry::<()>::validate_tool_call(&agent(), &ToolName::new("write"))
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            "Tool 'write' is not available. Please try again with one of these tools: [read, search]"
        );
    }

    #[tokio::test]
    async fn test_completion_tool_call() {
        let result = ToolRegistry::<()>::validate_tool_call(
            &agent(),
            &ToolsDiscriminants::AttemptCompletion.name(),
        );

        assert!(result.is_ok(), "Completion tool call should be valid");
    }
}
