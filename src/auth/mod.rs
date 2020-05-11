mod storage;
mod scheme;

pub use scheme::*;
pub use storage::*;

pub struct User {
    pub(crate) username: String,
    pub(crate) email: String
}
