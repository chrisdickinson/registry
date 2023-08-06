use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::models::{PackageIdentifier, User};
use crate::operations::{Authenticator, Configurator, PackageStorage, TokenAuthorizer};
use axum::body::Body;
use axum::http::{HeaderMap, StatusCode};
use axum::{
    body::Bytes,
    http::{request::Parts, Request},
};
use axum::{Json, RequestExt};
use axum_extra::extract::cookie::{Cookie, Key};
use axum_extra::extract::SignedCookieJar;
use chrono::{DateTime, Utc};
use futures::stream::BoxStream;
use futures_util::{pin_mut, StreamExt};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use serde::Deserialize;
use tokio::sync::RwLock;
use url::Url;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default)]
pub struct NotImplemented;

impl crate::operations::NotImplemented for NotImplemented {}

#[derive(Clone, Copy, Debug)]
pub struct Policy<
    AuthImpl = NotImplemented,
    TokenAuthzImpl = NotImplemented,
    PackageStorageImpl = NotImplemented,
    ConfiguratorImpl = EnvConfigurator,
> where
    AuthImpl: Authenticator + Send + Sync,
    TokenAuthzImpl: TokenAuthorizer + Send + Sync,
    PackageStorageImpl: PackageStorage + Send + Sync,
    ConfiguratorImpl: Configurator + Send + Sync,
{
    auth: AuthImpl,
    token_authz: TokenAuthzImpl,
    package_storage: PackageStorageImpl,
    configurator: ConfiguratorImpl,
}

impl Policy {
    pub fn new() -> Self {
        Self {
            package_storage: NotImplemented,
            auth: NotImplemented,
            token_authz: NotImplemented,
            configurator: EnvConfigurator::new(),
        }
    }
}

impl Default for Policy {
    fn default() -> Self {
        Policy::new()
    }
}

impl<A, T, S, C> Policy<A, T, S, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    S: PackageStorage + Send + Sync,
    C: Configurator + Send + Sync,
{
    pub fn with_authenticator<A1: Authenticator + Send + Sync>(
        self,
        auth: A1,
    ) -> Policy<A1, T, S, C> {
        Policy {
            auth,
            token_authz: self.token_authz,
            package_storage: self.package_storage,
            configurator: self.configurator,
        }
    }

    pub fn with_package_storage<S1: PackageStorage + Send + Sync>(
        self,
        package_storage: S1,
    ) -> Policy<A, T, S1, C> {
        Policy {
            auth: self.auth,
            token_authz: self.token_authz,
            configurator: self.configurator,
            package_storage,
        }
    }

    pub fn with_token_authorizer<T1: TokenAuthorizer + Send + Sync>(
        self,
        token_authz: T1,
    ) -> Policy<A, T1, S, C> {
        Policy {
            auth: self.auth,
            token_authz,
            configurator: self.configurator,
            package_storage: self.package_storage,
        }
    }
}

#[async_trait::async_trait]
impl<AuthenticatorImpl, TokenAuthorizerImpl, PackageStorageImpl, ConfiguratorImpl> Authenticator
    for Policy<AuthenticatorImpl, TokenAuthorizerImpl, PackageStorageImpl, ConfiguratorImpl>
where
    AuthenticatorImpl: Authenticator + Send + Sync,
    TokenAuthorizerImpl: TokenAuthorizer + Send + Sync,
    PackageStorageImpl: PackageStorage + Send + Sync,
    ConfiguratorImpl: Configurator + Send + Sync,
{
    type LoginSessionId = AuthenticatorImpl::LoginSessionId;
    type LoginWWWResponse = AuthenticatorImpl::LoginWWWResponse;

    async fn start_login_session(
        &self,
        req: Request<Body>,
    ) -> anyhow::Result<Self::LoginSessionId> {
        self.auth.start_login_session(req).await
    }

    async fn poll_login_session(&self, id: Self::LoginSessionId) -> anyhow::Result<Option<User>> {
        self.auth.poll_login_session(id).await
    }

    async fn complete_login_session<C: Configurator + Send + Sync>(
        &self,
        _config: &C,
        req: Request<Body>,
        id: Option<Self::LoginSessionId>,
    ) -> anyhow::Result<Self::LoginWWWResponse> {
        self.auth.complete_login_session(self, req, id).await
    }
}

#[async_trait::async_trait]
impl<A, T, S, C> TokenAuthorizer for Policy<A, T, S, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    S: PackageStorage + Send + Sync,
    C: Configurator + Send + Sync,
{
    type TokenSessionId = T::TokenSessionId;

    async fn start_session(&self, user: User) -> anyhow::Result<Self::TokenSessionId> {
        self.token_authz.start_session(user).await
    }

    async fn authenticate_session_bearer(
        &self,
        req: Self::TokenSessionId,
    ) -> anyhow::Result<Option<User>> {
        self.token_authz.authenticate_session_bearer(req).await
    }

    async fn authenticate_session(&self, req: &Parts) -> anyhow::Result<Option<User>> {
        self.token_authz.authenticate_session(req).await
    }
}

#[async_trait::async_trait]
impl<A, T, S, C> PackageStorage for Policy<A, T, S, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    S: PackageStorage + Send + Sync,
    C: Configurator + Send + Sync,
{
    type Error = S::Error;
    async fn stream_packument(
        &self,
        name: &PackageIdentifier,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        self.package_storage.stream_packument(name).await
    }

    async fn stream_tarball(
        &self,
        name: &PackageIdentifier,
        version: &str,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        self.package_storage.stream_tarball(name, version).await
    }
}

#[async_trait::async_trait]
impl<A, T, S, C> Configurator for Policy<A, T, S, C>
where
    A: Authenticator + Send + Sync,
    T: TokenAuthorizer + Send + Sync,
    S: PackageStorage + Send + Sync,
    C: Configurator + Send + Sync,
{
    fn fqdn(&self) -> &str {
        self.configurator.fqdn()
    }

    async fn oauth_config(&self) -> anyhow::Result<(String, String)> {
        self.configurator.oauth_config().await
    }

    async fn cookie_key(&self) -> anyhow::Result<Key> {
        self.configurator.cookie_key().await
    }
}

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

#[derive(Clone, Debug)]
pub struct RemoteRegistry {
    registry: String,
}

impl Default for RemoteRegistry {
    fn default() -> Self {
        Self {
            registry: "https://registry.npmjs.org".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl PackageStorage for RemoteRegistry {
    type Error = reqwest::Error;
    async fn stream_packument(
        &self,
        name: &PackageIdentifier,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        Ok(reqwest::get(format!("{}/{}", self.registry, name))
            .await?
            .bytes_stream()
            .boxed())
    }

    async fn stream_tarball(
        &self,
        pkg: &PackageIdentifier,
        version: &str,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        let url = if let Some(ref scope) = pkg.scope {
            format!(
                "{}/@{}/{}/-/{}-{}.tgz",
                self.registry, scope, pkg.name, pkg.name, version
            )
        } else {
            format!(
                "{}/{}/-/{}-{}.tgz",
                self.registry, pkg.name, pkg.name, version
            )
        };

        Ok(reqwest::get(url).await?.bytes_stream().boxed())
    }
}

#[derive(Clone, Debug)]
pub struct ReadThrough<R: PackageStorage + Clone + std::fmt::Debug + Send + Sync + 'static> {
    cache_dir: PathBuf,
    inner: R,
}

impl<R: PackageStorage + Clone + std::fmt::Debug + Send + Sync + 'static> ReadThrough<R> {
    pub fn new(cache_dir: impl AsRef<Path>, inner: R) -> Self {
        Self {
            cache_dir: PathBuf::from(cache_dir.as_ref()),
            inner,
        }
    }
}

#[async_trait::async_trait]
impl<R> PackageStorage for ReadThrough<R>
where
    R: PackageStorage + Clone + std::fmt::Debug + Send + Sync + 'static,
    <R as PackageStorage>::Error: std::error::Error + Send + Sync + 'static,
{
    type Error = std::io::Error;
    async fn stream_packument(
        &self,
        name: &PackageIdentifier,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        let key = format!("packument:{}", name);
        match cacache::Reader::open(&self.cache_dir, &key).await {
            Ok(reader) => Ok(tokio_util::io::ReaderStream::new(reader).boxed()),

            Err(cacache::Error::EntryNotFound(_, _)) => {
                use tokio::io::AsyncWriteExt;
                let stream = self.inner.stream_packument(name).await?;
                let mut writer =
                    cacache::Writer::create(self.cache_dir.as_path(), key.as_str()).await?;
                pin_mut!(stream);
                while let Some(chunk) = stream.next().await {
                    let Ok(chunk) = chunk else {
                        break;
                    };
                    writer.write_all(chunk.as_ref()).await?;
                }
                writer.commit().await?;

                return self.stream_packument(name).await;
            }
            Err(e) => return Err(e.into()),
        }
    }

    async fn stream_tarball(
        &self,
        name: &PackageIdentifier,
        version: &str,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        let key = format!("tarball:{}:{}", name, version);
        match cacache::Reader::open(&self.cache_dir, &key).await {
            Ok(reader) => Ok(tokio_util::io::ReaderStream::new(reader).boxed()),

            Err(cacache::Error::EntryNotFound(_, _)) => {
                use tokio::io::AsyncWriteExt;
                let stream = self.inner.stream_tarball(name, version).await?;
                let mut writer =
                    cacache::Writer::create(self.cache_dir.as_path(), key.as_str()).await?;
                pin_mut!(stream);
                while let Some(chunk) = stream.next().await {
                    let Ok(chunk) = chunk else {
                        break;
                    };
                    writer.write_all(chunk.as_ref()).await?;
                }
                writer.commit().await?;

                return self.stream_tarball(name, version).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

#[derive(Clone, Debug)]
struct LoginSession {
    initialized_at: DateTime<Utc>,
    user: Option<User>,
    hostname: Option<String>,
    csrftoken: Option<String>,
}

#[derive(Clone)]
pub struct OAuthAuthenticator {
    login_sessions: Arc<RwLock<HashMap<Uuid, LoginSession>>>,
    auth_url: AuthUrl,
    token_url: TokenUrl,
    scopes: Vec<Scope>,
}

impl std::fmt::Debug for OAuthAuthenticator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut formatter = f.debug_struct("OAuthAuthenticator");
        if let Ok(sessions) = self.login_sessions.try_read() {
            formatter.field("login_sessions", &sessions);
        }
        formatter.finish()
    }
}

impl OAuthAuthenticator {
    pub fn new(auth_url: &str, token_url: &str, scopes: Vec<String>) -> Self {
        Self {
            login_sessions: Arc::new(RwLock::new(HashMap::new())),
            auth_url: AuthUrl::new(auth_url.to_string()).expect("auth_url was invalid"),
            token_url: TokenUrl::new(token_url.to_string()).expect("token_url was invalid"),
            scopes: scopes.into_iter().map(Scope::new).collect(),
        }
    }

    pub fn for_github() -> Self {
        Self::new(
            "https://github.com/login/oauth/authorize",
            "https://github.com/login/oauth/access_token",
            vec!["read:org".to_string(), "read:user".to_string()],
        )
    }

    fn get_oauth_authorize_url(
        &self,
        fqdn: &Url,
        client_id: &str,
        client_secret: &str,
    ) -> (Url, CsrfToken) {
        let client = self.get_oauth_client(fqdn, client_id, client_secret);
        let mut authorize_url = client.authorize_url(CsrfToken::new_random);
        for scope in self.scopes.clone() {
            authorize_url = authorize_url.add_scope(scope);
        }

        authorize_url.url()
    }

    fn get_oauth_client(&self, fqdn: &Url, client_id: &str, client_secret: &str) -> BasicClient {
        let mut redirect_url = fqdn.clone();
        redirect_url.set_path("/-/v1/login/www/");

        // TODO: TKTK: we shouldn't recreate the client each time, but to reuse the client
        // we've got to have access to the client secret at init time. HOWEVER I'd like to
        // route all of that through Configurator, with the thought that some clients may
        // wish to use AWS Secrets or Vault or another, async method of getting the keys
        // to this service.
        oauth2::basic::BasicClient::new(
            ClientId::new(client_id.to_string()),
            Some(ClientSecret::new(client_secret.to_string())),
            self.auth_url.clone(),
            Some(self.token_url.clone()),
        )
        .set_redirect_uri(RedirectUrl::from_url(redirect_url))
    }
}

#[async_trait::async_trait]
impl Authenticator for OAuthAuthenticator {
    type LoginSessionId = Uuid;
    type LoginWWWResponse = (StatusCode, SignedCookieJar, HeaderMap, String);

    async fn start_login_session(
        &self,
        req: Request<Body>,
    ) -> anyhow::Result<Self::LoginSessionId> {
        let id = Uuid::new_v4();

        let body = req
            .extract::<Json<serde_json::Value>, _>()
            .await
            .ok()
            .map(|Json(body)| body);

        let hostname = body.and_then(|body| {
            body.get("hostname")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });

        self.login_sessions.write().await.insert(
            id,
            LoginSession {
                initialized_at: Utc::now(),
                csrftoken: None,
                user: None,
                hostname,
            },
        );

        Ok(id)
    }

    async fn poll_login_session(
        &self,
        bearer: Self::LoginSessionId,
    ) -> anyhow::Result<Option<User>> {
        let has_user = {
            let sessions = self.login_sessions.read().await;

            let Some(session) = sessions.get(&bearer) else {
                return Err(anyhow::anyhow!("unrecognized login session"))
            };

            session.user.is_some()
        };

        if !has_user {
            return Ok(None);
        }

        let session = self.login_sessions.write().await.remove(&bearer);
        Ok(session.and_then(|sess| sess.user))
    }

    // TODO: oh my god this is such slop. It really needs to be revisited when:
    // - we add more oauth providers (google, auth0, okta; oidc in general)
    // - we start to tighten our error handling
    async fn complete_login_session<C: Configurator + Send + Sync>(
        &self,
        config: &C,
        req: Request<Body>,
        bearer: Option<Self::LoginSessionId>,
    ) -> anyhow::Result<Self::LoginWWWResponse> {
        let fqdn = Url::parse(config.fqdn()).unwrap();
        let key = config.cookie_key().await?;
        let mut jar = SignedCookieJar::from_headers(req.headers(), key);

        if let Some(bearer) = bearer {
            if let Some(session) = self.login_sessions.write().await.get_mut(&bearer) {
                let (client_id, client_secret) = config.oauth_config().await?;
                let (auth_url, csrftoken) =
                    self.get_oauth_authorize_url(&fqdn, client_id.as_str(), client_secret.as_str());

                let mut headers = HeaderMap::new();
                headers.insert(
                    axum::http::header::LOCATION,
                    auth_url.to_string().try_into().unwrap(),
                );
                session.csrftoken = Some(csrftoken.secret().clone());
                jar = jar.add(
                    Cookie::build("sid", bearer.to_string())
                        .domain(fqdn.host().unwrap().to_string())
                        .secure(fqdn.scheme() == "https")
                        .http_only(true)
                        .finish(),
                );

                Ok((StatusCode::TEMPORARY_REDIRECT, jar, headers, String::new()))
            } else {
                Err(anyhow::anyhow!("unrecognized login session"))
            }
        } else {
            #[derive(Deserialize)]
            pub struct ReceivedCode {
                pub code: AuthorizationCode,
                pub state: CsrfToken,
            }

            if let Ok(received) =
                serde_urlencoded::from_str::<ReceivedCode>(req.uri().query().unwrap_or(""))
            {
                let Some(cookie) = jar.get("sid") else {
                    anyhow::bail!("expected session id cookie");
                };
                let Some(bearer) = cookie.value().parse().ok() else {
                    anyhow::bail!("stored invalid login session id");
                };
                let mut sessions = self.login_sessions.write().await;
                let Some(session) = sessions.get_mut(&bearer) else {
                    anyhow::bail!("unrecognized login session");
                };

                if Some(received.state.secret()) != session.csrftoken.as_ref() {
                    anyhow::bail!("csrf token mismatch");
                }

                let (client_id, client_secret) = config.oauth_config().await?;
                let client =
                    self.get_oauth_client(&fqdn, client_id.as_str(), client_secret.as_str());
                let token = client
                    .exchange_code(received.code)
                    .request_async(async_http_client)
                    .await?;

                let client = reqwest::Client::new();
                let auth_header = format!("Bearer {}", token.access_token().secret());

                // We pronounce "GitHub" as "j'thoob" here.
                #[derive(Deserialize)]
                struct GitHubUser {
                    login: String,
                    email: String,
                    name: Option<String>,
                }

                let userdata = client
                    .get("https://api.github.com/user")
                    .header("Authorization", auth_header)
                    .header("Content-Type", "application/vnd.github+json")
                    .header(
                        "User-Agent",
                        "regi/v1.0.0 (https://github.com/chrisdickinson/registry)",
                    )
                    .send()
                    .await?
                    .json::<GitHubUser>()
                    .await?;

                session.user = Some(User {
                    name: userdata.login,
                    email: userdata.email,
                    full_name: userdata.name,
                });
            };

            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::CONTENT_TYPE,
                "text/html; charset=utf-8".try_into().unwrap(),
            );

            Ok((
                StatusCode::OK,
                jar,
                headers,
                r#"
                <!doctype html>
                <html>
                <head></head>
                <body>
                    <h1>
                        ♕
                    </h1>
                </body>
                </html>
            "#
                .to_string(),
            ))
        }
    }
}

#[derive(Clone, Debug)]
struct TokenSession {
    initialized_at: DateTime<Utc>,
    user: User,
}

#[derive(Clone)]
pub struct InMemoryTokenAuthorizer {
    token_sessions: Arc<RwLock<HashMap<Uuid, TokenSession>>>,
}

impl std::fmt::Debug for InMemoryTokenAuthorizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut formatter = f.debug_struct("OAuthAuthenticator");
        if let Ok(sessions) = self.token_sessions.try_read() {
            formatter.field("token_sessions", &sessions);
        }
        formatter.finish()
    }
}

impl InMemoryTokenAuthorizer {
    pub fn new() -> Self {
        Self {
            token_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryTokenAuthorizer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TokenAuthorizer for InMemoryTokenAuthorizer {
    type TokenSessionId = Uuid;
    async fn start_session(&self, user: User) -> anyhow::Result<Self::TokenSessionId> {
        let key = Uuid::new_v4();
        self.token_sessions.write().await.insert(
            key,
            TokenSession {
                initialized_at: Utc::now(),
                user,
            },
        );

        Ok(key)
    }

    async fn authenticate_session_bearer(
        &self,
        token: Self::TokenSessionId,
    ) -> anyhow::Result<Option<User>> {
        let sessions = self.token_sessions.read().await;
        let session = sessions.get(&token);

        Ok(session.map(|sess| sess.user.clone()))
    }
}
