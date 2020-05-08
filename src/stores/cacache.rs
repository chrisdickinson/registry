use crate::stores::{PackageMetadata, ReadableStore, WritableStore};
use futures::io::AsyncBufRead;
use async_trait::async_trait;
use std::marker::Unpin;
use http_types::Result;
use chrono::{ offset::TimeZone, Utc };
use tracing::info;

pub struct CacacheStore {
    cache: String
}

impl CacacheStore {
    pub fn new<T: AsRef<str>>(dir: T) -> Self {
        CacacheStore {
            cache: dir.as_ref().to_string()
        }
    }
}

#[async_trait]
impl WritableStore for CacacheStore {
    async fn write_packument<T, W>(&self, package: T, packument_reader: W, meta: PackageMetadata) -> Result<Option<bool>>
        where T: AsRef<str> + Send + Sync,
              W: AsyncBufRead + Send + Sync + Unpin + 'static {

        let package_str = package.as_ref();
        let key = format!("p/{}", package_str);

        let mut fd = cacache::Writer::create(&self.cache, &key).await?;
        futures::io::copy(packument_reader, &mut fd).await?;
        fd.commit().await?;
        Ok(Some(true))
    }


    async fn write_tarball<T, S, W>(
        &self,
        package: T,
        version: S,
        tarball_reader: W,
        meta: PackageMetadata
    ) -> Result<Option<bool>>
    where
        T: AsRef<str> + Send + Sync,
        S: AsRef<str> + Send + Sync,
        W: AsyncBufRead + Send + Sync + Unpin + 'static
    {
        let package_str = package.as_ref();
        let version_str = version.as_ref();
        let key = format!("t/{}/{}", package_str, version_str);
        let mut fd = cacache::Writer::create(&self.cache, &key).await?;
        futures::io::copy(tarball_reader, &mut fd).await?;
        fd.commit().await?;
        Ok(Some(true))
    }
}

#[async_trait]
impl ReadableStore for CacacheStore {
    type PackumentReader = futures::io::Cursor<Vec<u8>>;
    type TarballReader = futures::io::Cursor<Vec<u8>>;

    async fn get_packument<T>(&self, package: T) -> Result<Option<(Self::PackumentReader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        let package_str = package.as_ref();
        let key = format!("p/{}", package_str);
        let meta = match cacache::metadata(&self.cache, &key).await? {
            Some(data) => data,
            None => return Ok(None)
        };

        let data = cacache::read(&self.cache, &key).await?;
        Ok(Some((futures::io::Cursor::new(data), meta.into())))
    }

    async fn get_tarball<T, S>(
        &self,
        package: T,
        version: S,
    ) -> Result<Option<(Self::TarballReader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
        S: AsRef<str> + Send + Sync,
    {
        let package_str = package.as_ref();
        let version_str = version.as_ref();
        let key = format!("t/{}/{}", package_str, version_str);
        let meta = match cacache::metadata(&self.cache, &key).await? {
            Some(data) => data,
            None => return Ok(None)
        };

        let data = cacache::read(&self.cache, &key).await?;
        Ok(Some((futures::io::Cursor::new(data), meta.into())))
    }
}

impl From<cacache::Metadata> for PackageMetadata {
    fn from(meta: cacache::Metadata) -> PackageMetadata {
        let seconds = dbg!(meta.time / 1000);
        let nanoseconds = dbg!((meta.time - seconds) * 1000000);

        PackageMetadata {
            integrity: meta.integrity.to_string(),
            last_fetched_at: Utc.timestamp(seconds as i64, 0)
        }
    }
}
