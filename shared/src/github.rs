use crate::tokens::{Token, TokenStore, Tokens};
use crate::{Effect, Event, Logger};
use chrono::{Duration, Utc};
use crux_core::command::RequestBuilder;
use crux_http::http::convert::{Deserialize, Serialize};
use crux_http::{Http, HttpError};
use std::future::Future;
use url_macro::url;

const GITHUB_RAW_MEDIA_TYPE_NAME: &str = "application/vnd.github.raw+json";
const GITHUB_JSON_MEDIA_TYPE_NAME: &str = "application/vnd.github+json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
struct GitHubAccessTokenResponse {
    access_token: String,
    token_type: String,
    scope: String,
    expires_in: u64,
    refresh_token: String,
    refresh_token_expires_in: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GitHubAuthenticatedUserResponse {
    pub login: String,
    pub name: String,
    pub avatar_url: String,
}

pub enum GitHubApiError {
    HttpError(HttpError),
    ReAuthenticationRequired,
}

impl From<HttpError> for GitHubApiError {
    fn from(value: HttpError) -> Self {
        GitHubApiError::HttpError(value)
    }
}

pub struct GitHubClient {
    base_url: String,
    token_manager: GitHubTokenManager,
}

impl GitHubClient {
    pub fn new(
        token_store: TokenStore,
        base_url: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        redirect_uri: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            token_manager: GitHubTokenManager {
                token_store,
                github_auth_handler: GitHubAuthenticationHandler::new(
                    client_id,
                    client_secret,
                    redirect_uri,
                ),
            },
        }
    }

    fn build_url(&self, endpoint: impl Into<String>) -> String {
        format!(
            "{}/{}",
            self.base_url.clone().trim_end_matches('/'),
            endpoint.into().trim_start_matches('/')
        )
    }

    pub fn get_access_token_from_code(
        &self,
        code: impl Into<String>,
    ) -> RequestBuilder<Effect, Event, impl Future<Output = Tokens>> {
        self.token_manager.get_access_token_from_code(code)
    }

    pub fn get_authenticated_user(
        &self,
    ) -> RequestBuilder<
        Effect,
        Event,
        impl Future<Output = Result<GitHubAuthenticatedUserResponse, GitHubApiError>>,
    > {
        let url = self.build_url("user");

        self.token_manager
            .get_access_token()
            .then_request(|access_token| {
                RequestBuilder::new(|ctx| async move {
                    if let Some(access_token) = access_token {
                        let res = Http::get(url)
                            .header(
                                "Authorization",
                                access_token.to_authorization_header_value(),
                            )
                            .header("Accept", GITHUB_JSON_MEDIA_TYPE_NAME)
                            .expect_json::<GitHubAuthenticatedUserResponse>()
                            .build()
                            .into_future(ctx.clone())
                            .await?
                            .body()
                            .cloned()
                            .unwrap();

                        Ok(res)
                    } else {
                        Err(GitHubApiError::ReAuthenticationRequired)
                    }
                })
            })
    }

    pub fn get_file_contents(
        &self,
        owner: impl Into<String>,
        repo: impl Into<String>,
        path: impl Into<String>,
    ) -> RequestBuilder<Effect, Event, impl Future<Output = Result<String, GitHubApiError>>> {
        let url = self.build_url(format!("repos/{}/{}/contents/{}", owner.into(), repo.into(), path.into()));

        self.token_manager
            .get_access_token()
            .then_request(|access_token| {
                RequestBuilder::new(|ctx| async move {
                    if let Some(access_token) = access_token {
                        let res = Http::get(url)
                            .header(
                                "Authorization",
                                access_token.to_authorization_header_value(),
                            )
                            .header("Accept", GITHUB_RAW_MEDIA_TYPE_NAME)
                            .expect_string()
                            .build()
                            .into_future(ctx.clone())
                            .await?
                            .body()
                            .cloned()
                            .unwrap();

                        Ok(res)
                    } else {
                        Err(GitHubApiError::ReAuthenticationRequired)
                    }
                })
            })
    }
}

#[derive(Clone)]
pub struct GitHubAuthenticationHandler {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

impl GitHubAuthenticationHandler {
    fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        redirect_uri: impl Into<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            redirect_uri: redirect_uri.into(),
        }
    }

    pub fn get_access_token_from_code(
        &self,
        code: impl Into<String>,
    ) -> RequestBuilder<Effect, Event, impl Future<Output = Tokens>> {
        #[derive(Serialize)]
        struct QueryParams {
            client_id: String,
            client_secret: String,
            redirect_uri: String,
            code: String,
        }

        let query_params = QueryParams {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            code: code.into(),
            redirect_uri: self.redirect_uri.clone(),
        };

        self.get_access_token(query_params)
    }

    fn get_access_token_from_refresh_token(
        &self,
        refresh_token: impl Into<String>,
    ) -> RequestBuilder<Effect, Event, impl Future<Output = Tokens>> {
        #[derive(Serialize)]
        struct QueryParams {
            client_id: String,
            client_secret: String,
            grant_type: String,
            refresh_token: String,
        }

        let query_params = QueryParams {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            grant_type: "refresh_token".into(),
            refresh_token: refresh_token.into(),
        };

        self.get_access_token(query_params)
    }

    fn get_access_token<Query: Serialize>(
        &self,
        query_params: Query,
    ) -> RequestBuilder<Effect, Event, impl Future<Output = Tokens>> {
        let url = url!("https://github.com/login/oauth/access_token");

        Http::post(url)
            .header("Accept", GITHUB_JSON_MEDIA_TYPE_NAME)
            .query(&query_params)
            .unwrap()
            .expect_json::<GitHubAccessTokenResponse>()
            .build()
            .map(|x| x.ok().unwrap().body().unwrap().clone().into())
    }
}

impl From<GitHubAccessTokenResponse> for Tokens {
    fn from(value: GitHubAccessTokenResponse) -> Self {
        let now = Utc::now();
        Self {
            access_token: Token::new(
                value.token_type.clone(),
                value.access_token,
                now + Duration::seconds(value.expires_in as i64),
            ),
            refresh_token: Token::new(
                value.token_type.clone(),
                value.refresh_token,
                now + Duration::seconds(value.refresh_token_expires_in as i64),
            ),
        }
    }
}

#[derive(Clone)]
struct GitHubTokenManager {
    token_store: TokenStore,
    github_auth_handler: GitHubAuthenticationHandler,
}

impl GitHubTokenManager {
    fn get_access_token(
        &self,
    ) -> RequestBuilder<Effect, Event, impl Future<Output = Option<Token>>> {
        let github_client = self.github_auth_handler.clone();
        let token_store = self.token_store.clone();
        token_store.get_tokens().then_request(|tokens| {
            RequestBuilder::new(|ctx| async move {
                if let Some(tokens) = tokens {
                    token_store
                        .set_tokens(tokens.clone())
                        .into_future(ctx.clone())
                        .await;

                    if tokens.access_token.is_valid() {
                        Some(tokens.access_token.clone())
                    } else if tokens.refresh_token.is_valid() {
                        github_client
                            .get_access_token_from_refresh_token(
                                tokens.refresh_token.access_token.clone(),
                            )
                            .map(|tokens| Some(tokens.access_token.clone()))
                            .into_future(ctx.clone())
                            .await
                            .clone()
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
        })
    }

    fn get_access_token_from_code(
        &self,
        code: impl Into<String>,
    ) -> RequestBuilder<Effect, Event, impl Future<Output = Tokens>> {
        self.github_auth_handler.get_access_token_from_code(code)
    }
}
