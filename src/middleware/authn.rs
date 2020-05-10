use tide::{Middleware, Next, Request, Response};
use http_types::{ StatusCode, Result };
use futures::future::BoxFuture;

pub struct User {
    pub(crate) username: String,
    pub(crate) email: String
}

#[async_trait::async_trait]
pub trait AuthenticationStorage<Request> {
    async fn get_user(&self, request: Request) -> Result<Option<User>>;
}

#[derive(Default, Debug)]
pub struct SimpleBasicStorage;

#[async_trait::async_trait]
impl AuthenticationStorage<BasicAuthRequest> for SimpleBasicStorage {
    async fn get_user(&self, request: BasicAuthRequest) -> Result<Option<User>> {
        if request.username == "chris" && request.password == "applecat1" { 
            Ok(Some(User { username: "chris".to_owned(), email: "chris@neversaw.us".to_owned() }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Default, Debug)]
pub struct SimpleBearerStorage;

#[async_trait::async_trait]
impl AuthenticationStorage<BearerAuthRequest> for SimpleBearerStorage {
    async fn get_user(&self, request: BearerAuthRequest) -> Result<Option<User>> {
        if request.token == "r_9e768f7a-8ab3-4c15-81ea-34a37e29b215" { 
            Ok(Some(User { username: "chris".to_owned(), email: "chris@neversaw.us".to_owned() }))
        } else {
            Ok(None)
        }
    }
}

#[async_trait::async_trait]
impl<L> AuthenticationStorage<L> for ()
    where L: Send + Sync + 'static {
    async fn get_user(&self, request: L) -> Result<Option<User>> {
        Ok(None)
    }
}

#[async_trait::async_trait]
impl<RequestL, RequestR, ReceiverL, ReceiverR> AuthenticationStorage<RequestR> for (ReceiverL, ReceiverR)
    where RequestL: Send + Sync + 'static,
          RequestR: Send + Sync + 'static,
          ReceiverL: AuthenticationStorage<RequestL> + Send + Sync + 'static,
          ReceiverR: AuthenticationStorage<RequestR> + Send + Sync + 'static, {
    async fn get_user(&self, request: RequestR) -> Result<Option<User>> {
        self.1.get_user(request).await
    }
}

#[async_trait::async_trait]
impl<RequestL, RequestR, ReceiverL, ReceiverR> AuthenticationStorage<RequestL> for (ReceiverL, ReceiverR)
    where RequestL: Send + Sync + 'static,
          RequestR: Send + Sync + 'static,
          ReceiverL: AuthenticationStorage<RequestL> + Send + Sync + 'static,
          ReceiverR: AuthenticationStorage<RequestR> + Send + Sync + 'static, {
    async fn get_user(&self, request: RequestL) -> Result<Option<User>> {
        self.0.get_user(request).await
    }
}


#[async_trait::async_trait]
pub trait SupportsAuthenticationScheme {
    type Request;

    async fn authenticate<S>(&self, state: &S, auth_param: &str) -> Result<Option<User>>
        where S: AuthenticationStorage<Self::Request> + Send + Sync + 'static;

    fn header_name() -> &'static str { "Authorization" }
    fn scheme_name() -> &'static str;
}

#[derive(Default, Debug)]
pub struct BasicAuthScheme;

pub struct BasicAuthRequest {
    username: String,
    password: String
}

#[async_trait::async_trait]
impl SupportsAuthenticationScheme for BasicAuthScheme {
    type Request = BasicAuthRequest;

    async fn authenticate<S>(&self, state: &S, auth_param: &str) -> Result<Option<User>>
        where S: AuthenticationStorage<Self::Request> + Send + Sync + 'static {
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
        let user = state.get_user(BasicAuthRequest {
            username: username.to_owned(),
            password: password.to_owned()
        }).await?;

        Ok(None)
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
impl SupportsAuthenticationScheme for BearerAuthScheme {
    type Request = BearerAuthRequest;

    async fn authenticate<S>(&self, state: &S, auth_param: &str) -> Result<Option<User>>
        where S: AuthenticationStorage<Self::Request> + Send + Sync + 'static {

        if !auth_param.starts_with(self.prefix.as_str()) {
            return Ok(None)
        }

        // validate that the auth_param (sans the prefix) is a valid uuid.

        // fetch the user from ... somewhere?
        let user = state.get_user(BearerAuthRequest {
            token: (&auth_param[self.prefix.len()..]).to_owned()
        }).await?;
        Ok(None)
    }

    fn scheme_name() -> &'static str { "Bearer " }
}

// this is the middleware!
pub struct Authentication<T: SupportsAuthenticationScheme> {
    pub(crate) scheme: T,
    header_name: http_types::headers::HeaderName,
}

impl<T: SupportsAuthenticationScheme> std::fmt::Debug for Authentication<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(formatter, "Authentication<Scheme>");
        Ok(())
    }
}

impl<T: SupportsAuthenticationScheme> Authentication<T> {
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
    where Scheme: SupportsAuthenticationScheme + Send + Sync + 'static,
          State: AuthenticationStorage<Scheme::Request> + Send + Sync + 'static {
    fn handle<'a>(
        &'a self,
        mut cx: Request<State>,
        next: Next<'a, State>,
    ) -> BoxFuture<'a, Result<Response>> {
        Box::pin(async move {
            // read the header
            let auth_header = cx.header(self.header_name());
            if auth_header.is_none() {
                return next.run(cx).await;
            }
            let value = auth_header.unwrap();

            if value.is_empty() {
                return next.run(cx).await;
            }

            if value.len() > 1 {
                // including multiple basic auth headers is... uh, a little weird.
                // fail the request.
                return Ok(Response::new(StatusCode::Unauthorized));
            }

            let value = value[0].as_str();
            if !value.starts_with(self.scheme_name()) {
                return next.run(cx).await;
            }

            let auth_param = &value[self.scheme_name().len()..];
            let state = cx.state();
            let maybe_user = self.scheme.authenticate(state, auth_param).await?;

            if let Some(user) = maybe_user {
                cx = cx.set_local(user);
            }

            return next.run(cx).await;
        })
    }
}
