use std::{fmt::Display, hash::Hash, str::FromStr};

use axum::{body::Body, http::Request, response::IntoResponse};
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::models::User;
use crate::operations::Configurator;

pub(crate) mod oauth;

#[derive(Clone, Debug)]
pub(crate) struct LoginSession {
    initialized_at: DateTime<Utc>,
    user: Option<User>,
    hostname: Option<String>,
    csrftoken: Option<String>,
}

#[async_trait::async_trait]
pub trait Authenticator: Send + Sync {
    type SessionId: Hash + FromStr + Display + Clone + Send + Sync;
    type Response: IntoResponse + Send + Sync;
    type User: Into<User> + Serialize + Send + Sync;

    async fn start_login_session(&self, req: Request<Body>) -> anyhow::Result<Self::SessionId>;

    async fn poll_login_session(
        &self,
        session: Self::SessionId,
    ) -> anyhow::Result<Option<Self::User>>;

    async fn complete_login_session<C: Configurator + Send + Sync>(
        &self,
        config: &C,
        req: Request<Body>,
        session: Option<Self::SessionId>,
    ) -> anyhow::Result<Self::Response>;

    async fn get_user(&self, _username: &str) -> anyhow::Result<Option<User>> {
        Ok(None)
    }
}
