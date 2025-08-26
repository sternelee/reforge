use std::sync::Arc;

use anyhow::Context;
use forge_app::ProviderRegistry;
use forge_app::domain::{Provider, ProviderUrl};
use forge_app::dto::AppConfig;
use tokio::sync::RwLock;

use crate::EnvironmentInfra;

type ProviderSearch = (&'static str, Box<dyn FnOnce(&str) -> Provider>);

pub struct ForgeProviderRegistry<F> {
    infra: Arc<F>,
    // IMPORTANT: This cache is used to avoid logging out if the user has logged out from other
    // session. This helps to keep the user logged in for current session.
    cache: Arc<RwLock<Option<Provider>>>,
}

impl<F: EnvironmentInfra> ForgeProviderRegistry<F> {
    pub fn new(infra: Arc<F>) -> Self {
        Self { infra, cache: Arc::new(Default::default()) }
    }

    fn provider_url(&self) -> Option<ProviderUrl> {
        if let Some(url) = self.infra.get_env_var("OPENAI_URL") {
            return Some(ProviderUrl::OpenAI(url));
        }

        // Check for Anthropic URL override
        if let Some(url) = self.infra.get_env_var("ANTHROPIC_URL") {
            return Some(ProviderUrl::Anthropic(url));
        }
        None
    }
    fn get_provider(&self, _forge_config: AppConfig) -> Option<Provider> {
        // if let Some(forge_key) = &forge_config.key_info {
        //     let provider = Provider::forge(forge_key.api_key.as_str());
        //     return Some(override_url(provider, self.provider_url()));
        // }
        resolve_env_provider(self.provider_url(), self.infra.as_ref())
    }
}

#[async_trait::async_trait]
impl<F: EnvironmentInfra> ProviderRegistry for ForgeProviderRegistry<F> {
    async fn get_provider(&self, config: AppConfig) -> anyhow::Result<Provider> {
        if let Some(provider) = self.cache.read().await.as_ref() {
            return Ok(provider.clone());
        }

        let provider = self
            .get_provider(config)
            .context("No valid provider configuration found. Please set one of the following environment variables: OPENROUTER_API_KEY, REQUESTY_API_KEY, XAI_API_KEY, OPENAI_API_KEY, ANTHROPIC_API_KEY, ZAI_API_KEY, VERCEL_API_KEY, DEEPSEEK_API_KEY, DASHSCOPE_API_KEY, CHATGLM_API_KEY, or MOONSHOT_API_KEY. For more details, visit: https://forgecode.dev/docs/custom-providers/")?;
        self.cache.write().await.replace(provider.clone());
        Ok(provider)
    }
}

fn resolve_env_provider<F: EnvironmentInfra>(
    url: Option<ProviderUrl>,
    env: &F,
) -> Option<Provider> {
    // Check if a specific provider is requested via FORGE_PROVIDER
    if let Some(requested_provider) = env.get_env_var("FORGE_PROVIDER") {
        let provider_name = requested_provider.to_uppercase();

        // Map of provider names to their environment variables and constructor functions
        let provider_map: Vec<(&str, &str, Box<dyn FnOnce(&str) -> Provider>)> = vec![
            (
                "OPENROUTER",
                "OPENROUTER_API_KEY",
                Box::new(Provider::open_router),
            ),
            ("REQUESTY", "REQUESTY_API_KEY", Box::new(Provider::requesty)),
            ("XAI", "XAI_API_KEY", Box::new(Provider::xai)),
            ("OPENAI", "OPENAI_API_KEY", Box::new(Provider::openai)),
            (
                "ANTHROPIC",
                "ANTHROPIC_API_KEY",
                Box::new(Provider::anthropic),
            ),
            ("CEREBRAS", "CEREBRAS_API_KEY", Box::new(Provider::cerebras)),
            ("ZAI", "ZAI_API_KEY", Box::new(Provider::zai)),
            ("VERCEL", "VERCEL_API_KEY", Box::new(Provider::vercel)),
            ("DEEPSEEK", "DEEPSEEK_API_KEY", Box::new(Provider::deepseek)),
            ("QWEN", "DASHSCOPE_API_KEY", Box::new(Provider::qwen)),
            ("CHATGLM", "CHATGLM_API_KEY", Box::new(Provider::chatglm)),
            ("MOONSHOT", "MOONSHOT_API_KEY", Box::new(Provider::moonshot)),
            ("WISDOM", "WISDOM_API_KEY", Box::new(Provider::wisdom)),
        ];

        // Try to find the requested provider
        for (name, env_var, constructor) in provider_map {
            if provider_name == name {
                if let Some(api_key) = env.get_env_var(env_var) {
                    let provider = constructor(&api_key);
                    return Some(override_url(provider, url));
                } else {
                    // Requested provider found but no API key set
                    return None;
                }
            }
        }

        // If we get here, the requested provider is not recognized
        return None;
    }

    // Fall back to the original behavior when no specific provider is requested
    let keys: [ProviderSearch; 13] = [
        // ("FORGE_KEY", Box::new(Provider::forge)),
        ("OPENROUTER_API_KEY", Box::new(Provider::open_router)),
        ("REQUESTY_API_KEY", Box::new(Provider::requesty)),
        ("XAI_API_KEY", Box::new(Provider::xai)),
        ("OPENAI_API_KEY", Box::new(Provider::openai)),
        ("ANTHROPIC_API_KEY", Box::new(Provider::anthropic)),
        ("CEREBRAS_API_KEY", Box::new(Provider::cerebras)),
        ("ZAI_API_KEY", Box::new(Provider::zai)),
        ("VERCEL_API_KEY", Box::new(Provider::vercel)),
        ("DEEPSEEK_API_KEY", Box::new(Provider::deepseek)),
        ("DASHSCOPE_API_KEY", Box::new(Provider::qwen)),
        ("CHATGLM_API_KEY", Box::new(Provider::chatglm)),
        ("MOONSHOT_API_KEY", Box::new(Provider::moonshot)),
        ("WISDOM_API_KEY", Box::new(Provider::wisdom)),
    ];

    keys.into_iter().find_map(|(key, fun)| {
        env.get_env_var(key).map(|key| {
            let provider = fun(&key);
            override_url(provider, url.clone())
        })
    })
}

fn override_url(mut provider: Provider, url: Option<ProviderUrl>) -> Provider {
    if let Some(url) = url {
        provider.url(url);
    }
    provider
}
