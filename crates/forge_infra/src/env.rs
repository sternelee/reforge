use std::path::{Path, PathBuf};
use std::str::FromStr;

use forge_domain::{Environment, Provider, RetryConfig, TlsBackend, TlsVersion};
use forge_services::EnvironmentInfra;
use reqwest::Url;

#[derive(Clone)]
pub struct ForgeEnvironmentInfra {
    restricted: bool,
    cwd: PathBuf,
}

impl ForgeEnvironmentInfra {
    /// Creates a new EnvironmentFactory with specified working directory
    ///
    /// # Arguments
    /// * `restricted` - If true, use restricted shell mode (rbash) If false,
    ///   use unrestricted shell mode (sh/bash)
    /// * `cwd` - Required working directory path
    pub fn new(restricted: bool, cwd: PathBuf) -> Self {
        Self::dot_env(&cwd);
        Self { restricted, cwd }
    }

    /// Get path to appropriate shell based on platform and mode
    fn get_shell_path(&self) -> String {
        if cfg!(target_os = "windows") {
            std::env::var("COMSPEC").unwrap_or("cmd.exe".to_string())
        } else if self.restricted {
            // Default to rbash in restricted mode
            "/bin/rbash".to_string()
        } else {
            // Use user's preferred shell or fallback to sh
            std::env::var("SHELL").unwrap_or("/bin/sh".to_string())
        }
    }

    fn get(&self) -> Environment {
        let cwd = self.cwd.clone();
        let retry_config = resolve_retry_config();

        let forge_api_url = self
            .get_env_var("FORGE_API_URL")
            .as_ref()
            .and_then(|url| Url::parse(url.as_str()).ok())
            .unwrap_or_else(|| Url::parse(Provider::FORGE_URL).unwrap());

        // Convert 10 KB to bytes as default
        let default_max_bytes: f64 = 10.0 * 1024.0;
        let max_bytes =
            parse_env::<f64>("FORGE_MAX_SEARCH_RESULT_BYTES").unwrap_or(default_max_bytes);

        Environment {
            os: std::env::consts::OS.to_string(),
            pid: std::process::id(),
            cwd,
            shell: self.get_shell_path(),
            base_path: dirs::home_dir()
                .map(|a| a.join("forge"))
                .unwrap_or(PathBuf::from(".").join("forge")),
            home: dirs::home_dir(),
            retry_config,
            max_search_lines: 200,
            max_search_result_bytes: max_bytes.ceil() as usize,
            fetch_truncation_limit: 40_000,
            max_read_size: 2000,
            stdout_max_prefix_length: 200,
            stdout_max_suffix_length: 200,
            tool_timeout: parse_env::<u64>("FORGE_TOOL_TIMEOUT").unwrap_or(300),
            stdout_max_line_length: parse_env::<usize>("FORGE_STDOUT_MAX_LINE_LENGTH")
                .unwrap_or(2000),
            http: resolve_http_config(),
            max_file_size: 256 << 10, // 256 KiB
            forge_api_url,
        }
    }

    /// Load all `.env` files with priority to lower (closer) files.
    fn dot_env(cwd: &Path) -> Option<()> {
        let mut paths = vec![];
        let mut current = PathBuf::new();

        for component in cwd.components() {
            current.push(component);
            paths.push(current.clone());
        }

        paths.reverse();

        for path in paths {
            let env_file = path.join(".env");
            if env_file.is_file() {
                dotenv::from_path(&env_file).ok();
            }
        }

        Some(())
    }
}

impl EnvironmentInfra for ForgeEnvironmentInfra {
    fn get_environment(&self) -> Environment {
        self.get()
    }

    fn get_env_var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

fn parse_env<T: FromStr>(name: &str) -> Option<T> {
    std::env::var(name)
        .as_ref()
        .ok()
        .and_then(|var| T::from_str(var).ok())
}

/// Resolves retry configuration from environment variables or returns defaults
fn resolve_retry_config() -> RetryConfig {
    let mut config = RetryConfig::default();

    if let Some(parsed) = parse_env::<u64>("FORGE_RETRY_INITIAL_BACKOFF_MS") {
        config.initial_backoff_ms = parsed;
    }
    if let Some(parsed) = parse_env::<u64>("FORGE_RETRY_BACKOFF_FACTOR") {
        config.backoff_factor = parsed;
    }
    if let Some(parsed) = parse_env::<usize>("FORGE_RETRY_MAX_ATTEMPTS") {
        config.max_retry_attempts = parsed;
    }
    if let Some(parsed) = parse_env::<bool>("FORGE_SUPPRESS_RETRY_ERRORS") {
        config.suppress_retry_errors = parsed;
    }

    // Special handling for comma-separated status codes
    if let Ok(val) = std::env::var("FORGE_RETRY_STATUS_CODES") {
        let status_codes: Vec<u16> = val
            .split(',')
            .filter_map(|code| code.trim().parse::<u16>().ok())
            .collect();
        if !status_codes.is_empty() {
            config.retry_status_codes = status_codes;
        }
    }

    config
}

fn resolve_http_config() -> forge_domain::HttpConfig {
    let mut config = forge_domain::HttpConfig::default();

    if let Some(parsed) = parse_env::<u64>("FORGE_HTTP_CONNECT_TIMEOUT") {
        config.connect_timeout = parsed;
    }
    if let Some(parsed) = parse_env::<u64>("FORGE_HTTP_READ_TIMEOUT") {
        config.read_timeout = parsed;
    }
    if let Some(parsed) = parse_env::<u64>("FORGE_HTTP_POOL_IDLE_TIMEOUT") {
        config.pool_idle_timeout = parsed;
    }
    if let Some(parsed) = parse_env::<usize>("FORGE_HTTP_POOL_MAX_IDLE_PER_HOST") {
        config.pool_max_idle_per_host = parsed;
    }
    if let Some(parsed) = parse_env::<usize>("FORGE_HTTP_MAX_REDIRECTS") {
        config.max_redirects = parsed;
    }
    if let Some(parsed) = parse_env::<bool>("FORGE_HTTP_USE_HICKORY") {
        config.hickory = parsed;
    }
    if let Some(parsed) = parse_env::<TlsBackend>("FORGE_HTTP_TLS_BACKEND") {
        config.tls_backend = parsed;
    }
    if let Some(parsed) = parse_env::<TlsVersion>("FORGE_HTTP_MIN_TLS_VERSION") {
        config.min_tls_version = Some(parsed);
    }
    if let Some(parsed) = parse_env::<TlsVersion>("FORGE_HTTP_MAX_TLS_VERSION") {
        config.max_tls_version = Some(parsed);
    }
    if let Some(parsed) = parse_env::<bool>("FORGE_HTTP_ADAPTIVE_WINDOW") {
        config.adaptive_window = parsed;
    }

    // Special handling for keep_alive_interval to allow disabling it
    if let Ok(val) = std::env::var("FORGE_HTTP_KEEP_ALIVE_INTERVAL") {
        if val.to_lowercase() == "none" || val.to_lowercase() == "disabled" {
            config.keep_alive_interval = None;
        } else if let Some(parsed) = parse_env::<u64>("FORGE_HTTP_KEEP_ALIVE_INTERVAL") {
            config.keep_alive_interval = Some(parsed);
        }
    }

    if let Some(parsed) = parse_env::<u64>("FORGE_HTTP_KEEP_ALIVE_TIMEOUT") {
        config.keep_alive_timeout = parsed;
    }
    if let Some(parsed) = parse_env::<bool>("FORGE_HTTP_KEEP_ALIVE_WHILE_IDLE") {
        config.keep_alive_while_idle = parsed;
    }
    if let Some(parsed) = parse_env::<bool>("FORGE_HTTP_ACCEPT_INVALID_CERTS") {
        config.accept_invalid_certs = parsed;
    }
    if let Some(val) = parse_env::<String>("FORGE_HTTP_ROOT_CERT_PATHS") {
        let paths: Vec<String> = val
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !paths.is_empty() {
            config.root_cert_paths = Some(paths);
        }
    }

    config
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::{env, fs};

    use forge_domain::{TlsBackend, TlsVersion};
    use serial_test::serial;
    use tempfile::{TempDir, tempdir};

    use super::*;

    fn setup_envs(structure: Vec<(&str, &str)>) -> (TempDir, PathBuf) {
        let root = tempdir().unwrap();
        let root_path = root.path().to_path_buf();

        for (rel_path, content) in &structure {
            let dir = root_path.join(rel_path);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join(".env"), content).unwrap();
        }

        let deepest_path = root_path.join(structure[0].0);
        // We MUST return root path, because dropping it will remove temp dir
        (root, deepest_path)
    }

    fn clean_retry_env_vars() {
        let retry_env_vars = [
            "FORGE_RETRY_INITIAL_BACKOFF_MS",
            "FORGE_RETRY_BACKOFF_FACTOR",
            "FORGE_RETRY_MAX_ATTEMPTS",
            "FORGE_RETRY_STATUS_CODES",
            "FORGE_SUPPRESS_RETRY_ERRORS",
        ];

        for var in &retry_env_vars {
            unsafe {
                env::remove_var(var);
            }
        }
    }

    fn clean_http_env_vars() {
        let http_env_vars = [
            "FORGE_HTTP_CONNECT_TIMEOUT",
            "FORGE_HTTP_READ_TIMEOUT",
            "FORGE_HTTP_POOL_IDLE_TIMEOUT",
            "FORGE_HTTP_POOL_MAX_IDLE_PER_HOST",
            "FORGE_HTTP_MAX_REDIRECTS",
            "FORGE_HTTP_USE_HICKORY",
            "FORGE_HTTP_TLS_BACKEND",
            "FORGE_HTTP_MIN_TLS_VERSION",
            "FORGE_HTTP_MAX_TLS_VERSION",
            "FORGE_HTTP_ADAPTIVE_WINDOW",
            "FORGE_HTTP_KEEP_ALIVE_INTERVAL",
            "FORGE_HTTP_KEEP_ALIVE_TIMEOUT",
            "FORGE_HTTP_KEEP_ALIVE_WHILE_IDLE",
            "FORGE_HTTP_ACCEPT_INVALID_CERTS",
            "FORGE_HTTP_ROOT_CERT_PATHS",
        ];

        for var in &http_env_vars {
            unsafe {
                env::remove_var(var);
            }
        }
    }

    #[test]
    #[serial]
    fn test_dot_env_loading() {
        // Test single env file
        let (_root, cwd) = setup_envs(vec![("", "TEST_KEY1=VALUE1")]);
        ForgeEnvironmentInfra::dot_env(&cwd);
        assert_eq!(env::var("TEST_KEY1").unwrap(), "VALUE1");

        // Test nested env files with override (closer files win)
        let (_root, cwd) = setup_envs(vec![("a/b", "TEST_KEY2=SUB"), ("a", "TEST_KEY2=ROOT")]);
        ForgeEnvironmentInfra::dot_env(&cwd);
        assert_eq!(env::var("TEST_KEY2").unwrap(), "SUB");

        // Test multiple keys from different levels
        let (_root, cwd) = setup_envs(vec![
            ("a/b", "SUB_KEY3=SUB_VAL"),
            ("a", "ROOT_KEY3=ROOT_VAL"),
        ]);
        ForgeEnvironmentInfra::dot_env(&cwd);
        assert_eq!(env::var("ROOT_KEY3").unwrap(), "ROOT_VAL");
        assert_eq!(env::var("SUB_KEY3").unwrap(), "SUB_VAL");

        // Test standard env precedence (std env wins over .env files)
        let (_root, cwd) = setup_envs(vec![("a/b", "TEST_KEY4=SUB_VAL")]);
        unsafe {
            env::set_var("TEST_KEY4", "STD_ENV_VAL");
        }
        ForgeEnvironmentInfra::dot_env(&cwd);
        assert_eq!(env::var("TEST_KEY4").unwrap(), "STD_ENV_VAL");
    }

    #[test]
    #[serial]
    fn test_retry_config_parsing() {
        clean_retry_env_vars();

        // Test defaults match RetryConfig::default()
        let actual = resolve_retry_config();
        let expected = RetryConfig::default();
        assert_eq!(actual.max_retry_attempts, expected.max_retry_attempts);
        assert_eq!(actual.initial_backoff_ms, expected.initial_backoff_ms);
        assert_eq!(actual.backoff_factor, expected.backoff_factor);
        assert_eq!(actual.retry_status_codes, expected.retry_status_codes);
        assert_eq!(actual.suppress_retry_errors, expected.suppress_retry_errors);

        // Test environment variable overrides
        unsafe {
            env::set_var("FORGE_RETRY_INITIAL_BACKOFF_MS", "500");
            env::set_var("FORGE_RETRY_BACKOFF_FACTOR", "3");
            env::set_var("FORGE_RETRY_MAX_ATTEMPTS", "5");
            env::set_var("FORGE_RETRY_STATUS_CODES", "429,500,502");
            env::set_var("FORGE_SUPPRESS_RETRY_ERRORS", "true");
        }

        let actual = resolve_retry_config();
        assert_eq!(actual.initial_backoff_ms, 500);
        assert_eq!(actual.backoff_factor, 3);
        assert_eq!(actual.max_retry_attempts, 5);
        assert_eq!(actual.retry_status_codes, vec![429, 500, 502]);
        assert_eq!(actual.suppress_retry_errors, true);

        clean_retry_env_vars();
    }

    #[test]
    #[serial]
    fn test_retry_config_invalid_values() {
        clean_retry_env_vars();

        // Set invalid values - should fallback to defaults
        unsafe {
            env::set_var("FORGE_RETRY_INITIAL_BACKOFF_MS", "invalid");
            env::set_var("FORGE_RETRY_MAX_ATTEMPTS", "abc");
            env::set_var("FORGE_RETRY_STATUS_CODES", "invalid,codes");
        }

        let actual = resolve_retry_config();
        let expected = RetryConfig::default();
        assert_eq!(actual.initial_backoff_ms, expected.initial_backoff_ms);
        assert_eq!(actual.max_retry_attempts, expected.max_retry_attempts);
        assert_eq!(actual.retry_status_codes, expected.retry_status_codes);

        clean_retry_env_vars();
    }

    #[test]
    #[serial]
    fn test_http_config_parsing() {
        clean_http_env_vars();

        // Test defaults match HttpConfig::default()
        let actual = resolve_http_config();
        let expected = forge_domain::HttpConfig::default();
        assert_eq!(actual.connect_timeout, expected.connect_timeout);
        assert_eq!(actual.read_timeout, expected.read_timeout);
        assert_eq!(actual.tls_backend, expected.tls_backend);
        assert_eq!(actual.hickory, expected.hickory);
        assert_eq!(actual.accept_invalid_certs, expected.accept_invalid_certs);
        assert_eq!(actual.root_cert_paths, expected.root_cert_paths);

        // Test environment variable overrides
        unsafe {
            env::set_var("FORGE_HTTP_CONNECT_TIMEOUT", "30");
            env::set_var("FORGE_HTTP_USE_HICKORY", "true");
            env::set_var("FORGE_HTTP_TLS_BACKEND", "rustls");
            env::set_var("FORGE_HTTP_MIN_TLS_VERSION", "1.2");
            env::set_var("FORGE_HTTP_KEEP_ALIVE_INTERVAL", "30");
            env::set_var("FORGE_HTTP_ACCEPT_INVALID_CERTS", "true");
            env::set_var(
                "FORGE_HTTP_ROOT_CERT_PATHS",
                "/path/to/cert1.pem,/path/to/cert2.crt",
            );
        }

        let actual = resolve_http_config();
        assert_eq!(actual.connect_timeout, 30);
        assert_eq!(actual.hickory, true);
        assert_eq!(actual.tls_backend, TlsBackend::Rustls);
        assert_eq!(actual.min_tls_version, Some(TlsVersion::V1_2));
        assert_eq!(actual.keep_alive_interval, Some(30));
        assert_eq!(actual.accept_invalid_certs, true);
        assert_eq!(
            actual.root_cert_paths,
            Some(vec![
                "/path/to/cert1.pem".to_string(),
                "/path/to/cert2.crt".to_string()
            ])
        );

        clean_http_env_vars();
    }

    #[test]
    #[serial]
    fn test_http_config_keep_alive_special_cases() {
        clean_http_env_vars();

        // Test "none" and "disabled" values disable keep_alive_interval
        for disable_value in ["none", "disabled", "NONE", "DISABLED"] {
            unsafe {
                env::set_var("FORGE_HTTP_KEEP_ALIVE_INTERVAL", disable_value);
            }
            let actual = resolve_http_config();
            assert_eq!(actual.keep_alive_interval, None);
        }

        clean_http_env_vars();
    }

    #[test]
    #[serial]
    fn test_max_search_result_bytes() {
        unsafe {
            env::remove_var("FORGE_MAX_SEARCH_RESULT_BYTES");
        }

        // Test default value
        let forge_env = ForgeEnvironmentInfra::new(false, PathBuf::from("/tmp"));
        let environment = forge_env.get_environment();
        let expected_default = (10.0_f64 * 1024.0).ceil() as usize;
        assert_eq!(environment.max_search_result_bytes, expected_default);

        // Test environment override
        unsafe {
            env::set_var("FORGE_MAX_SEARCH_RESULT_BYTES", "1048576");
        }
        let environment = forge_env.get_environment();
        assert_eq!(environment.max_search_result_bytes, 1048576);

        // Test fractional value gets ceiled
        unsafe {
            env::set_var("FORGE_MAX_SEARCH_RESULT_BYTES", "524288.5");
        }
        let environment = forge_env.get_environment();
        assert_eq!(environment.max_search_result_bytes, 524289);

        // Test invalid value falls back to default
        unsafe {
            env::set_var("FORGE_MAX_SEARCH_RESULT_BYTES", "invalid");
        }
        let environment = forge_env.get_environment();
        assert_eq!(environment.max_search_result_bytes, expected_default);

        unsafe {
            env::remove_var("FORGE_MAX_SEARCH_RESULT_BYTES");
        }
    }

    #[test]
    #[serial]
    fn test_tool_timeout_env_var() {
        let cwd = tempdir().unwrap().path().to_path_buf();
        let infra = ForgeEnvironmentInfra::new(false, cwd);

        // Test Default value when env var is not set
        {
            unsafe {
                env::remove_var("FORGE_TOOL_TIMEOUT");
            }
            let env = infra.get_environment();
            assert_eq!(env.tool_timeout, 300);
        }

        // Test Value from env var
        {
            unsafe {
                env::set_var("FORGE_TOOL_TIMEOUT", "15");
            }
            let env = infra.get_environment();
            assert_eq!(env.tool_timeout, 15);
            unsafe {
                env::remove_var("FORGE_TOOL_TIMEOUT");
            }
        }

        // Test Fallback to default for invalid value
        {
            unsafe {
                env::set_var("TOOL_TIMEOUT_SECONDS", "not-a-number");
            }
            let env = infra.get_environment();
            assert_eq!(env.tool_timeout, 300);
            unsafe {
                env::remove_var("TOOL_TIMEOUT_SECONDS");
            }
        }
    }
}
