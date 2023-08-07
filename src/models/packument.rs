use std::{
    collections::{HashMap, HashSet},
    fmt::{Debug, Display},
    str::FromStr,
    string::FromUtf8Error,
};

use libflate::gzip::Decoder;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use tar::Archive;

use chrono::{DateTime, Utc};
use thiserror::Error;

// Chosen at random.
const MAX_FILE_COUNT: usize = 16000;

// 1 GiB, chosen at random.
const MAX_UNPACKED_SIZE: usize = 1 << 30;

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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct MaintainerObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(untagged)]
pub enum Maintainer {
    Byline(String),
    Object(MaintainerObject),
}

use {once_cell::sync::Lazy, regex::Regex};

impl Maintainer {
    pub fn into_object(self) -> MaintainerObject {
        static RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"(?<name>[^<(]+)\s*(<(?<email>[^>]+)>)?\s*(\((?<url>[^)]+)\))?").unwrap()
        });

        match self {
            Maintainer::Object(o) => o,
            Maintainer::Byline(s) => {
                let Some(caps) = RE.captures(s.as_str()) else {
                    return Default::default();
                };

                let name = caps.name("name").map(|xs| xs.as_str().trim().to_string());
                let email = caps.name("email").map(|xs| xs.as_str().trim().to_string());
                let url = caps.name("url").map(|xs| xs.as_str().trim().to_string());

                MaintainerObject { name, email, url }
            }
        }
    }
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

#[derive(Clone, Debug)]
pub enum PackageModification {
    AddStar(String),
    RemoveStar(String),

    AddTag {
        tag: String,
        version: String,
    },

    RemoveTag {
        tag: String,
    },

    AddMaintainer(String),
    RemoveMaintainer(String),

    AddVersion {
        tag: String,
        version: Box<PackumentVersion>,
        tarball: Option<Vec<u8>>,
    },
}

impl PackageModification {
    pub(crate) fn from_diff(old: Packument, new: Packument) -> anyhow::Result<Self> {
        if let Some((old_stargazers, new_stargazers)) = old.stargazers.zip(new.stargazers) {
            let old_stargazers: HashSet<_> = old_stargazers.keys().map(String::as_str).collect();
            let new_stargazers: HashSet<_> = new_stargazers.keys().map(String::as_str).collect();

            if old_stargazers != new_stargazers {
                let mut removed: Vec<&str> = old_stargazers
                    .difference(&new_stargazers)
                    .copied()
                    .collect();

                if !removed.is_empty() {
                    if removed.len() > 1 {
                        anyhow::bail!("Can only remove a single stargazer at a time")
                    }

                    return Ok(Self::RemoveStar(removed.pop().unwrap().to_string()));
                }

                let mut added: Vec<&str> = new_stargazers
                    .difference(&old_stargazers)
                    .copied()
                    .collect();

                if !added.is_empty() {
                    if added.len() > 1 {
                        anyhow::bail!("Can only add a single stargazer at a time")
                    }

                    return Ok(Self::AddStar(added.pop().unwrap().to_string()));
                }
            }
        }

        if let Some(((dist_tags, versions), attachments)) =
            new.dist_tags.zip(new.versions).zip(new.attachments)
        {
            if (dist_tags.tags.len() == 1 && dist_tags.latest.is_none())
                || (dist_tags.latest.is_some() && dist_tags.tags.is_empty())
            {
                let Some(version_name) = dist_tags.latest.as_ref().or(dist_tags.tags.values().next()) else {
                    anyhow::bail!("Could not find tag for publish")
                };

                // TODO: validate the tag name!
                let Some(tag_name) = dist_tags.latest.as_ref()
                    .map(|_| "latest".to_string())
                    .or(dist_tags.tags.keys().next().cloned()) else {
                    anyhow::bail!("Could not find new tag name")
                };

                let Some(version) = versions.get(version_name) else {
                    anyhow::bail!("Attempted tag publish failed: did not refer to new version")
                };

                let Some(pkg_name) = new.name.or(new.id) else {
                    anyhow::bail!("Package name not present")
                };

                let pkg_name: PackageIdentifier = pkg_name.parse()?;

                let attachment_name = format!("{}-{}.tgz", pkg_name.name, version_name);
                let Some(attachment) = attachments.get(attachment_name.as_str()) else {
                    anyhow::bail!("Expected attachment not found")
                };

                if attachment.content_type != "application/octet-stream" {
                    anyhow::bail!(
                        "Expected attachment to have application/octet-stream content-type"
                    )
                };

                // TODO: check times on old packument, make sure we aren't overwriting an old,
                // deleted packument version

                let mut r = Cursor::new(attachment.data.as_bytes());
                let mut decoded = base64::read::DecoderReader::new(
                    &mut r,
                    &base64::engine::general_purpose::STANDARD,
                );

                let mut debase64d: Vec<u8> = Vec::with_capacity(attachment.data.as_bytes().len());

                let mut decoded = io_tee::TeeReader::new(&mut decoded, &mut debase64d);

                let mut gunzipped = Decoder::new(&mut decoded)?;
                let mut tarball = Archive::new(&mut gunzipped);

                let mut unpacked_size = 0usize;
                let mut file_count = 0usize;
                let mut saw_package_json = false;
                for entry in tarball.entries()? {
                    let Ok(entry) = entry else {
                        anyhow::bail!("Encountered bad tarball entry")
                    };

                    unpacked_size += entry.size() as usize;
                    file_count += 1;

                    if file_count > MAX_FILE_COUNT {
                        anyhow::bail!("Tarball exceeded maximum file count")
                    }

                    if unpacked_size > MAX_UNPACKED_SIZE {
                        anyhow::bail!("Tarball exceeded maximum unpacked size")
                    }

                    let Ok(path) = entry.path() else {
                        anyhow::bail!("Malformed unicode path")
                    };

                    let Ok(path) = path.strip_prefix("package/") else {
                        anyhow::bail!("Tarball entry didn't start with 'package/'")
                    };

                    saw_package_json =
                        saw_package_json || path.display().to_string() == "package.json";
                }

                if !saw_package_json {
                    anyhow::bail!("Tarball did not contain package.json")
                }

                return Ok(PackageModification::AddVersion {
                    tag: tag_name,
                    version: Box::new(version.clone()),
                    tarball: Some(debase64d),
                });
            }
        }

        if let Some((old_maintainers, new_maintainers)) = old.maintainers.zip(new.maintainers) {
            let old_maintainers: HashSet<_> = old_maintainers
                .iter()
                .filter_map(|maint| maint.clone().into_object().name)
                .collect();
            let new_maintainers: HashSet<_> = new_maintainers
                .iter()
                .filter_map(|maint| maint.clone().into_object().name)
                .collect();
            if old_maintainers != new_maintainers {
                let mut removed: Vec<_> = old_maintainers.difference(&new_maintainers).collect();

                if !removed.is_empty() {
                    if removed.len() > 1 {
                        anyhow::bail!("Can only remove a single maintainer at a time")
                    }

                    return Ok(Self::RemoveMaintainer(removed.pop().unwrap().to_string()));
                }

                let mut added: Vec<_> = new_maintainers.difference(&old_maintainers).collect();

                if !added.is_empty() {
                    if added.len() > 1 {
                        anyhow::bail!("Can only add a single maintainer at a time")
                    }

                    return Ok(Self::AddMaintainer(added.pop().unwrap().to_string()));
                }
            }
        }

        anyhow::bail!("wip")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maintainer_to_object() {
        let m = Maintainer::Byline(
            "homer j. simpson <homer@simpso.ns> (https://example.com)".to_string(),
        );
        assert_eq!(
            m.into_object(),
            MaintainerObject {
                name: Some("homer j. simpson".to_string()),
                email: Some("homer@simpso.ns".to_string()),
                url: Some("https://example.com".to_string()),
            }
        );

        let m = Maintainer::Byline("homer j. simpson (https://example.com)".to_string());
        assert_eq!(
            m.into_object(),
            MaintainerObject {
                name: Some("homer j. simpson".to_string()),
                email: None,
                url: Some("https://example.com".to_string()),
            }
        );

        let m = Maintainer::Byline("homer j. simpson <homer@simpso.ns>".to_string());
        assert_eq!(
            m.into_object(),
            MaintainerObject {
                name: Some("homer j. simpson".to_string()),
                email: Some("homer@simpso.ns".to_string()),
                url: None,
            }
        );

        let m = Maintainer::Byline("gary".to_string());
        assert_eq!(
            m.into_object(),
            MaintainerObject {
                name: Some("gary".to_string()),
                email: None,
                url: None,
            }
        );
    }
}
