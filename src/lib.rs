mod extractors;
mod handlers;
mod layers;
mod models;
mod operations;

pub use handlers::v1::routes;
pub use operations::policy::Policy;

pub use operations::{Authenticator, Configurator, PackageStorage, TokenAuthorizer};

pub mod services {
    pub mod token_authorizers {
        pub use crate::operations::token_authorizer::in_memory::InMemoryTokenAuthorizer as InMemory;
    }

    pub mod authenticators {
        pub use crate::operations::authenticator::oauth::OAuthAuthenticator as OAuth;
    }

    pub mod configurators {
        pub use crate::operations::configurator::env::EnvConfigurator as Env;
    }

    pub mod storage {
        pub mod package {
            pub use crate::operations::package_storage::read_through::ReadThrough;
            pub use crate::operations::package_storage::remote::RemoteRegistry;
        }

        pub mod user {
            pub use crate::operations::user_storage::in_memory::InMemoryUserStorage as InMemory;
        }
    }
}
