use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

type PackageIdentifier = String;

// Many thanks to Ryan Day for putting together type definitions
// for common NPM objects. [1]
// [1]: https://github.com/npm/types/blob/7f357f45e2b4205cd8474339a95092a5e6e77917/index.d.ts

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Maintainer {
    Byline(String),
    Object {
        name: Option<String>,
        email: Option<String>,
        url: Option<String>,
    },
}

#[derive(Serialize, Deserialize)]
struct Dist {
    tarball: String,
    shasum: String,
    integrity: Option<String>,

    #[serde(rename = "fileCount")]
    file_count: Option<usize>,

    #[serde(rename = "unpackedSize")]
    unpacked_size: Option<usize>,

    #[serde(rename = "npm-signature")]
    npm_signature: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct PackumentVersion {
    #[serde(rename = "gitHead")]
    git_head: Option<String>,
    id: String,
    #[serde(rename = "npmVersion")]
    npm_version: Option<String>,

    #[serde(rename = "nodeVersion")]
    node_version: Option<String>,

    #[serde(rename = "npm_user")]
    npm_user: Maintainer,
    maintainers: Vec<Maintainer>,

    dist: Dist,

    #[serde(rename = "_hasShrinkwrap")]
    has_shrinkwrap: Option<bool>,

    types: Option<String>,

    #[serde(flatten)]
    meta: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct PackumentTime {
    created: DateTime<Utc>,
    modified: DateTime<Utc>,
    versions: HashMap<String, DateTime<Utc>>,
}

#[derive(Serialize, Deserialize)]
struct DistTags {
    latest: Option<String>,
    tags: HashMap<String, String>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Repository {
    Url(String),
    Object {
        r#type: Option<String>,
        url: Option<String>,
    },
}

#[derive(Serialize, Deserialize)]
struct Bugs {
    url: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Packument {
    name: PackageIdentifier,
    readme: Option<String>,
    description: Option<String>,
    #[serde(rename = "dist-tags")]
    dist_tags: DistTags,
    versions: HashMap<String, PackumentVersion>,
    maintainers: Vec<Maintainer>,
    time: PackumentTime,
    homepage: Option<String>,
    keywords: Option<Vec<String>>,
    repository: Option<Repository>,
    author: Option<Maintainer>,
    bugs: Option<Bugs>,
    license: String,
    #[serde(rename = "readmeFilename")]
    readme_filename: Option<String>,
    attachments: HashMap<String, String>,
}
