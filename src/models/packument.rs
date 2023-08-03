use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    str::FromStr,
    string::FromUtf8Error,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PackumentError {
    #[error("Invalid package identifier: {0}")]
    InvalidPackageIdentifier(&'static str),
    #[error("Package identifier must be valid UTF-8")]
    PackageIdentifierMustBeUtf8(#[from] FromUtf8Error),
}

pub struct PackageIdentifier {
    pub scope: Option<String>,
    pub name: String,
}

impl Display for PackageIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref scope) = self.scope {
            write!(f, "@{}/{}", scope, self.name)
        } else {
            f.write_str(self.name.as_str())
        }
    }
}

impl Debug for PackageIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref scope) = self.scope {
            write!(f, "@{}/{}", scope, self.name)
        } else {
            f.write_str(self.name.as_str())
        }
    }
}

impl FromStr for PackageIdentifier {
    type Err = PackumentError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = urlencoding::decode(s).map_err(PackumentError::PackageIdentifierMustBeUtf8)?;
        let mut parts = s.split('/');
        let mut scope = None;
        let name = if let Some(part) = parts.next() {
            if let Some(s) = part.strip_prefix('@') {
                scope = Some(s.to_string());
                if let Some(part) = parts.next() {
                    part.to_string()
                } else {
                    return Err(PackumentError::InvalidPackageIdentifier(
                        "expected a name component after a scope component",
                    ));
                }
            } else {
                part.to_string()
            }
        } else {
            return Err(PackumentError::InvalidPackageIdentifier(
                "there must be some kind of package name",
            ));
        };

        if parts.next().is_some() {
            return Err(PackumentError::InvalidPackageIdentifier(
                "package identifiers must have at most 1 slash",
            ));
        }

        Ok(PackageIdentifier { scope, name })
    }
}

// Many thanks to Ryan Day for putting together type definitions [1]
// for common NPM objects.
// [1]: https://github.com/npm/types/blob/7f357f45e2b4205cd8474339a95092a5e6e77917/index.d.ts

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(untagged)]
pub enum Maintainer {
    Byline(String),
    Object {
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Signature {
    pub(crate) keyid: String,
    pub(crate) sig: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Attachment {
    pub(crate) content_type: String,
    pub(crate) data: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Dist {
    pub(crate) tarball: String,
    pub(crate) shasum: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) integrity: Option<String>,

    #[serde(rename = "fileCount", skip_serializing_if = "Option::is_none")]
    pub(crate) file_count: Option<usize>,

    #[serde(rename = "unpackedSize", skip_serializing_if = "Option::is_none")]
    pub(crate) unpacked_size: Option<usize>,

    pub(crate) signatures: Option<Vec<Signature>>,

    #[serde(rename = "npm-signature", skip_serializing_if = "Option::is_none")]
    pub(crate) npm_signature: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct PackumentVersion {
    #[serde(rename = "gitHead", skip_serializing_if = "Option::is_none")]
    pub(crate) git_head: Option<String>,

    #[serde(rename = "_id")]
    pub(crate) id: String,

    #[serde(rename = "_rev")]
    pub(crate) rev: Option<String>,

    #[serde(rename = "npmVersion", skip_serializing_if = "Option::is_none")]
    pub(crate) npm_version: Option<String>,

    #[serde(rename = "nodeVersion", skip_serializing_if = "Option::is_none")]
    pub(crate) node_version: Option<String>,

    #[serde(rename = "_npmUser", skip_serializing_if = "Option::is_none")]
    pub(crate) npm_user: Option<Maintainer>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) maintainers: Option<Vec<Maintainer>>,

    pub(crate) dist: Dist,

    #[serde(rename = "_hasShrinkwrap")]
    pub(crate) has_shrinkwrap: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) types: Option<String>,

    #[serde(flatten)]
    pub(crate) meta: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PackumentTime {
    pub(crate) created: DateTime<Utc>,
    pub(crate) modified: DateTime<Utc>,
    #[serde(flatten)]
    pub(crate) versions: HashMap<String, DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct DistTags {
    pub(crate) latest: Option<String>,
    #[serde(flatten)]
    pub(crate) tags: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum Repository {
    Url(String),
    Object {
        r#type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum License {
    Raw(String),
    Object {
        r#type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Bugs {
    pub(crate) url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Packument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,

    #[serde(rename = "_rev", skip_serializing_if = "Option::is_none")]
    pub(crate) rev: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) readme: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "dist-tags")]
    pub(crate) dist_tags: Option<DistTags>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) versions: Option<HashMap<String, PackumentVersion>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) maintainers: Option<Vec<Maintainer>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) time: Option<PackumentTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) homepage: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) keywords: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) repository: Option<Repository>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) author: Option<Maintainer>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bugs: Option<Bugs>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) license: Option<License>,

    #[serde(rename = "users", skip_serializing_if = "Option::is_none")]
    pub(crate) stargazers: Option<HashMap<String, bool>>,

    #[serde(rename = "readmeFilename", skip_serializing_if = "Option::is_none")]
    pub(crate) readme_filename: Option<String>,

    #[serde(rename = "_attachments", skip_serializing_if = "Option::is_none")]
    pub(crate) attachments: Option<HashMap<String, Attachment>>,
}
