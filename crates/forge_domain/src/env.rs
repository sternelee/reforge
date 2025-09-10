use std::path::PathBuf;

use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{HttpConfig, RetryConfig};

const VERSION: &str = match option_env!("APP_VERSION") {
    Some(val) => val,
    None => env!("CARGO_PKG_VERSION"),
};

#[derive(Debug, Setters, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[setters(strip_option)]
/// Represents the environment in which the application is running.
pub struct Environment {
    /// The operating system of the environment.
    pub os: String,
    /// The process ID of the current process.
    pub pid: u32,
    /// The current working directory.
    pub cwd: PathBuf,
    /// The home directory.
    pub home: Option<PathBuf>,
    /// The shell being used.
    pub shell: String,
    /// The base path relative to which everything else stored.
    pub base_path: PathBuf,
    /// Base URL for Forge's backend APIs
    pub forge_api_url: Url,
    /// Configuration for the retry mechanism
    pub retry_config: RetryConfig,
    /// The maximum number of lines returned for FSSearch.
    pub max_search_lines: usize,
    /// Maximum bytes allowed for search results
    pub max_search_result_bytes: usize,
    /// Maximum characters for fetch content
    pub fetch_truncation_limit: usize,
    /// Maximum lines for shell output prefix
    pub stdout_max_prefix_length: usize,
    /// Maximum lines for shell output suffix
    pub stdout_max_suffix_length: usize,
    /// Maximum characters per line for shell output
    pub stdout_max_line_length: usize,
    /// Maximum number of lines to read from a file
    pub max_read_size: u64,
    /// Http configuration
    pub http: HttpConfig,
    /// Maximum file size in bytes for operations
    pub max_file_size: u64,
    /// Maximum execution time in seconds for a single tool call.
    /// Controls how long a tool can run before being terminated.
    pub tool_timeout: u64,
}

impl Environment {
    pub fn db_path(&self) -> PathBuf {
        self.base_path.clone()
    }

    pub fn log_path(&self) -> PathBuf {
        self.base_path.join("logs")
    }

    pub fn history_path(&self) -> PathBuf {
        self.base_path.join(".forge_history")
    }
    pub fn snapshot_path(&self) -> PathBuf {
        self.base_path.join("snapshots")
    }
    pub fn mcp_user_config(&self) -> PathBuf {
        self.base_path.join(".mcp.json")
    }

    pub fn templates(&self) -> PathBuf {
        self.base_path.join("templates")
    }
    pub fn agent_path(&self) -> PathBuf {
        self.base_path.join("agents")
    }
    pub fn agent_cwd_path(&self) -> PathBuf {
        self.cwd.join(".forge/agents")
    }
    pub fn permissions_path(&self) -> PathBuf {
        self.base_path.join("permissions.yaml")
    }

    pub fn mcp_local_config(&self) -> PathBuf {
        self.cwd.join(".mcp.json")
    }
    pub fn version(&self) -> String {
        VERSION.to_string()
    }
    pub fn app_config(&self) -> PathBuf {
        self.base_path.join(".config.json")
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_agent_cwd_path() {
        // Create a test environment with arbitrary values
        let fixture = Environment {
            os: "linux".to_string(),
            pid: 1234,
            cwd: PathBuf::from("/current/working/dir"),
            home: Some(PathBuf::from("/home/user")),
            shell: "zsh".to_string(),
            base_path: PathBuf::from("/home/user/.forge"),
            forge_api_url: "https://api.example.com".parse().unwrap(),
            retry_config: RetryConfig::default(),
            max_search_lines: 1000,
            max_search_result_bytes: 10240,
            fetch_truncation_limit: 50000,
            stdout_max_prefix_length: 100,
            stdout_max_suffix_length: 100,
            stdout_max_line_length: 500,
            max_read_size: 2000,
            http: HttpConfig::default(),
            max_file_size: 104857600,
            tool_timeout: 300,
        };

        let actual = fixture.agent_cwd_path();
        let expected = PathBuf::from("/current/working/dir/.forge/agents");

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_agent_cwd_path_independent_from_agent_path() {
        // Create a test environment with different base_path and cwd
        let fixture = Environment {
            os: "linux".to_string(),
            pid: 1234,
            cwd: PathBuf::from("/different/current/dir"),
            home: Some(PathBuf::from("/different/home")),
            shell: "bash".to_string(),
            base_path: PathBuf::from("/completely/different/base"),
            forge_api_url: "https://api.example.com".parse().unwrap(),
            retry_config: RetryConfig::default(),
            max_search_lines: 1000,
            max_search_result_bytes: 10240,
            fetch_truncation_limit: 50000,
            stdout_max_prefix_length: 100,
            stdout_max_suffix_length: 100,
            stdout_max_line_length: 500,
            max_read_size: 2000,
            http: HttpConfig::default(),
            max_file_size: 104857600,
            tool_timeout: 300,
        };

        let agent_path = fixture.agent_path();
        let agent_cwd_path = fixture.agent_cwd_path();
        let expected_agent_path = PathBuf::from("/completely/different/base/agents");
        let expected_agent_cwd_path = PathBuf::from("/different/current/dir/.forge/agents");

        // Verify that agent_path uses base_path
        assert_eq!(agent_path, expected_agent_path);

        // Verify that agent_cwd_path is independent and always relative to CWD
        assert_eq!(agent_cwd_path, expected_agent_cwd_path);

        // Verify they are different paths
        assert_ne!(agent_path, agent_cwd_path);
    }
}
