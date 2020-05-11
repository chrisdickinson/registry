use http_types::{ StatusCode, Result };
use std::any::Any;
use crate::auth::storage::AuthnStorage;

#[async_trait::async_trait]
pub trait AuthnScheme<User: Send + Sync + 'static> {
    type Request: Any + Send + Sync;

    async fn authenticate<S>(&self, state: &S, auth_param: &str) -> Result<Option<User>>
        where S: AuthnStorage<User> + Send + Sync + 'static;

    fn header_name() -> &'static str { "Authorization" }
    fn scheme_name() -> &'static str;
}

#[derive(Default, Debug)]
pub struct BasicAuthScheme;

#[derive(Debug)]
pub struct BasicAuthRequest {
    pub username: String,
    pub password: String
}

#[async_trait::async_trait]
impl<User: Send + Sync + 'static> AuthnScheme<User> for BasicAuthScheme {
    type Request = BasicAuthRequest;

    async fn authenticate<S>(&self, state: &S, auth_param: &str) -> Result<Option<User>>
        where S: AuthnStorage<User> + Send + Sync + 'static {
        let bytes = base64::decode(auth_param);
        if bytes.is_err() {
            // This is invalid. Fail the request.
            return Err(http_types::Error::from_str(StatusCode::Unauthorized, "Basic auth param must be valid base64."));
        }

        let as_utf8 = String::from_utf8(bytes.unwrap());
        if as_utf8.is_err() {
            // You know the drill.
            return Err(http_types::Error::from_str(StatusCode::Unauthorized, "Basic auth param base64 must contain valid utf-8."));
        }

        let as_utf8 = as_utf8.unwrap();
        let parts: Vec<_> = as_utf8.split(":").collect();

        if parts.len() < 2 {
            return Ok(None)
        }

        let (username, password) = (parts[0], parts[1]);

        let user = state.maybe_get_user(Box::new(BasicAuthRequest {
            username: username.to_owned(),
            password: password.to_owned()
        })).await?;

        Ok(user)
    }

    fn scheme_name() -> &'static str { "Basic " }
}

#[derive(Default, Debug)]
pub struct BearerAuthScheme {
    prefix: String
}

pub struct BearerAuthRequest {
    pub token: String
}

#[async_trait::async_trait]
impl<User: Send + Sync + 'static> AuthnScheme<User> for BearerAuthScheme {
    type Request = BearerAuthRequest;

    async fn authenticate<S>(&self, state: &S, auth_param: &str) -> Result<Option<User>>
        where S: AuthnStorage<User> + Send + Sync + 'static {

        if !auth_param.starts_with(self.prefix.as_str()) {
            return Ok(None)
        }

        // validate that the auth_param (sans the prefix) is a valid uuid.

        // fetch the user from ... somewhere?
        let user = state.maybe_get_user(Box::new(BearerAuthRequest {
            token: (&auth_param[self.prefix.len()..]).to_owned()
        })).await?;
        Ok(user)
    }

    fn scheme_name() -> &'static str { "Bearer " }
}


