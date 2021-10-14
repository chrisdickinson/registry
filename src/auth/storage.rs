use http_types::Result; 
use std::any::Any;

/// A storage provider. It must define an associated `Request` type, representing
/// a struct or parameter sent by a corresponding `Scheme`.
#[async_trait::async_trait]
pub trait AuthnStorage<User: Send + Sync + 'static, Request: Any + Send + Sync + 'static> {
    async fn get_user(&self, _request: Request) -> Result<Option<User>> {
        Ok(None)
    }
}
