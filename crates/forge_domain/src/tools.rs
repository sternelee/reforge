#![allow(clippy::enum_variant_names)]
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use convert_case::{Case, Casing};
use derive_more::From;
use eserde::Deserialize;
use forge_tool_macros::ToolDescription;
use schemars::JsonSchema;
use schemars::schema::RootSchema;
use serde::Serialize;
use serde_json::Map;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumDiscriminants, EnumIter};

use crate::{ToolCallFull, ToolDefinition, ToolDescription, ToolName};

/// Enum representing all possible tool input types.
///
/// This enum contains variants for each type of input that can be passed to
/// tools in the application. Each variant corresponds to the input type for a
/// specific tool.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    JsonSchema,
    From,
    EnumIter,
    Display,
    PartialEq,
    EnumDiscriminants,
)]
#[strum_discriminants(derive(Display))]
#[serde(tag = "name", content = "arguments", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum Tools {
    Read(FSRead),
    Write(FSWrite),
    Search(FSSearch),
    Remove(FSRemove),
    Patch(FSPatch),
    Undo(FSUndo),
    Shell(Shell),
    Fetch(NetFetch),
    Followup(Followup),
    AttemptCompletion(AttemptCompletion),
    Plan(PlanCreate),
}

/// Input structure for agent tool calls. This serves as the generic schema
/// for dynamically registered agent tools, allowing users to specify tasks
/// for specific agents.
#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentInput {
    /// A list of clear and detailed descriptions of the tasks to be performed
    /// by the agent in parallel. Provide sufficient context and specific
    /// requirements to enable the agent to understand and execute the work
    /// accurately.
    pub tasks: Vec<String>,
    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Reads file contents from the specified absolute path. Ideal for analyzing
/// code, configuration files, documentation, or textual data. Returns the
/// content as a string with line number prefixes by default. For files larger
/// than 2,000 lines, the tool automatically returns only the first 2,000 lines.
/// You should always rely on this default behavior and avoid specifying custom
/// ranges unless absolutely necessary. If needed, specify a range with the
/// start_line and end_line parameters, ensuring the total range does not exceed
/// 2,000 lines. Specifying a range exceeding this limit will result in an
/// error. Binary files are automatically detected and rejected.
#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct FSRead {
    /// The path of the file to read, always provide absolute paths.
    pub path: String,

    /// Optional start position in lines (1-based). If provided, reading
    /// will start from this line position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<i32>,

    /// If true, prefixes each line with its line index (starting at 1).
    /// Defaults to true.
    #[serde(default = "default_true")]
    pub show_line_numbers: bool,

    /// Optional end position in lines (inclusive). If provided, reading
    /// will end at this line position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i32>,
    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Use it to create a new file at a specified path with the provided content.
/// Always provide absolute paths for file locations. The tool
/// automatically handles the creation of any missing intermediary directories
/// in the specified path.
/// IMPORTANT: DO NOT attempt to use this tool to move or rename files, use the
/// shell tool instead.
#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct FSWrite {
    /// The path of the file to write to (absolute path required)
    pub path: String,

    /// The content to write to the file. ALWAYS provide the COMPLETE intended
    /// content of the file, without any truncation or omissions. You MUST
    /// include ALL parts of the file, even if they haven't been modified.
    pub content: String,

    /// If set to true, existing files will be overwritten. If not set and the
    /// file exists, an error will be returned with the content of the
    /// existing file.
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub overwrite: bool,
    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Recursively searches directories for files by content (regex) and/or name
/// (glob pattern). Provides context-rich results with line numbers for content
/// matches. Two modes: content search (when regex provided) or file finder
/// (when regex omitted). Uses case-insensitive Rust regex syntax. Requires
/// absolute paths. Avoids binary files and excluded directories. Best for code
/// exploration, API usage discovery, configuration settings, or finding
/// patterns across projects. For large pages, returns the first 200
/// lines and stores the complete content in a temporary file for
/// subsequent access.
#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct FSSearch {
    /// The absolute path of the directory or file to search in. If it's a
    /// directory, it will be searched recursively. If it's a file path,
    /// only that specific file will be searched.
    pub path: String,

    /// The regular expression pattern to search for in file contents. Uses Rust
    /// regex syntax. If not provided, only file name matching will be
    /// performed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regex: Option<String>,

    /// Starting index for the search results (1-based).
    pub start_index: Option<i32>,

    /// Maximum number of lines to return in the search results.
    pub max_search_lines: Option<i32>,

    /// Glob pattern to filter files (e.g., '*.ts' for TypeScript files).
    /// If not provided, it will search all files (*).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_pattern: Option<String>,
    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Request to remove a file at the specified path. Use this when you need to
/// delete an existing file. The path must be absolute. This operation cannot
/// be undone, so use it carefully.
#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct FSRemove {
    /// The path of the file to remove (absolute path required)
    pub path: String,
    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Operation types that can be performed on matched text
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, AsRefStr, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum PatchOperation {
    /// Prepend content before the matched text
    #[default]
    Prepend,

    /// Append content after the matched text
    Append,

    /// Should be used only when you want to replace the first occurrence.
    /// Use only for specific, targeted replacements where you need to modify
    /// just the first match.
    Replace,

    /// Should be used for renaming variables, functions, types, or any
    /// widespread replacements across the file. This is the recommended
    /// choice for consistent refactoring operations as it ensures all
    /// occurrences are updated.
    ReplaceAll,

    /// Swap the matched text with another text (search for the second text and
    /// swap them)
    Swap,
}

// TODO: do the Blanket impl for all the unit enums
impl JsonSchema for PatchOperation {
    fn schema_name() -> String {
        std::any::type_name::<Self>()
            .split("::")
            .last()
            .unwrap_or("PatchOperation")
            .to_string()
    }

    fn json_schema(_gen: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        use schemars::schema::{InstanceType, Schema, SchemaObject};
        let variants: Vec<serde_json::Value> = Self::iter()
            .map(|variant| variant.as_ref().to_case(Case::Snake).into())
            .collect();
        Schema::Object(SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            enum_values: Some(variants),
            metadata: Some(Box::new(schemars::schema::Metadata {
                ..Default::default()
            })),
            ..Default::default()
        })
    }
}

/// Modifies files with targeted line operations on matched patterns. Supports
/// prepend, append, replace, replace_all, swap, delete
/// operations. Ideal for precise changes to configs, code, or docs while
/// preserving context. Not suitable for complex refactoring or modifying all
/// pattern occurrences - use `write` instead for complete
/// rewrites and `undo` for undoing the last operation. Fails if
/// search pattern isn't found.
#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct FSPatch {
    /// The path to the file to modify
    pub path: String,

    /// The exact line to search for in the file. When
    /// skipped the patch operation applies to the entire content. `Append` adds
    /// the new content to the end, `Prepend` adds it to the beginning, and
    /// `Replace` fully overwrites the original content. `Swap` requires a
    /// search target, so without one, it makes no changes.
    pub search: Option<String>,

    /// The operation to perform on the matched text. Possible options are:
    /// - 'prepend': Add content before the matched text
    /// - 'append': Add content after the matched text
    /// - 'replace': Use only for specific, targeted replacements where you need
    ///   to modify just the first match.
    /// - 'replace_all': Should be used for renaming variables, functions,
    ///   types, or any widespread replacements across the file. This is the
    ///   recommended choice for consistent refactoring operations as it ensures
    ///   all occurrences are updated.
    /// - 'swap': Replace the matched text with another text (search for the
    ///   second text and swap them)
    pub operation: PatchOperation,

    /// The content to use for the operation (replacement text, line to
    /// prepend/append, or target line for swap operations)
    pub content: String,

    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Reverts the most recent file operation (create/modify/delete) on a specific
/// file. Use this tool when you need to recover from incorrect file changes or
/// if a revert is requested by the user.
#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct FSUndo {
    /// The absolute path of the file to revert to its previous state.
    pub path: String,
    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Executes shell commands with safety measures using restricted bash (rbash).
/// Prevents potentially harmful operations like absolute path execution and
/// directory changes. Use for file system interaction, running utilities,
/// installing packages, or executing build commands. For operations requiring
/// unrestricted access, advise users to run forge CLI with '-u' flag. Returns
/// complete output including stdout, stderr, and exit code for diagnostic
/// purposes.
#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct Shell {
    /// The shell command to execute.
    pub command: String,

    /// The working directory where the command should be executed.
    pub cwd: PathBuf,

    /// Whether to preserve ANSI escape codes in the output.
    /// If true, ANSI escape codes will be preserved in the output.
    /// If false (default), ANSI escape codes will be stripped from the output.
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub keep_ansi: bool,
    /// Environment variable names to pass to command execution (e.g., ["PATH",
    /// "HOME", "USER"]). The system automatically reads the specified
    /// values and applies them during command execution.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<String>>,

    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Input type for the net fetch tool
#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct NetFetch {
    /// URL to fetch
    pub url: String,

    /// Get raw content without any markdown conversion (default: false)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<bool>,

    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Use this tool when you encounter ambiguities, need clarification, or require
/// more details to proceed effectively. Use this tool judiciously to maintain a
/// balance between gathering necessary information and avoiding excessive
/// back-and-forth.
#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct Followup {
    /// Question to ask the user
    pub question: String,

    /// If true, allows selecting multiple options; if false (default), only one
    /// option can be selected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiple: Option<bool>,

    /// First option to choose from
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option1: Option<String>,

    /// Second option to choose from
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option2: Option<String>,

    /// Third option to choose from
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option3: Option<String>,

    /// Fourth option to choose from
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option4: Option<String>,

    /// Fifth option to choose from
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option5: Option<String>,

    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

/// After each tool use, the user will respond with the result of
/// that tool use, i.e. if it succeeded or failed, along with any reasons for
/// failure. Once you've received the results of tool uses and can confirm that
/// the task is complete, use this tool to present the result of your work to
/// the user in markdown format. The user may respond with feedback if they are
/// not satisfied with the result, which you can use to make improvements and
/// try again. IMPORTANT NOTE: This tool CANNOT be used until you've confirmed
/// from the user that any previous tool uses were successful. Failure to do so
/// will result in code corruption and system failure. Before using this tool,
/// you must ask yourself if you've confirmed from the user that any previous
/// tool uses were successful. If not, then DO NOT use this tool.
#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct AttemptCompletion {
    /// The result of the task. Formulate this result in a way that is final and
    /// does not require further input from the user. Don't end your result with
    /// questions or offers for further assistance.
    pub result: String,
}

/// Creates a new plan file with the specified name, version, and content. Use
/// this tool to create structured project plans, task breakdowns, or
/// implementation strategies that can be tracked and referenced throughout
/// development sessions.
#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct PlanCreate {
    /// The name of the plan (will be used in the filename)
    pub plan_name: String,

    /// The version of the plan (e.g., "v1", "v2", "1.0")
    pub version: String,

    /// The content to write to the plan file. This should be the complete
    /// plan content in markdown format.
    pub content: String,

    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

fn default_raw() -> Option<bool> {
    Some(false)
}

/// Retrieves content from URLs as markdown or raw text. Enables access to
/// current online information including websites, APIs and documentation. Use
/// for obtaining up-to-date information beyond training data, verifying facts,
/// or retrieving specific online content. Handles HTTP/HTTPS and converts HTML
/// to readable markdown by default. Cannot access private/restricted resources
/// requiring authentication. Respects robots.txt and may be blocked by
/// anti-scraping measures. For large pages, returns the first 40,000 characters
/// and stores the complete content in a temporary file for subsequent access.
#[derive(Default, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct FetchInput {
    /// URL to fetch
    pub url: String,
    /// Get raw content without any markdown conversion (default: false)
    #[serde(default = "default_raw")]
    pub raw: Option<bool>,
    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}
/// Request to list files and directories within the specified directory. If
/// recursive is true, it will list all files and directories recursively. If
/// recursive is false or not provided, it will only list the top-level
/// contents. The path must be absolute. Do not use this tool to confirm the
/// existence of files you may have created, as the user will let you know if
/// the files were created successfully or not.
#[derive(Default, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct FSListInput {
    /// The path of the directory to list contents for (absolute path required)
    pub path: String,
    /// Whether to list files recursively. Use true for recursive listing, false
    /// or omit for top-level only.
    pub recursive: Option<bool>,
    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Request to retrieve detailed metadata about a file or directory at the
/// specified path. Returns comprehensive information including size, creation
/// time, last modified time, permissions, and type. Path must be absolute. Use
/// this when you need to understand file characteristics without reading the
/// actual content.
#[derive(Default, Deserialize, JsonSchema, ToolDescription, PartialEq)]
pub struct FSFileInfoInput {
    /// The path of the file or directory to inspect (absolute path required)
    pub path: String,
    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UndoInput {
    /// The absolute path of the file to revert to its previous state. Must be
    /// the exact path that was previously modified, created, or deleted by
    /// a Forge file operation. If the file was deleted, provide the
    /// original path it had before deletion. The system requires a prior
    /// snapshot for this path.
    pub path: String,
    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Input for the select tool
#[derive(Deserialize, JsonSchema)]
pub struct SelectInput {
    /// Question to ask the user
    pub question: String,

    /// First option to choose from
    pub option1: Option<String>,

    /// Second option to choose from
    pub option2: Option<String>,

    /// Third option to choose from
    pub option3: Option<String>,

    /// Fourth option to choose from
    pub option4: Option<String>,

    /// Fifth option to choose from
    pub option5: Option<String>,

    /// If true, allows selecting multiple options; if false (default), only one
    /// option can be selected
    #[schemars(default)]
    pub multiple: Option<bool>,
    /// One sentence explanation as to why this specific tool is being used, and
    /// how it contributes to the goal.
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Helper function to check if a value equals its default value
fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

impl ToolDescription for Tools {
    fn description(&self) -> String {
        match self {
            Tools::Patch(v) => v.description(),
            Tools::Shell(v) => v.description(),
            Tools::Followup(v) => v.description(),
            Tools::Fetch(v) => v.description(),
            Tools::AttemptCompletion(v) => v.description(),
            Tools::Search(v) => v.description(),
            Tools::Read(v) => v.description(),
            Tools::Remove(v) => v.description(),
            Tools::Undo(v) => v.description(),
            Tools::Write(v) => v.description(),
            Tools::Plan(v) => v.description(),
        }
    }
}
lazy_static::lazy_static! {
    // Cache of all tool names
    static ref FORGE_TOOLS: HashSet<ToolName> = Tools::iter()
        .map(ToolName::new)
        .collect();
}

impl Tools {
    pub fn schema(&self) -> RootSchema {
        use schemars::r#gen::SchemaSettings;
        let r#gen = SchemaSettings::default()
            .with(|s| {
                // incase of null, add nullable property.
                s.option_nullable = true;
                // incase of option type, don't add null in type.
                s.option_add_null_type = false;
                s.meta_schema = None;
                s.inline_subschemas = true;
            })
            .into_generator();
        match self {
            Tools::Patch(_) => r#gen.into_root_schema_for::<FSPatch>(),
            Tools::Shell(_) => r#gen.into_root_schema_for::<Shell>(),
            Tools::Followup(_) => r#gen.into_root_schema_for::<Followup>(),
            Tools::Fetch(_) => r#gen.into_root_schema_for::<NetFetch>(),
            Tools::AttemptCompletion(_) => r#gen.into_root_schema_for::<AttemptCompletion>(),
            Tools::Search(_) => r#gen.into_root_schema_for::<FSSearch>(),
            Tools::Read(_) => r#gen.into_root_schema_for::<FSRead>(),
            Tools::Remove(_) => r#gen.into_root_schema_for::<FSRemove>(),
            Tools::Undo(_) => r#gen.into_root_schema_for::<FSUndo>(),
            Tools::Write(_) => r#gen.into_root_schema_for::<FSWrite>(),
            Tools::Plan(_) => r#gen.into_root_schema_for::<PlanCreate>(),
        }
    }

    pub fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(self)
            .description(self.description())
            .input_schema(self.schema())
    }
    pub fn contains(tool_name: &ToolName) -> bool {
        FORGE_TOOLS.contains(tool_name)
    }
    pub fn should_yield(tool_name: &ToolName) -> bool {
        // Tools that convey that the execution should yield
        [
            ToolsDiscriminants::Followup,
            ToolsDiscriminants::AttemptCompletion,
        ]
        .iter()
        .any(|v| v.to_string().to_case(Case::Snake).eq(tool_name.as_str()))
    }
    pub fn is_attempt_completion(tool_name: &ToolName) -> bool {
        // Tool that convey that conversation might be completed
        [ToolsDiscriminants::AttemptCompletion]
            .iter()
            .any(|v| v.to_string().to_case(Case::Snake).eq(tool_name.as_str()))
    }

    /// Convert a tool input to its corresponding domain operation for policy
    /// checking. Returns None for tools that don't require permission
    /// checks.
    pub fn to_policy_operation(
        &self,
        cwd: PathBuf,
    ) -> Option<crate::policies::PermissionOperation> {
        let cwd_path = cwd.clone();
        let display_path_for = |path: &str| {
            format!(
                "`{}`",
                format_display_path(Path::new(path), cwd_path.as_path())
            )
        };

        match self {
            Tools::Read(input) => Some(crate::policies::PermissionOperation::Read {
                path: std::path::PathBuf::from(&input.path),
                cwd,
                message: format!("Read file: {}", display_path_for(&input.path)),
            }),
            Tools::Write(input) => Some(crate::policies::PermissionOperation::Write {
                path: std::path::PathBuf::from(&input.path),
                cwd,
                message: format!("Create/overwrite file: {}", display_path_for(&input.path)),
            }),
            Tools::Search(input) => {
                let base_message = format!(
                    "Search in directory/file: {}",
                    display_path_for(&input.path)
                );
                let message = match (&input.regex, &input.file_pattern) {
                    (Some(regex), Some(pattern)) => {
                        format!("{base_message} for pattern: '{regex}' in '{pattern}' files")
                    }
                    (Some(regex), None) => {
                        format!("{base_message} for pattern: {regex}")
                    }
                    (None, Some(pattern)) => {
                        format!("{base_message} in '{pattern}' files")
                    }
                    (None, None) => base_message,
                };
                Some(crate::policies::PermissionOperation::Read {
                    path: std::path::PathBuf::from(&input.path),
                    cwd,
                    message,
                })
            }
            Tools::Remove(input) => Some(crate::policies::PermissionOperation::Write {
                path: std::path::PathBuf::from(&input.path),
                cwd,
                message: format!("Remove file: {}", display_path_for(&input.path)),
            }),
            Tools::Patch(input) => Some(crate::policies::PermissionOperation::Write {
                path: std::path::PathBuf::from(&input.path),
                cwd,
                message: format!("Modify file: {}", display_path_for(&input.path)),
            }),
            Tools::Shell(input) => Some(crate::policies::PermissionOperation::Execute {
                command: input.command.clone(),
                cwd,
                message: format!("Execute shell command: {}", input.command),
            }),
            Tools::Fetch(input) => Some(crate::policies::PermissionOperation::Fetch {
                url: input.url.clone(),
                cwd,
                message: format!("Fetch content from URL: {}", input.url),
            }),
            // Operations that don't require permission checks
            Tools::Undo(_) | Tools::Followup(_) | Tools::AttemptCompletion(_) | Tools::Plan(_) => {
                None
            }
        }
    }
}

fn format_display_path(path: &Path, cwd: &Path) -> String {
    // Try to create a relative path for display if possible
    let display_path = if path.starts_with(cwd) {
        match path.strip_prefix(cwd) {
            Ok(rel_path) => rel_path.display().to_string(),
            Err(_) => path.display().to_string(),
        }
    } else {
        path.display().to_string()
    };

    if display_path.is_empty() {
        ".".to_string()
    } else {
        display_path
    }
}

impl TryFrom<ToolCallFull> for Tools {
    type Error = crate::Error;

    fn try_from(value: ToolCallFull) -> Result<Self, Self::Error> {
        let mut map = Map::new();
        map.insert("name".into(), value.name.as_str().into());
        map.insert("arguments".into(), value.arguments.parse()?);

        serde_json::from_value(serde_json::Value::Object(map))
            .map_err(|error| crate::Error::AgentCallArgument { error })
    }
}

impl ToolsDiscriminants {
    pub fn name(&self) -> ToolName {
        ToolName::new(self.to_string().to_case(Case::Snake))
    }

    // TODO: This is an extremely slow operation
    pub fn definition(&self) -> ToolDefinition {
        Tools::iter()
            .find(|tool| tool.definition().name == self.name())
            .map(|tool| tool.definition())
            .expect("Forge tool definition not found")
    }
}

impl TryFrom<&ToolCallFull> for AgentInput {
    type Error = crate::Error;
    fn try_from(value: &ToolCallFull) -> Result<Self, Self::Error> {
        let value = value.arguments.parse()?;
        serde_json::from_value(value).map_err(|error| crate::Error::AgentCallArgument { error })
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use strum::IntoEnumIterator;

    use crate::{ToolName, Tools, ToolsDiscriminants};

    #[test]
    fn test_is_complete() {
        let complete_tool = ToolName::new("attempt_completion");
        let incomplete_tool = ToolName::new("read");

        assert!(Tools::is_attempt_completion(&complete_tool));
        assert!(!Tools::is_attempt_completion(&incomplete_tool));
    }

    #[test]
    fn test_tool_definition() {
        let actual = ToolsDiscriminants::Remove.name();
        let expected = ToolName::new("remove");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_tool_definition_json() {
        let tools = Tools::iter()
            .map(|tool| {
                let definition = tool.definition();
                serde_json::to_string_pretty(&definition)
                    .expect("Failed to serialize tool definition to JSON")
            })
            .collect::<Vec<_>>()
            .join("\n");

        insta::assert_snapshot!(tools);
    }

    #[test]
    fn test_fs_search_message_with_regex() {
        use std::path::PathBuf;

        use crate::FSSearch;
        use crate::policies::PermissionOperation;

        let search_with_regex = Tools::Search(FSSearch {
            path: "/home/user/project".to_string(),
            regex: Some("fn main".to_string()),
            start_index: None,
            max_search_lines: None,
            file_pattern: None,
            explanation: None,
        });

        let operation = search_with_regex
            .to_policy_operation(PathBuf::from("/test/cwd"))
            .unwrap();

        match operation {
            PermissionOperation::Read { message, .. } => {
                assert_eq!(
                    message,
                    "Search in directory/file: `/home/user/project` for pattern: fn main"
                );
            }
            _ => panic!("Expected Read operation"),
        }
    }

    #[test]
    fn test_fs_search_message_without_regex() {
        use std::path::PathBuf;

        use crate::FSSearch;
        use crate::policies::PermissionOperation;

        let search_without_regex = Tools::Search(FSSearch {
            path: "/home/user/project".to_string(),
            regex: None,
            start_index: None,
            max_search_lines: None,
            file_pattern: None,
            explanation: None,
        });

        let operation = search_without_regex
            .to_policy_operation(PathBuf::from("/test/cwd"))
            .unwrap();

        match operation {
            PermissionOperation::Read { message, .. } => {
                assert_eq!(message, "Search in directory/file: `/home/user/project`");
            }
            _ => panic!("Expected Read operation"),
        }
    }

    #[test]
    fn test_fs_search_message_with_file_pattern_only() {
        use std::path::PathBuf;

        use crate::FSSearch;
        use crate::policies::PermissionOperation;

        let search_with_pattern = Tools::Search(FSSearch {
            path: "/home/user/project".to_string(),
            regex: None,
            start_index: None,
            max_search_lines: None,
            file_pattern: Some("*.rs".to_string()),
            explanation: None,
        });

        let operation = search_with_pattern
            .to_policy_operation(PathBuf::from("/test/cwd"))
            .unwrap();

        match operation {
            PermissionOperation::Read { message, .. } => {
                assert_eq!(
                    message,
                    "Search in directory/file: `/home/user/project` in '*.rs' files"
                );
            }
            _ => panic!("Expected Read operation"),
        }
    }

    #[test]
    fn test_fs_search_message_with_regex_and_file_pattern() {
        use std::path::PathBuf;

        use crate::FSSearch;
        use crate::policies::PermissionOperation;

        let search_with_both = Tools::Search(FSSearch {
            path: "/home/user/project".to_string(),
            regex: Some("fn main".to_string()),
            start_index: None,
            max_search_lines: None,
            file_pattern: Some("*.rs".to_string()),
            explanation: None,
        });

        let operation = search_with_both
            .to_policy_operation(PathBuf::from("/test/cwd"))
            .unwrap();

        match operation {
            PermissionOperation::Read { message, .. } => {
                assert_eq!(
                    message,
                    "Search in directory/file: `/home/user/project` for pattern: 'fn main' in '*.rs' files"
                );
            }
            _ => panic!("Expected Read operation"),
        }
    }
}
