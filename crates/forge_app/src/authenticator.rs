use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use forge_domain::RetryConfig;

use crate::dto::InitAuth;
use crate::{AppConfigService, AuthService, Error};

pub struct Authenticator<S> {
    service: Arc<S>,
}

impl<S: AppConfigService + AuthService> Authenticator<S> {
    pub fn new(service: Arc<S>) -> Self {
        Self { service }
    }
    pub async fn init(&self) -> anyhow::Result<InitAuth> {
        self.service.init_auth().await
    }
    pub async fn login(&self, init_auth: &InitAuth) -> anyhow::Result<()> {
        self.poll(
            RetryConfig::default()
                .max_retry_attempts(300usize)
                .max_delay(2)
                .backoff_factor(1u64),
            || self.login_inner(init_auth),
        )
        .await
    }
    pub async fn logout(&self) -> anyhow::Result<()> {
        if let Some(mut config) = self.service.get_app_config().await {
            config.key_info.take();
            self.service.set_app_config(&config).await?;
        }
        Ok(())
    }
    async fn login_inner(&self, init_auth: &InitAuth) -> anyhow::Result<()> {
        let mut config = self.service.get_app_config().await.unwrap_or_default();
        if config.key_info.is_some() {
            return Ok(());
        }
        let key = self.service.login(init_auth).await?;

        config.key_info.replace(key);
        self.service.set_app_config(&config).await?;
        Ok(())
    }
    async fn poll<T, F>(
        &self,
        config: RetryConfig,
        call: impl Fn() -> F + Send,
    ) -> anyhow::Result<T>
    where
        F: Future<Output = anyhow::Result<T>> + Send,
    {
        let mut builder = ExponentialBuilder::default()
            .with_factor(1.0)
            .with_factor(config.backoff_factor as f32)
            .with_max_times(config.max_retry_attempts)
            .with_jitter();
        if let Some(max_delay) = config.max_delay {
            builder = builder.with_max_delay(Duration::from_secs(max_delay))
        }

        call.retry(builder)
            .when(|e| {
                // Only retry on Error::AuthInProgress (202 status)
                e.downcast_ref::<Error>()
                    .map(|v| matches!(v, Error::AuthInProgress))
                    .unwrap_or(false)
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_poll_retry_condition() {
        // Test that the retry condition only matches AuthInProgress errors
        let auth_in_progress_error = anyhow::Error::from(Error::AuthInProgress);
        let other_error = anyhow::anyhow!("Some other error");
        let serde_error = anyhow::Error::from(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "test",
        )));

        // Create a test closure that mimics the retry condition
        let retry_condition = |e: &anyhow::Error| {
            if let Some(app_error) = e.downcast_ref::<Error>() {
                matches!(app_error, Error::AuthInProgress)
            } else {
                false
            }
        };

        // Test cases
        assert_eq!(retry_condition(&auth_in_progress_error), true);
        assert_eq!(retry_condition(&other_error), false);
        assert_eq!(retry_condition(&serde_error), false);
    }
}
