use crate::auth::{ BasicAuthRequest, BearerAuthRequest };
use http_types::Result; 
use std::any::Any;

/// A storage provider. It must define an associated `Request` type, representing
/// a struct or parameter sent by a corresponding `Scheme`.
#[async_trait::async_trait]
pub trait AuthnStorage<User: Send + Sync + 'static, Request: Any + Send + Sync + 'static> {
    #[doc(hidden)]
    async fn maybe_get_user(&self, request: Box<dyn Any + Send + Sync + 'static>) -> Result<Option<User>> {
        match (request as Box<dyn Any + Send + 'static>).downcast::<Request>() {
            Ok(boxed) => self.get_user(*boxed).await,
            Err(_) => Ok(None)
        }
    }

    async fn get_user(&self, _request: Request) -> Result<Option<User>> {
        Ok(None)
    }
}
