use axum_extra::extract::cookie::Key;
use serde::Serialize;

use super::configurator::env::EnvConfigurator;
use super::not_implemented::NotImplemented;
use super::*;

#[derive(Clone, Copy, Debug)]
pub struct Policy<
    AuthImpl = NotImplemented,
    TokenAuthzImpl = NotImplemented,
    UserStorageImpl = NotImplemented,
    PackageStorageImpl = NotImplemented,
    ConfiguratorImpl = EnvConfigurator,
> where
    AuthImpl: Authenticator + Send + Sync,
    TokenAuthzImpl: TokenAuthorizer + Send + Sync,
    UserStorageImpl: UserStorage + Send + Sync,
    PackageStorageImpl: PackageStorage + Send + Sync,
    ConfiguratorImpl: Configurator + Send + Sync,
{
    auth: AuthImpl,
    token_authz: TokenAuthzImpl,
    user_storage: UserStorageImpl,
    package_storage: PackageStorageImpl,
    configurator: ConfiguratorImpl,
}

impl Policy {
    pub fn new() -> Self {
        Self {
            package_storage: NotImplemented,
            user_storage: NotImplemented,
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

impl<A, T, U, P, C> Policy<A, T, U, P, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    U: UserStorage + Send + Sync,
    P: PackageStorage + Send + Sync,
    C: Configurator + Send + Sync,
{
    pub fn with_authenticator<A1: Authenticator + Send + Sync>(
        self,
        auth: A1,
    ) -> Policy<A1, T, U, P, C> {
        Policy {
            auth,
            token_authz: self.token_authz,
            package_storage: self.package_storage,
            user_storage: self.user_storage,
            configurator: self.configurator,
        }
    }

    pub fn with_package_storage<P1: PackageStorage + Send + Sync>(
        self,
        package_storage: P1,
    ) -> Policy<A, T, U, P1, C> {
        Policy {
            auth: self.auth,
            token_authz: self.token_authz,
            configurator: self.configurator,
            user_storage: self.user_storage,
            package_storage,
        }
    }

    pub fn with_user_storage<U1: UserStorage + Send + Sync>(
        self,
        user_storage: U1,
    ) -> Policy<A, T, U1, P, C> {
        Policy {
            auth: self.auth,
            token_authz: self.token_authz,
            configurator: self.configurator,
            user_storage,
            package_storage: self.package_storage,
        }
    }

    pub fn with_token_authorizer<T1: TokenAuthorizer + Send + Sync>(
        self,
        token_authz: T1,
    ) -> Policy<A, T1, U, P, C> {
        Policy {
            auth: self.auth,
            token_authz,
            configurator: self.configurator,
            user_storage: self.user_storage,
            package_storage: self.package_storage,
        }
    }
}

#[async_trait::async_trait]
impl<A, T, U, P, C> Authenticator for Policy<A, T, U, P, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    U: UserStorage + Send + Sync,
    P: PackageStorage + Send + Sync,
    C: Configurator + Send + Sync,
{
    type SessionId = A::SessionId;
    type Response = A::Response;
    type User = A::User;

    async fn start_login_session(&self, req: Request<Body>) -> anyhow::Result<Self::SessionId> {
        self.auth.start_login_session(req).await
    }

    async fn poll_login_session(&self, id: Self::SessionId) -> anyhow::Result<Option<Self::User>> {
        self.auth.poll_login_session(id).await
    }

    async fn complete_login_session<C0: Configurator + Send + Sync>(
        &self,
        _config: &C0,
        req: Request<Body>,
        id: Option<Self::SessionId>,
    ) -> anyhow::Result<Self::Response> {
        self.auth.complete_login_session(self, req, id).await
    }
}

#[async_trait::async_trait]
impl<A, T, U, P, C> TokenAuthorizer for Policy<A, T, U, P, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    U: UserStorage + Send + Sync,
    P: PackageStorage + Send + Sync,
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
impl<A, T, U, P, C> PackageStorage for Policy<A, T, U, P, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    U: UserStorage + Send + Sync,
    P: PackageStorage + Send + Sync,
    C: Configurator + Send + Sync,
{
    type Error = P::Error;
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
impl<A, T, U, P, C> Configurator for Policy<A, T, U, P, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    U: UserStorage + Send + Sync,
    P: PackageStorage + Send + Sync,
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

#[async_trait::async_trait]
impl<A, T, U, P, C> UserStorage for Policy<A, T, U, P, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    U: UserStorage + Send + Sync,
    P: PackageStorage + Send + Sync,
    C: Configurator + Send + Sync,
{
    async fn register_user<UserImpl: Into<User> + Serialize + Send + Sync>(
        &self,
        user: UserImpl,
    ) -> anyhow::Result<User> {
        self.user_storage.register_user(user).await
    }

    async fn get_user(&self, username: &str) -> anyhow::Result<User> {
        self.user_storage.get_user(username).await
    }

    async fn list_users(&self) -> anyhow::Result<Vec<User>> {
        self.user_storage.list_users().await
    }
}
