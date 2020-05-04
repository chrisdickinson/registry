use async_trait::async_trait;
use futures::future::Either;
use http_types::Result;

use crate::stores::{PackageMetadata, ReadableStore};

#[async_trait]
impl<L: ReadableStore + Send, R: ReadableStore + Send> ReadableStore for (L, R) {
    type PackumentReader = Either<L::PackumentReader, R::PackumentReader>;
    type TarballReader = Either<L::TarballReader, R::TarballReader>;

    async fn get_packument<T>(&self, package: T) -> Result<Option<(Self::PackumentReader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        let package_str = package.as_ref();
        let first = self.0.get_packument(package_str).await?;
        if let Some((stream, meta)) = first {
            return Ok(Some((Either::Left(stream), meta)));
        }

        let second = self.1.get_packument(package_str).await?;
        if let Some((stream, meta)) = second {
            return Ok(Some((Either::Right(stream), meta)));
        }

        Ok(None)
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
        let first = self.0.get_tarball(package_str, version_str).await?;
        if let Some((stream, meta)) = first {
            return Ok(Some((Either::Left(stream), meta)));
        }

        let second = self.1.get_tarball(package_str, version_str).await?;
        if let Some((stream, meta)) = second {
            return Ok(Some((Either::Right(stream), meta)));
        }

        Ok(None)
    }
}

#[async_trait]
impl<Inner: ReadableStore + Send> ReadableStore for Vec<Inner> {
    type PackumentReader = Inner::PackumentReader;
    type TarballReader = Inner::TarballReader;

    async fn get_packument<T>(&self, package: T) -> Result<Option<(Self::PackumentReader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        for store in self {
            let package_str = package.as_ref();
            let resp = store.get_packument(package_str).await?;
            if let Some((stream, meta)) = resp {
                return Ok(Some((stream, meta)));
            }
        }

        Ok(None)
    }
}
