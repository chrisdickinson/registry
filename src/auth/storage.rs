use crate::auth::{ BasicAuthRequest, BearerAuthRequest };
use http_types::Result; 
use crate::auth::User;
use std::any::Any;

/// A storage provider. It must define an associated `Request` type, representing
/// a struct or parameter sent by a corresponding `Scheme`.
#[async_trait::async_trait]
pub trait AuthnStorage<User: Send + Sync + 'static> {
    type Request: Any + Send + Sync + 'static;

    #[doc(hidden)]
    async fn maybe_get_user(&self, request: Box<dyn Any + Send + Sync + 'static>) -> Result<Option<User>> {
        match (request as Box<dyn Any + Send + 'static>).downcast::<Self::Request>() {
            Ok(boxed) => self.get_user(*boxed).await,
            Err(_) => Ok(None)
        }
    }

    async fn get_user(&self, _request: Self::Request) -> Result<Option<User>> {
        Ok(None)
    }
}

#[async_trait::async_trait]
impl<User: Send + Sync + 'static> AuthnStorage<User> for () {
    type Request = ();

    async fn get_user(&self, _request: ()) -> Result<Option<User>> {
        Ok(None)
    }
}

/// Support nested tuples as an authentication store, e.g.,:
/// ```no_run
/// let state = (
///     BasicStorage, (
///         BearerStorage,
///         SomeOtherStorage
///     )
/// );
/// let app = tide::with_state(state);
/// ```
#[async_trait::async_trait]
impl<LHS, RHS, User> AuthnStorage<User> for (LHS, RHS)
    where LHS: AuthnStorage<User> + Send + Sync,
          RHS: AuthnStorage<User> + Send + Sync,
          User: Send + Sync + 'static {
    type Request = Box<dyn Any + Send + Sync + 'static>;

    async fn maybe_get_user(&self, request: Box<dyn Any + Send + Sync + 'static>) -> Result<Option<User>> {
        if request.is::<LHS::Request>() {
            self.0.maybe_get_user(request).await
        } else {
            self.1.maybe_get_user(request).await
        }
    }

    async fn get_user(&self, request: Self::Request) -> Result<Option<User>> {
        Ok(None)
    }
}

#[derive(Default, Debug)]
pub struct SimpleBasicStorage;

#[async_trait::async_trait]
impl AuthnStorage<User> for SimpleBasicStorage {
    type Request = BasicAuthRequest;

    async fn get_user(&self, request: Self::Request) -> Result<Option<User>> {
        // Lest you worry, this is a fake password.
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
impl AuthnStorage<User> for SimpleBearerStorage {
    type Request = BearerAuthRequest;

    async fn get_user(&self, request: Self::Request) -> Result<Option<User>> {
        if request.token == "r_9e768f7a-8ab3-4c15-81ea-34a37e29b215" {
            Ok(Some(User { username: "chris".to_owned(), email: "chris@neversaw.us".to_owned() }))
        } else {
            Ok(None)
        }
    }
}

