use axum_extra::extract::cookie::Key;

use super::Configurator;

#[derive(Debug, Clone)]
pub struct EnvConfigurator {
    fqdn: String,
}

impl EnvConfigurator {
    pub fn new() -> Self {
        let fqdn = std::env::var("REGI_FQDN")
            .ok()
            .or_else(|| {
                std::env::var("HOST")
                    .ok()
                    .zip(std::env::var("PORT").ok())
                    .map(|(host, port)| format!("http://{}:{}", host, port))
            })
            .unwrap_or_else(|| "http://localhost:8000".to_string());
        Self { fqdn }
    }
}

impl Default for EnvConfigurator {
    fn default() -> Self {
        EnvConfigurator::new()
    }
}

#[async_trait::async_trait]
impl Configurator for EnvConfigurator {
    fn fqdn(&self) -> &str {
        &self.fqdn
    }

    async fn oauth_config(&self) -> anyhow::Result<(String, String)> {
        let client_id = std::env::var("REGI_OAUTH_CLIENT_ID")?;
        let client_secret = std::env::var("REGI_OAUTH_CLIENT_SECRET")?;
        Ok((client_id, client_secret))
    }

    async fn cookie_key(&self) -> anyhow::Result<Key> {
        let secret = std::env::var("REGI_COOKIE_SECRET")?;
        Ok(Key::from(secret.as_bytes()))
    }
}
