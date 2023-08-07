use std::collections::HashMap;
use std::sync::Arc;

use crate::models::User;
use crate::policies::{Authenticator, Configurator, UserStorage};
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::{Json, RequestExt};
use axum_extra::extract::cookie::Cookie;
use axum_extra::extract::SignedCookieJar;
use chrono::Utc;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use url::Url;
use uuid::Uuid;

use super::LoginSession;

#[derive(Clone)]
pub struct OAuthAuthenticator {
    login_sessions: Arc<RwLock<HashMap<Uuid, LoginSession>>>,
    auth_url: AuthUrl,
    token_url: TokenUrl,
    scopes: Vec<Scope>,
}

// We pronounce "GitHub" as "j'thoob" here.
#[derive(Clone, Serialize, Deserialize)]
struct GitHubUser {
    login: String,
    email: String,
    name: Option<String>,
}

impl From<GitHubUser> for User {
    fn from(userdata: GitHubUser) -> Self {
        Self {
            name: userdata.login,
            email: userdata.email,
            full_name: userdata.name,
        }
    }
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
    type SessionId = Uuid;
    type Response = (StatusCode, SignedCookieJar, HeaderMap, String);
    type User = User; // TKTK

    async fn start_login_session(&self, req: Request<Body>) -> anyhow::Result<Self::SessionId> {
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

    async fn poll_login_session(&self, bearer: Self::SessionId) -> anyhow::Result<Option<User>> {
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
    async fn complete_login_session<C: Configurator + Send + Sync, U: UserStorage + Send + Sync>(
        &self,
        config: &C,
        user_storage: &U,
        req: Request<Body>,
        bearer: Option<Self::SessionId>,
    ) -> anyhow::Result<Self::Response> {
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

                let user = user_storage.register_user(userdata).await?;

                session.user = Some(user);
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
