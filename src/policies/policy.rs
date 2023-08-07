use super::configurator::env::EnvConfigurator;
use super::not_implemented::NotImplemented;
use super::*;

pub trait PolicyHolder {
    type Authenticator: Authenticator + Send + Sync;
    type TokenAuthorizer: TokenAuthorizer + Send + Sync;
    type UserStorage: UserStorage + Send + Sync;
    type PackageStorage: PackageStorage + Send + Sync;
    type Configurator: Configurator + Send + Sync;

    fn as_authenticator(&self) -> &Self::Authenticator;
    fn as_token_authorizer(&self) -> &Self::TokenAuthorizer;
    fn as_user_storage(&self) -> &Self::UserStorage;
    fn as_package_storage(&self) -> &Self::PackageStorage;
    fn as_configurator(&self) -> &Self::Configurator;
}

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

impl<A, T, U, P, C> PolicyHolder for Policy<A, T, U, P, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    U: UserStorage + Send + Sync,
    P: PackageStorage + Send + Sync,
    C: Configurator + Send + Sync,
{
    type Authenticator = A;

    type TokenAuthorizer = T;

    type UserStorage = U;

    type PackageStorage = P;

    type Configurator = C;

    fn as_authenticator(&self) -> &Self::Authenticator {
        &self.auth
    }

    fn as_token_authorizer(&self) -> &Self::TokenAuthorizer {
        &self.token_authz
    }

    fn as_user_storage(&self) -> &Self::UserStorage {
        &self.user_storage
    }

    fn as_package_storage(&self) -> &Self::PackageStorage {
        &self.package_storage
    }

    fn as_configurator(&self) -> &Self::Configurator {
        &self.configurator
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
