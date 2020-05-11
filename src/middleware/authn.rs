use tide::{Middleware, Next, Request, Response};
use http_types::{ StatusCode, Result };
use futures::future::BoxFuture;
use tracing::{info, error};
use std::any::Any;

pub struct User {
    pub(crate) username: String,
    pub(crate) email: String
}

#[async_trait::async_trait]
pub trait AuthnStorage {
    type Request: Any + Send + Sync + 'static;

    async fn get_user(&self, request: Box<dyn Any + Send + Sync + 'static>) -> Result<Option<User>>;
}


#[async_trait::async_trait]
impl<LHS, RHS> AuthnStorage for (LHS, RHS)
    where LHS: AuthnStorage + Send + Sync,
          RHS: AuthnStorage + Send + Sync {
    type Request = Box<dyn Any + Send + Sync + 'static>;

    async fn get_user(&self, request: Box<dyn Any + Send + Sync + 'static>) -> Result<Option<User>> {
        if request.is::<LHS::Request>() {
            return self.0.get_user(request).await;
        }

        self.1.get_user(request).await
    }
}

#[derive(Default, Debug)]
pub struct SimpleBasicStorage;

#[async_trait::async_trait]
impl AuthnStorage for SimpleBasicStorage {
    type Request = BasicAuthRequest;

    async fn get_user(&self, request: Box<dyn Any + Send + Sync + 'static>) -> Result<Option<User>> {
        match request.downcast_ref::<Self::Request>() {
            Some(req) => {
                if req.username == "chris" && req.password == "applecat1" { 
                    Ok(Some(User { username: "chris".to_owned(), email: "chris@neversaw.us".to_owned() }))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None)
        }
    }
}

#[derive(Default, Debug)]
pub struct SimpleBearerStorage;

#[async_trait::async_trait]
impl AuthnStorage for SimpleBearerStorage {
    type Request = BearerAuthRequest;

    async fn get_user(&self, request: Box<dyn Any + Send + Sync + 'static>) -> Result<Option<User>> {
        match request.downcast_ref::<Self::Request>() {
            Some(req) => {
                if req.token == "r_9e768f7a-8ab3-4c15-81ea-34a37e29b215" {
                    Ok(Some(User { username: "chris".to_owned(), email: "chris@neversaw.us".to_owned() }))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None)
        }
    }
}

#[async_trait::async_trait]
pub trait AuthnScheme {
    type Request: Any + Send + Sync;

    async fn authenticate<S>(&self, state: &S, auth_param: &str) -> Result<Option<User>>
        where S: AuthnStorage + Send + Sync + 'static;

    fn header_name() -> &'static str { "Authorization" }
    fn scheme_name() -> &'static str;
}

#[derive(Default, Debug)]
pub struct BasicAuthScheme;

#[derive(Debug)]
pub struct BasicAuthRequest {
    username: String,
    password: String
}

#[async_trait::async_trait]
impl AuthnScheme for BasicAuthScheme {
    type Request = BasicAuthRequest;

    async fn authenticate<S>(&self, state: &S, auth_param: &str) -> Result<Option<User>>
        where S: AuthnStorage + Send + Sync + 'static {
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

        let user = state.get_user(Box::new(BasicAuthRequest {
            username: username.to_owned(),
            password: password.to_owned()
        }) as Box<_>).await?;

        Ok(user)
    }

    fn scheme_name() -> &'static str { "Basic " }
}

#[derive(Default, Debug)]
pub struct BearerAuthScheme {
    prefix: String
}

pub struct BearerAuthRequest {
    token: String
}

#[async_trait::async_trait]
impl AuthnScheme for BearerAuthScheme {
    type Request = BearerAuthRequest;

    async fn authenticate<S>(&self, state: &S, auth_param: &str) -> Result<Option<User>>
        where S: AuthnStorage + Send + Sync + 'static {

        if !auth_param.starts_with(self.prefix.as_str()) {
            return Ok(None)
        }

        // validate that the auth_param (sans the prefix) is a valid uuid.

        // fetch the user from ... somewhere?
        let user = state.get_user(Box::new(BearerAuthRequest {
            token: (&auth_param[self.prefix.len()..]).to_owned()
        })).await?;
        Ok(user)
    }

    fn scheme_name() -> &'static str { "Bearer " }
}

// this is the middleware!
pub struct Authentication<T: AuthnScheme> {
    pub(crate) scheme: T,
    header_name: http_types::headers::HeaderName,
}

impl<T: AuthnScheme> std::fmt::Debug for Authentication<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(formatter, "Authentication<Scheme>")?;
        Ok(())
    }
}

impl<T: AuthnScheme> Authentication<T> {
    pub fn new(scheme: T) -> Self {
        Self {
            scheme,

            // XXX: parse this once, at instantiation of the middleware
            header_name: T::header_name().parse().unwrap(),
        }
    }

    fn header_name(&self) -> &http_types::headers::HeaderName {
        &self.header_name
    }

    fn scheme_name(&self) -> &str {
        T::scheme_name()
    }
}

impl<Scheme, State> Middleware<State> for Authentication<Scheme> 
    where Scheme: AuthnScheme + Send + Sync + 'static,
          State: AuthnStorage + Send + Sync + 'static {
    fn handle<'a>(
        &'a self,
        mut cx: Request<State>,
        next: Next<'a, State>,
    ) -> BoxFuture<'a, Result<Response>> {
        Box::pin(async move {
            // read the header
            let auth_header = cx.header(self.header_name());
            if auth_header.is_none() {
                info!("no auth header, proceeding");
                return next.run(cx).await;
            }
            let value = auth_header.unwrap();

            if value.is_empty() {
                info!("empty auth header, proceeding");
                return next.run(cx).await;
            }

            if value.len() > 1 {
                // including multiple basic auth headers is... uh, a little weird.
                // fail the request.
                error!("multiple auth headers, bailing");
                return Ok(Response::new(StatusCode::Unauthorized));
            }

            let value = value[0].as_str();
            if !value.starts_with(self.scheme_name()) {
                info!("not our auth header");
                return next.run(cx).await;
            }

            let auth_param = &value[self.scheme_name().len()..];
            let state = cx.state();

            info!("saw auth header, attempting to auth");
            // we need to grab the appropriate state! state may be
            let maybe_user = self.scheme.authenticate(state, auth_param).await?;

            if let Some(user) = maybe_user {
                cx = cx.set_local(user);
            } else {
                error!("Authorization header sent but no user returned, bailing");
                return Ok(Response::new(StatusCode::Unauthorized));
            }

            return next.run(cx).await;
        })
    }
}
