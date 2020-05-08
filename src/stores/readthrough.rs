use crate::stores::{ PackageMetadata, ReadableStore, WritableStore };
use async_trait::async_trait;
use http_types::Result;
use tracing::info;

pub struct ReadThrough<L: ReadableStore + WritableStore + Send + Sync, R: ReadableStore + Send + Sync> {
    cache: L,
    inner: R
}

impl<L: ReadableStore + WritableStore + Send + Sync, R: ReadableStore + Send + Sync> ReadThrough<L, R> {
    pub fn new(cache: L, inner: R) -> Self {
        ReadThrough {
            cache,
            inner
        }
    }
}

#[async_trait]
impl<L: ReadableStore + WritableStore + Send + Sync, R: ReadableStore + Send + Sync> ReadableStore for ReadThrough<L, R> {
    type PackumentReader = L::PackumentReader;
    type TarballReader = L::TarballReader;

    async fn get_packument<T>(&self, package: T) -> Result<Option<(Self::PackumentReader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        let package_str = package.as_ref();
        let cache_result = self.cache.get_packument(package_str).await?;
        if let Some((reader, meta)) = cache_result {
            info!("read from cache");
            return Ok(Some((reader, meta)))
        }

        let inner_result = self.inner.get_packument(package_str).await?;
        if let Some((reader, meta)) = inner_result {
            info!("writing from inner to cache");
            self.cache.write_packument(package_str, reader, meta).await?;
        }

        self.get_packument(package_str).await
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
        let cache_result = self.cache.get_tarball(package_str, version_str).await?;
        if let Some((reader, meta)) = cache_result {
            return Ok(Some((reader, meta)))
        }

        let inner_result = self.inner.get_tarball(package_str, version_str).await?;
        if let Some((reader, meta)) = inner_result {
            self.cache.write_tarball(package_str, version_str, reader, meta).await?;
        }

        self.get_tarball(package_str, version_str).await
    }
}
