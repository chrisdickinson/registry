use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct Dist {
    pub(crate) shasum: String,
    pub(crate) tarball: String,

    pub(crate) integrity: Option<String>,
    #[serde(rename = "fileCount")]
    pub(crate) file_count: Option<i64>,
    #[serde(rename = "unpackedSize")]
    pub(crate) unpacked_size: Option<i64>,
    #[serde(rename = "npm-signature")]
    pub(crate) npm_signature: Option<String>,

    #[serde(flatten)]
    pub(crate) rest: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize)]
pub struct Version {
    pub(crate) dist: Dist,
    #[serde(rename = "_hasShrinkwrap")]
    pub(crate) has_shrinkwrap: Option<bool>,

    #[serde(flatten)]
    pub(crate) rest: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize)]
pub struct Human {
    pub(crate) name: String,
    pub(crate) email: String,
}

#[derive(Serialize, Deserialize)]
pub struct Packument {
    pub(crate) author: Option<Human>,
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) versions: HashMap<String, Version>,
    pub(crate) time: HashMap<String, DateTime<Utc>>,
    #[serde(rename = "dist-tags")]
    pub(crate) tags: HashMap<String, String>,
    pub(crate) maintainers: Vec<Human>,
    pub(crate) users: Option<HashMap<String, bool>>,

    #[serde(flatten)]
    pub(crate) rest: HashMap<String, Value>,
}

/*
pub enum PackumentAction {
    Create,
    PublishVersion,
    DeprecateVersion,
    UndeprecateVersion,
    UnpublishVersions,
    Unpublish,
    CreateTag,
    DeleteTag,
    AddCollaborator,
    RemoveCollaborator
}
*/
