use axum_extra::extract::cookie::Key;

pub(crate) mod env;

#[async_trait::async_trait]
pub trait Configurator {
    fn fqdn(&self) -> &str;

    async fn oauth_config(&self) -> anyhow::Result<(String, String)>;
    async fn cookie_key(&self) -> anyhow::Result<Key>;
}
