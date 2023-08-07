use super::*;

trait Unimplemented: Send + Sync {}

#[derive(Clone, Copy, Debug, Default)]
pub struct NotImplemented;

impl Unimplemented for NotImplemented {}

#[async_trait::async_trait]
impl<T: Unimplemented> Authenticator for T {
    type SessionId = String;
    type Response = String;
    type User = User;

    async fn start_login_session(&self, _req: Request<Body>) -> anyhow::Result<Self::SessionId> {
        Err(anyhow::anyhow!("not implemented"))
    }

    async fn poll_login_session(&self, _id: Self::SessionId) -> anyhow::Result<Option<Self::User>> {
        Err(anyhow::anyhow!("not implemented"))
    }

    async fn complete_login_session<C: Configurator + Send + Sync>(
        &self,
        _config: &C,
        _req: Request<Body>,
        _id: Option<Self::SessionId>,
    ) -> anyhow::Result<Self::Response> {
        Err(anyhow::anyhow!("not implemented"))
    }
}

#[async_trait::async_trait]
impl<T: Unimplemented> TokenAuthorizer for T {
    type TokenSessionId = String;

    async fn start_session(&self, _user: User) -> anyhow::Result<Self::TokenSessionId> {
        Err(anyhow::anyhow!("not implemented"))
    }

    async fn authenticate_session(&self, _req: &Parts) -> anyhow::Result<Option<User>> {
        Err(anyhow::anyhow!("not implemented"))
    }
}

#[async_trait::async_trait]
impl<T: Unimplemented> PackageStorage for T {
    type Error = anyhow::Error;
    async fn stream_packument(
        &self,
        _name: &PackageIdentifier,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        Err(anyhow::anyhow!("not implemented"))
    }

    async fn stream_tarball(
        &self,
        _name: &PackageIdentifier,
        _version: &str,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        Err(anyhow::anyhow!("not implemented"))
    }
}
