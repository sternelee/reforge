use std::sync::Arc;

use anyhow::Result;
use forge_app::ConversationService;
use forge_app::domain::{Conversation, ConversationId};

use crate::ConversationRepository;

/// Service for managing conversations, including creation, retrieval, and
/// updates
#[derive(Clone)]
pub struct ForgeConversationService<S> {
    conversation_repository: Arc<S>,
}

impl<S: ConversationRepository> ForgeConversationService<S> {
    /// Creates a new ForgeConversationService with the provided repository
    pub fn new(repo: Arc<S>) -> Self {
        Self { conversation_repository: repo }
    }
}

#[async_trait::async_trait]
impl<S: ConversationRepository> ConversationService for ForgeConversationService<S> {
    async fn modify_conversation<F, T>(&self, id: &ConversationId, f: F) -> Result<T>
    where
        F: FnOnce(&mut Conversation) -> T + Send,
        T: Send,
    {
        let mut conversation = self
            .conversation_repository
            .get_conversation(id)
            .await?
            .ok_or_else(|| forge_app::domain::Error::ConversationNotFound(*id))?;
        let out = f(&mut conversation);
        let _ = self
            .conversation_repository
            .upsert_conversation(conversation)
            .await?;
        Ok(out)
    }

    async fn find_conversation(&self, id: &ConversationId) -> Result<Option<Conversation>> {
        self.conversation_repository.get_conversation(id).await
    }

    async fn upsert_conversation(&self, conversation: Conversation) -> Result<()> {
        let _ = self
            .conversation_repository
            .upsert_conversation(conversation)
            .await?;
        Ok(())
    }

    async fn get_conversations(&self, limit: Option<usize>) -> Result<Option<Vec<Conversation>>> {
        self.conversation_repository
            .get_all_conversations(limit)
            .await
    }

    async fn last_conversation(&self) -> Result<Option<Conversation>> {
        self.conversation_repository.get_last_conversation().await
    }
}
