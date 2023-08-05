use std::{collections::HashSet, fmt::Display, hash::Hash, str::FromStr};

use axum::{
    body::{Body, Bytes},
    http::{request::Parts, Request},
    response::IntoResponse,
};
use axum_extra::extract::cookie::Key;
use futures::stream::BoxStream;
use libflate::gzip::Decoder;
use std::io::Cursor;
use tar::Archive;

use crate::models::{PackageIdentifier, Packument, PackumentVersion, User};

// Chosen at random.
const MAX_FILE_COUNT: usize = 16000;

// 1 GiB, chosen at random.
const MAX_UNPACKED_SIZE: usize = 1 << 30;

#[async_trait::async_trait]
pub trait PackageStorage: Send + Sync {
    type Error: Into<axum::BoxError> + Send + Sync + 'static;
    async fn fetch_packument(&self, name: &PackageIdentifier) -> anyhow::Result<Packument> {
        let stream = self.stream_packument(name).await?;
        use futures::TryStreamExt;

        let data: Vec<Bytes> = stream.try_collect().await.map_err(|e| {
            let box_error: axum::BoxError = e.into();
            anyhow::anyhow!(box_error)
        })?;
        let data = data.as_slice().concat();

        Ok(serde_json::from_slice(data.as_slice())?)
    }

    async fn stream_packument(
        &self,
        name: &PackageIdentifier,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>>;

    async fn stream_tarball(
        &self,
        name: &PackageIdentifier,
        version: &str,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>>;
}

#[async_trait::async_trait]
pub trait TokenAuthorizer {
    type TokenSessionId: Hash + FromStr + Display + Clone + Send + Sync;

    async fn authenticate_session_bearer(
        &self,
        _bearer: Self::TokenSessionId,
    ) -> anyhow::Result<Option<User>> {
        Ok(None)
    }

    async fn start_session(&self, user: User) -> anyhow::Result<Self::TokenSessionId>;

    async fn authenticate_session(&self, req: &Parts) -> anyhow::Result<Option<User>> {
        let Some(authentication) = req.headers.get("authorization") else {
            return Ok(None);
        };

        let Ok(authentication) = authentication.to_str() else {
            return Ok(None);
        };

        let Some(authentication) = authentication.strip_prefix("Bearer ").or_else(|| authentication.strip_prefix("bearer ")) else {
            return Ok(None);
        };

        let Ok(token): Result<Self::TokenSessionId, _> = authentication.trim().parse() else {
            return Ok(None);
        };

        self.authenticate_session_bearer(token).await
    }
}

#[async_trait::async_trait]
pub trait Authenticator: Send + Sync {
    type LoginSessionId: Hash + FromStr + Display + Clone + Send + Sync;
    type LoginWWWResponse: IntoResponse + Send + Sync;

    async fn start_login_session(&self, req: Request<Body>)
        -> anyhow::Result<Self::LoginSessionId>;
    async fn poll_login_session(
        &self,
        session: Self::LoginSessionId,
    ) -> anyhow::Result<Option<User>>;
    async fn complete_login_session<C: Configurator + Send + Sync>(
        &self,
        config: &C,
        req: Request<Body>,
        session: Option<Self::LoginSessionId>,
    ) -> anyhow::Result<Self::LoginWWWResponse>;

    async fn get_user(&self, _username: &str) -> anyhow::Result<Option<User>> {
        Ok(None)
    }
}

#[async_trait::async_trait]
pub trait Configurator {
    fn fqdn(&self) -> &str;

    async fn oauth_config(&self) -> anyhow::Result<(String, String)>;
    async fn cookie_key(&self) -> anyhow::Result<Key>;
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
