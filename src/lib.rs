mod extractors;
mod handlers;
mod layers;
mod models;
mod policies;

pub use handlers::v1::routes;
pub use policies::policy::Policy;

pub use policies::{Authenticator, Configurator, PackageStorage, TokenAuthorizer};

pub mod policy {
    pub mod token_authorizers {
        pub use crate::policies::token_authorizer::in_memory::InMemoryTokenAuthorizer as InMemory;
    }

    pub mod authenticators {
        pub use crate::policies::authenticator::oauth::OAuthAuthenticator as OAuth;
    }

    pub mod configurators {
        pub use crate::policies::configurator::env::EnvConfigurator as Env;
    }

    pub mod storage {
        pub mod package {
            pub use crate::policies::package_storage::read_through::ReadThrough;
            pub use crate::policies::package_storage::remote::RemoteRegistry;
        }

        pub mod user {
            pub use crate::policies::user_storage::in_memory::InMemoryUserStorage as InMemory;
        }
    }
}
