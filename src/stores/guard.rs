use async_trait::async_trait;
use http_types::Result;

use crate::stores::{PackageMetadata, ReadableStore};

pub struct GuardStore<F: Fn(&str) -> bool, R: ReadableStore> {
    inner: R,
    test: F
}

impl<F: Fn(&str) -> bool, R: ReadableStore> GuardStore<F, R> {
    pub fn new(inner: R, test: F) -> Self {
        GuardStore {
            inner,
            test
        }
    }
}

#[async_trait]
impl<F: Fn(&str) -> bool + Send + Sync + 'static, R: ReadableStore + Send> ReadableStore for GuardStore<F, R> {
    type PackumentReader = R::PackumentReader;
    type TarballReader = R::TarballReader;

    async fn get_packument<T>(&self, package: T) -> Result<Option<(Self::PackumentReader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        let package_str = package.as_ref();
        if (self.test)(package_str) {
            return self.inner.get_packument(package_str).await;
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
        if (self.test)(package_str) {
            return self.inner.get_tarball(package_str, version_str).await;
        }

        Ok(None)
    }
}
