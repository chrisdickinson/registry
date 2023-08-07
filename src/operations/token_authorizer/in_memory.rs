use std::collections::HashMap;

use std::sync::Arc;

use crate::models::User;
use crate::operations::TokenAuthorizer;

use chrono::Utc;

use futures_util::StreamExt;

use tokio::sync::RwLock;

use uuid::Uuid;

use super::TokenSession;

#[derive(Clone)]
pub struct InMemoryTokenAuthorizer {
    token_sessions: Arc<RwLock<HashMap<Uuid, TokenSession>>>,
}

impl std::fmt::Debug for InMemoryTokenAuthorizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut formatter = f.debug_struct("InMemoryTokenAuthorizer");
        if let Ok(sessions) = self.token_sessions.try_read() {
            formatter.field("token_sessions", &sessions);
        }
        formatter.finish()
    }
}

impl InMemoryTokenAuthorizer {
    pub fn new() -> Self {
        Self {
            token_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryTokenAuthorizer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TokenAuthorizer for InMemoryTokenAuthorizer {
    type TokenSessionId = Uuid;
    async fn start_session(&self, user: User) -> anyhow::Result<Self::TokenSessionId> {
        let key = Uuid::new_v4();
        self.token_sessions.write().await.insert(
            key,
            TokenSession {
                initialized_at: Utc::now(),
                user,
            },
        );

        Ok(key)
    }

    async fn authenticate_session_bearer(
        &self,
        token: Self::TokenSessionId,
    ) -> anyhow::Result<Option<User>> {
        let sessions = self.token_sessions.read().await;
        let session = sessions.get(&token);

        Ok(session.map(|sess| sess.user.clone()))
    }
}
