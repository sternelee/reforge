use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone)]
pub enum ProviderUrl {
    OpenAI(String),
    Anthropic(String),
}
impl ProviderUrl {
    pub fn into_string(self) -> String {
        match self {
            ProviderUrl::OpenAI(url) => url,
            ProviderUrl::Anthropic(url) => url,
        }
    }
}

/// Providers that can be used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Provider {
    OpenAI { url: Url, key: Option<String> },
    Anthropic { url: Url, key: String },
}

impl Provider {
    pub fn url(&mut self, url: ProviderUrl) {
        match url {
            ProviderUrl::OpenAI(url) => self.open_ai_url(url),
            ProviderUrl::Anthropic(url) => self.anthropic_url(url),
        }
    }
    /// Sets the OpenAI URL if the provider is an OpenAI compatible provider
    fn open_ai_url(&mut self, url: String) {
        match self {
            Provider::OpenAI { url: set_url, .. } => {
                if url.ends_with("/") {
                    *set_url = Url::parse(&url).unwrap();
                } else {
                    *set_url = Url::parse(&format!("{url}/")).unwrap();
                }
            }
            Provider::Anthropic { .. } => {}
        }
    }

    /// Sets the Anthropic URL if the provider is Anthropic
    fn anthropic_url(&mut self, url: String) {
        match self {
            Provider::Anthropic { url: set_url, .. } => {
                if url.ends_with("/") {
                    *set_url = Url::parse(&url).unwrap();
                } else {
                    *set_url = Url::parse(&format!("{url}/")).unwrap();
                }
            }
            Provider::OpenAI { .. } => {}
        }
    }

    pub fn forge(key: &str) -> Provider {
        Provider::OpenAI {
            url: Url::parse(Provider::FORGE_URL).unwrap(),
            key: Some(key.into()),
        }
    }

    pub fn openai(key: &str) -> Provider {
        Provider::OpenAI {
            url: Url::parse(Provider::OPENAI_URL).unwrap(),
            key: Some(key.into()),
        }
    }

    pub fn open_router(key: &str) -> Provider {
        Provider::OpenAI {
            url: Url::parse(Provider::OPEN_ROUTER_URL).unwrap(),
            key: Some(key.into()),
        }
    }

    pub fn requesty(key: &str) -> Provider {
        Provider::OpenAI {
            url: Url::parse(Provider::REQUESTY_URL).unwrap(),
            key: Some(key.into()),
        }
    }

    pub fn zai(key: &str) -> Provider {
        Provider::OpenAI {
            url: Url::parse(Provider::ZAI_URL).unwrap(),
            key: Some(key.into()),
        }
    }

    pub fn cerebras(key: &str) -> Provider {
        Provider::OpenAI {
            url: Url::parse(Provider::CEREBRAS_URL).unwrap(),
            key: Some(key.into()),
        }
    }

    pub fn xai(key: &str) -> Provider {
        Provider::OpenAI {
            url: Url::parse(Provider::XAI_URL).unwrap(),
            key: Some(key.into()),
        }
    }

    pub fn anthropic(key: &str) -> Provider {
        Provider::Anthropic {
            url: Url::parse(Provider::ANTHROPIC_URL).unwrap(),
            key: key.into(),
        }
    }

    pub fn vercel(key: &str) -> Provider {
        Provider::OpenAI {
            url: Url::parse(Provider::VERCEL_URL).unwrap(),
            key: Some(key.into()),
        }
    }

    pub fn deepseek(key: &str) -> Provider {
        Provider::OpenAI {
            url: Url::parse(Provider::DEEPSEEK_URL).unwrap(),
            key: Some(key.into()),
        }
    }

    pub fn qwen(key: &str) -> Provider {
        Provider::OpenAI {
            url: Url::parse(Provider::QWEN_URL).unwrap(),
            key: Some(key.into()),
        }
    }

    pub fn chatglm(key: &str) -> Provider {
        Provider::OpenAI {
            url: Url::parse(Provider::CHATGLM_URL).unwrap(),
            key: Some(key.into()),
        }
    }

    pub fn moonshot(key: &str) -> Provider {
        Provider::OpenAI {
            url: Url::parse(Provider::MOONSHOT_URL).unwrap(),
            key: Some(key.into()),
        }
    }

    pub fn wisdom(key: &str) -> Provider {
        Provider::OpenAI {
            url: Url::parse(Provider::WISDOM_URL).unwrap(),
            key: Some(key.into()),
        }
    }

    pub fn key(&self) -> Option<&str> {
        match self {
            Provider::OpenAI { key, .. } => key.as_deref(),
            Provider::Anthropic { key, .. } => Some(key),
        }
    }
}

impl Provider {
    pub const OPEN_ROUTER_URL: &str = "https://openrouter.ai/api/v1/";
    pub const REQUESTY_URL: &str = "https://router.requesty.ai/v1/";
    pub const XAI_URL: &str = "https://api.x.ai/v1/";
    pub const OPENAI_URL: &str = "https://api.openai.com/v1/";
    pub const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/";
    pub const FORGE_URL: &str = "https://antinomy.ai/api/v1/";
    pub const ZAI_URL: &str = "https://api.z.ai/api/paas/v4/";
    pub const CEREBRAS_URL: &str = "https://api.cerebras.ai/v1/";
    pub const VERCEL_URL: &str = "https://ai-gateway.vercel.sh/v1/";
    pub const DEEPSEEK_URL: &str = "https://api.deepseek.com/v1/";
    pub const QWEN_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1/";
    pub const CHATGLM_URL: &str = "https://open.bigmodel.cn/api/paas/v4/";
    pub const MOONSHOT_URL: &str = "https://api.moonshot.cn/v1/";
    pub const WISDOM_URL: &str = "https://wisdom-gate.juheapi.com/v1/";

    /// Converts the provider to it's base URL
    pub fn to_base_url(&self) -> Url {
        match self {
            Provider::OpenAI { url, .. } => url.clone(),
            Provider::Anthropic { url, .. } => url.clone(),
        }
    }

    pub fn is_forge(&self) -> bool {
        match self {
            Provider::OpenAI { url, .. } => url.as_str().starts_with(Self::FORGE_URL),
            Provider::Anthropic { .. } => false,
        }
    }

    pub fn is_open_router(&self) -> bool {
        match self {
            Provider::OpenAI { url, .. } => url.as_str().starts_with(Self::OPEN_ROUTER_URL),
            Provider::Anthropic { .. } => false,
        }
    }

    pub fn is_requesty(&self) -> bool {
        match self {
            Provider::OpenAI { url, .. } => url.as_str().starts_with(Self::REQUESTY_URL),
            Provider::Anthropic { .. } => false,
        }
    }

    pub fn is_zai(&self) -> bool {
        match self {
            Provider::OpenAI { url, .. } => url.as_str().starts_with(Self::ZAI_URL),
            Provider::Anthropic { .. } => false,
        }
    }

    pub fn is_cerebras(&self) -> bool {
        match self {
            Provider::OpenAI { url, .. } => url.as_str().starts_with(Self::CEREBRAS_URL),
            Provider::Anthropic { .. } => false,
        }
    }

    pub fn is_xai(&self) -> bool {
        match self {
            Provider::OpenAI { url, .. } => url.as_str().starts_with(Self::XAI_URL),
            Provider::Anthropic { .. } => false,
        }
    }

    pub fn is_open_ai(&self) -> bool {
        match self {
            Provider::OpenAI { url, .. } => url.as_str().starts_with(Self::OPENAI_URL),
            Provider::Anthropic { .. } => false,
        }
    }

    pub fn is_anthropic(&self) -> bool {
        match self {
            Provider::OpenAI { .. } => false,
            Provider::Anthropic { url, .. } => url.as_str().starts_with(Self::ANTHROPIC_URL),
        }
    }

    pub fn is_vercel(&self) -> bool {
        match self {
            Provider::OpenAI { url, .. } => url.as_str().starts_with(Self::VERCEL_URL),
            Provider::Anthropic { .. } => false,
        }
    }

    pub fn is_deepseek(&self) -> bool {
        match self {
            Provider::OpenAI { url, .. } => url.as_str().starts_with(Self::DEEPSEEK_URL),
            Provider::Anthropic { .. } => false,
        }
    }

    pub fn is_qwen(&self) -> bool {
        match self {
            Provider::OpenAI { url, .. } => url.as_str().starts_with(Self::QWEN_URL),
            Provider::Anthropic { .. } => false,
        }
    }

    pub fn is_chatglm(&self) -> bool {
        match self {
            Provider::OpenAI { url, .. } => url.as_str().starts_with(Self::CHATGLM_URL),
            Provider::Anthropic { .. } => false,
        }
    }

    pub fn is_moonshot(&self) -> bool {
        match self {
            Provider::OpenAI { url, .. } => url.as_str().starts_with(Self::MOONSHOT_URL),
            Provider::Anthropic { .. } => false,
        }
    }

    pub fn is_wisdom(&self) -> bool {
        match self {
            Provider::OpenAI { url, .. } => url.as_str().starts_with(Self::WISDOM_URL),
            Provider::Anthropic { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_open_ai_url() {
        let mut provider = Provider::OpenAI {
            url: Url::from_str("https://example.com/").unwrap(),
            key: None,
        };

        // Test URL without trailing slash
        provider.open_ai_url("https://new-openai-url.com".to_string());
        assert_eq!(
            provider,
            Provider::OpenAI {
                url: Url::from_str("https://new-openai-url.com/").unwrap(),
                key: None
            }
        );

        // Test URL with trailing slash
        provider.open_ai_url("https://another-openai-url.com/".to_string());
        assert_eq!(
            provider,
            Provider::OpenAI {
                url: Url::from_str("https://another-openai-url.com/").unwrap(),
                key: None
            }
        );

        // Test URL with subpath without trailing slash
        provider.open_ai_url("https://new-openai-url.com/v1/api".to_string());
        assert_eq!(
            provider,
            Provider::OpenAI {
                url: Url::from_str("https://new-openai-url.com/v1/api/").unwrap(),
                key: None
            }
        );

        // Test URL with subpath with trailing slash
        provider.open_ai_url("https://another-openai-url.com/v2/api/".to_string());
        assert_eq!(
            provider,
            Provider::OpenAI {
                url: Url::from_str("https://another-openai-url.com/v2/api/").unwrap(),
                key: None
            }
        );
    }

    #[test]
    fn test_anthropic_url() {
        let mut provider = Provider::Anthropic {
            url: Url::from_str("https://example.com/").unwrap(),
            key: "key".to_string(),
        };

        // Test URL without trailing slash
        provider.anthropic_url("https://new-anthropic-url.com".to_string());
        assert_eq!(
            provider,
            Provider::Anthropic {
                url: Url::from_str("https://new-anthropic-url.com/").unwrap(),
                key: "key".to_string()
            }
        );

        // Test URL with trailing slash
        provider.anthropic_url("https://another-anthropic-url.com/".to_string());
        assert_eq!(
            provider,
            Provider::Anthropic {
                url: Url::from_str("https://another-anthropic-url.com/").unwrap(),
                key: "key".to_string()
            }
        );

        // Test URL with subpath without trailing slash
        provider.anthropic_url("https://new-anthropic-url.com/v1/complete".to_string());
        assert_eq!(
            provider,
            Provider::Anthropic {
                url: Url::from_str("https://new-anthropic-url.com/v1/complete/").unwrap(),
                key: "key".to_string()
            }
        );

        // Test URL with subpath with trailing slash
        provider.anthropic_url("https://another-anthropic-url.com/v2/complete/".to_string());
        assert_eq!(
            provider,
            Provider::Anthropic {
                url: Url::from_str("https://another-anthropic-url.com/v2/complete/").unwrap(),
                key: "key".to_string()
            }
        );
    }

    #[test]
    fn test_xai() {
        let fixture = "test_key";
        let actual = Provider::xai(fixture);
        let expected = Provider::OpenAI {
            url: Url::from_str("https://api.x.ai/v1/").unwrap(),
            key: Some(fixture.to_string()),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_is_xai() {
        let fixture_xai = Provider::xai("key");
        assert!(fixture_xai.is_xai());

        let fixture_other = Provider::openai("key");
        assert!(!fixture_other.is_xai());
    }

    #[test]
    fn test_wisdom() {
        let fixture = "test_key";
        let actual = Provider::wisdom(fixture);
        let expected = Provider::OpenAI {
            url: Url::from_str("https://wisdom-gate.juheapi.com/v1/").unwrap(),
            key: Some(fixture.to_string()),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_is_wisdom() {
        let fixture_wisdom = Provider::wisdom("key");
        assert!(fixture_wisdom.is_wisdom());

        let fixture_other = Provider::openai("key");
        assert!(!fixture_other.is_wisdom());
    }
}
