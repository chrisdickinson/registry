use tide::{Middleware, Next, Request, Response};
use http_types::{ StatusCode, Result };
use futures::future::BoxFuture;
use tracing::{info, error};
use crate::auth::{ AuthnScheme, AuthnStorage };
use std::marker::PhantomData;

// this is the middleware!
pub struct Authentication<User: Send + Sync + 'static, Scheme: AuthnScheme<User>> {
    pub(crate) scheme: Scheme,
    header_name: http_types::headers::HeaderName,
    _user_t: PhantomData<User>
}

impl<User: Send + Sync + 'static, Scheme: AuthnScheme<User>> std::fmt::Debug for Authentication<User, Scheme> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(formatter, "Authentication<Scheme>")?;
        Ok(())
    }
}

impl<User: Send + Sync + 'static, Scheme: AuthnScheme<User>> Authentication<User, Scheme> {
    pub fn new(scheme: Scheme) -> Self {
        Self {
            scheme,

            // XXX: parse this once, at instantiation of the middleware
            header_name: Scheme::header_name().parse().unwrap(),
            _user_t: PhantomData::default()
        }
    }

    fn header_name(&self) -> &http_types::headers::HeaderName {
        &self.header_name
    }

    fn scheme_name(&self) -> &str {
        Scheme::scheme_name()
    }
}

impl<Scheme, State, User> Middleware<State> for Authentication<User, Scheme>
    where Scheme: AuthnScheme<User> + Send + Sync + 'static,
          State: AuthnStorage<User, Scheme::Request> + Send + Sync + 'static,
          User: Send + Sync + 'static {
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
