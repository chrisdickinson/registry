use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct Dist {
    shasum: String,
    tarball: String,

    integrity: Option<String>,
    #[serde(rename = "fileCount")]
    file_count: Option<i64>,
    #[serde(rename = "unpackedSize")]
    unpacked_size: Option<i64>,
    #[serde(rename = "npm-signature")]
    npm_signature: Option<String>,

    #[serde(flatten)]
    rest: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize)]
pub struct Version {
    dist: Dist,
    #[serde(rename = "_hasShrinkwrap")]
    has_shrinkwrap: Option<bool>,

    #[serde(flatten)]
    rest: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize)]
pub struct Human {
    name: String,
    email: String,
}

#[derive(Serialize, Deserialize)]
pub struct Packument {
    author: Option<Human>,
    name: String,
    description: Option<String>,
    versions: HashMap<String, Version>,
    time: HashMap<String, DateTime<Utc>>,
    #[serde(rename = "dist-tags")]
    tags: HashMap<String, String>,
    maintainers: Vec<Human>,
    users: Option<Vec<String>>,

    #[serde(flatten)]
    rest: HashMap<String, Value>,
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
