use axum_extra::extract::cookie::Key;

use super::configurator::env::EnvConfigurator;
use super::not_implemented::NotImplemented;
use super::*;

#[derive(Clone, Copy, Debug)]
pub struct Policy<
    AuthImpl = NotImplemented,
    TokenAuthzImpl = NotImplemented,
    PackageStorageImpl = NotImplemented,
    ConfiguratorImpl = EnvConfigurator,
> where
    AuthImpl: Authenticator + Send + Sync,
    TokenAuthzImpl: TokenAuthorizer + Send + Sync,
    PackageStorageImpl: PackageStorage + Send + Sync,
    ConfiguratorImpl: Configurator + Send + Sync,
{
    auth: AuthImpl,
    token_authz: TokenAuthzImpl,
    package_storage: PackageStorageImpl,
    configurator: ConfiguratorImpl,
}

impl Policy {
    pub fn new() -> Self {
        Self {
            package_storage: NotImplemented,
            auth: NotImplemented,
            token_authz: NotImplemented,
            configurator: EnvConfigurator::new(),
        }
    }
}

impl Default for Policy {
    fn default() -> Self {
        Policy::new()
    }
}

impl<A, T, S, C> Policy<A, T, S, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    S: PackageStorage + Send + Sync,
    C: Configurator + Send + Sync,
{
    pub fn with_authenticator<A1: Authenticator + Send + Sync>(
        self,
        auth: A1,
    ) -> Policy<A1, T, S, C> {
        Policy {
            auth,
            token_authz: self.token_authz,
            package_storage: self.package_storage,
            configurator: self.configurator,
        }
    }

    pub fn with_package_storage<S1: PackageStorage + Send + Sync>(
        self,
        package_storage: S1,
    ) -> Policy<A, T, S1, C> {
        Policy {
            auth: self.auth,
            token_authz: self.token_authz,
            configurator: self.configurator,
            package_storage,
        }
    }

    pub fn with_token_authorizer<T1: TokenAuthorizer + Send + Sync>(
        self,
        token_authz: T1,
    ) -> Policy<A, T1, S, C> {
        Policy {
            auth: self.auth,
            token_authz,
            configurator: self.configurator,
            package_storage: self.package_storage,
        }
    }
}

#[async_trait::async_trait]
impl<AuthenticatorImpl, TokenAuthorizerImpl, PackageStorageImpl, ConfiguratorImpl> Authenticator
    for Policy<AuthenticatorImpl, TokenAuthorizerImpl, PackageStorageImpl, ConfiguratorImpl>
where
    AuthenticatorImpl: Authenticator + Send + Sync,
    TokenAuthorizerImpl: TokenAuthorizer + Send + Sync,
    PackageStorageImpl: PackageStorage + Send + Sync,
    ConfiguratorImpl: Configurator + Send + Sync,
{
    type SessionId = AuthenticatorImpl::SessionId;
    type Response = AuthenticatorImpl::Response;
    type User = AuthenticatorImpl::User;

    async fn start_login_session(&self, req: Request<Body>) -> anyhow::Result<Self::SessionId> {
        self.auth.start_login_session(req).await
    }

    async fn poll_login_session(&self, id: Self::SessionId) -> anyhow::Result<Option<Self::User>> {
        self.auth.poll_login_session(id).await
    }

    async fn complete_login_session<C: Configurator + Send + Sync>(
        &self,
        _config: &C,
        req: Request<Body>,
        id: Option<Self::SessionId>,
    ) -> anyhow::Result<Self::Response> {
        self.auth.complete_login_session(self, req, id).await
    }
}

#[async_trait::async_trait]
impl<A, T, S, C> TokenAuthorizer for Policy<A, T, S, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    S: PackageStorage + Send + Sync,
    C: Configurator + Send + Sync,
{
    type TokenSessionId = T::TokenSessionId;

    async fn start_session(&self, user: User) -> anyhow::Result<Self::TokenSessionId> {
        self.token_authz.start_session(user).await
    }

    async fn authenticate_session_bearer(
        &self,
        req: Self::TokenSessionId,
    ) -> anyhow::Result<Option<User>> {
        self.token_authz.authenticate_session_bearer(req).await
    }

    async fn authenticate_session(&self, req: &Parts) -> anyhow::Result<Option<User>> {
        self.token_authz.authenticate_session(req).await
    }
}

#[async_trait::async_trait]
impl<A, T, S, C> PackageStorage for Policy<A, T, S, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    S: PackageStorage + Send + Sync,
    C: Configurator + Send + Sync,
{
    type Error = S::Error;
    async fn stream_packument(
        &self,
        name: &PackageIdentifier,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        self.package_storage.stream_packument(name).await
    }

    async fn stream_tarball(
        &self,
        name: &PackageIdentifier,
        version: &str,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        self.package_storage.stream_tarball(name, version).await
    }
}

#[async_trait::async_trait]
impl<A, T, S, C> Configurator for Policy<A, T, S, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    S: PackageStorage + Send + Sync,
    C: Configurator + Send + Sync,
{
    fn fqdn(&self) -> &str {
        self.configurator.fqdn()
    }

    async fn oauth_config(&self) -> anyhow::Result<(String, String)> {
        self.configurator.oauth_config().await
    }

    async fn cookie_key(&self) -> anyhow::Result<Key> {
        self.configurator.cookie_key().await
    }
}
