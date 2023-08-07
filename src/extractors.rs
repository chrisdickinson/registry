use axum::{extract::FromRequestParts, http::request::Parts, http::StatusCode, Json};

use crate::{
    models::User,
    policies::{policy::PolicyHolder, TokenAuthorizer},
};

pub(crate) struct Authenticated(pub User);

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for Authenticated
where
    S: Send + Sync + PolicyHolder,
{
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match state
            .as_token_authorizer()
            .authenticate_session(parts)
            .await
        {
            Ok(Some(user)) => Ok(Authenticated(user)),
            Ok(None) => Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "message": "you must be logged in to use this endpoint"
                })),
            )),
            Err(e) => {
                tracing::error!(?parts, error = ?e, "encountered internal error while attempting to authenticate session");
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "message": "you must be logged in to use this endpoint"
                    })),
                ))
            }
        }
    }
}
