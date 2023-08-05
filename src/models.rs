mod package_version;
mod packument;
use serde::{Deserialize, Serialize};

pub use packument::*;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct User {
    pub(crate) name: String,
    pub(crate) email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) full_name: Option<String>,
}
