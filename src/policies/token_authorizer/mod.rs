use std::{fmt::Display, hash::Hash, str::FromStr};

use axum::http::request::Parts;
use chrono::{DateTime, Utc};

use crate::models::User;

pub(crate) mod in_memory;

#[derive(Clone, Debug)]
pub(crate) struct TokenSession {
    initialized_at: DateTime<Utc>,
    user: User,
}

#[async_trait::async_trait]
pub trait TokenAuthorizer {
    type TokenSessionId: Hash + FromStr + Display + Clone + Send + Sync;

    async fn authenticate_session_bearer(
        &self,
        _bearer: Self::TokenSessionId,
    ) -> anyhow::Result<Option<User>> {
        Ok(None)
    }

    async fn start_session(&self, user: User) -> anyhow::Result<Self::TokenSessionId>;

    async fn authenticate_session(&self, req: &Parts) -> anyhow::Result<Option<User>> {
        let Some(authentication) = req.headers.get("authorization") else {
            return Ok(None);
        };

        let Ok(authentication) = authentication.to_str() else {
            return Ok(None);
        };

        let Some(authentication) = authentication.strip_prefix("Bearer ").or_else(|| authentication.strip_prefix("bearer ")) else {
            return Ok(None);
        };

        let Ok(token): Result<Self::TokenSessionId, _> = authentication.trim().parse() else {
            return Ok(None);
        };

        self.authenticate_session_bearer(token).await
    }
}
