use chrono::{DateTime, Duration, Utc};
use crux_core::capability::Operation;
use crux_core::command::RequestBuilder;
use crux_core::{
    macros::effect,
    render::{render, RenderOperation},
    Command, Request,
};
use crux_http::command::Http;
use crux_http::protocol::HttpRequest;
use crux_http::{HttpError, Response};
use crux_kv::{KeyValue, KeyValueOperation};
use rand::distr::{Alphanumeric, SampleString};
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::future::Future;
use url::Url;
use url_macro::url;

const GITHUB_JSON_MEDIA_TYPE_NAME: &str = "application/vnd.github+json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
struct Configuration {
    github: GitHubConfiguration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
struct GitHubConfiguration {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

#[derive(Default)]
pub struct Model {
    services: Services,
    user_info: Option<UserInfo>,
    films: Vec<WatchedFilm>,
    log: VecDeque<String>,
}

pub struct Services {
    github_client: GitHubClient,
    token_store: TokenStore,
}

impl Default for Services {
    fn default() -> Self {
        let config: Configuration = toml::from_str(include_str!("config.toml")).unwrap();

        Self {
            github_client: GitHubClient::new(
                config.github.client_id,
                config.github.client_secret,
                config.github.redirect_uri,
            ),
            token_store: TokenStore,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WatchedFilm {
    title: String,
    rating: Rating,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Rating {
    VeryBad,
    Bad,
    Meh,
    Good,
    VeryGood,
    Goat,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ViewModel {
    pub log: VecDeque<String>,
    pub films: Vec<WatchedFilm>,
    pub user_info: Option<UserInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct UserInfo {
    pub name: String,
    pub avatar_url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Event {
    InitialLoad,
    LoginButtonClicked,
    CallbackReceived(String),

    // Local core events
    #[serde(skip)]
    SetTokensInStore(Tokens),
    #[serde(skip)]
    GetTokensFromStore,
    #[serde(skip)]
    GotTokensFromStore(Option<Tokens>),
    #[serde(skip)]
    GetTokensFromGitHub {
        code: Option<String>,
    },
    #[serde(skip)]
    GotTokensFromGitHub(Tokens),
    #[serde(skip)]
    GetGithubUser {
        access_token: String,
    },
    #[serde(skip)]
    GotGitHubUser(GitHubAuthenticatedUserResponse),
    #[serde(skip)]
    OnTokensLoaded(Tokens),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[repr(C)]
pub enum HttpResult<T, E> {
    Ok(T),
    Err(E),
}

impl<T> From<crux_http::Result<Response<T>>> for HttpResult<Response<T>, HttpError> {
    fn from(value: crux_http::Result<Response<T>>) -> Self {
        match value {
            Ok(response) => HttpResult::Ok(response),
            Err(error) => HttpResult::Err(error),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GitHubAccessTokenResponse {
    access_token: String,
    token_type: String,
    scope: String,
    expires_in: u64,
    refresh_token: String,
    refresh_token_expires_in: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Tokens {
    access_token: String,
    access_token_expires_at: DateTime<Utc>,
    refresh_token: String,
    refresh_token_expires_at: DateTime<Utc>,
}

impl Tokens {
    fn get_access_token(&self) -> Option<&str> {
        let now = Utc::now();

        if now < self.access_token_expires_at {
            Some(&self.access_token)
        } else if now < self.refresh_token_expires_at {
            None // TODO: Implement refresh token logic
        } else {
            None
        }
    }
}

pub struct TokenStore;

const GITHUB_TOKENS_STORAGE_KEY: &str = "github_tokens";

impl TokenStore {
    fn get_tokens(&self) -> RequestBuilder<Effect, Event, impl Future<Output = Option<Tokens>>> {
        KeyValue::get(GITHUB_TOKENS_STORAGE_KEY).map(|x| {
            x.ok()
                .flatten()
                .and_then(|data| bincode::deserialize::<Tokens>(&data).ok())
        })
    }

    fn set_tokens(
        &self,
        tokens: Tokens,
    ) -> RequestBuilder<Effect, Event, impl Future<Output = ()>> {
        KeyValue::set(
            GITHUB_TOKENS_STORAGE_KEY,
            bincode::serialize(&tokens).unwrap(),
        )
        .map(|_| ())
    }
}

pub struct GitHubClient {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

impl GitHubClient {
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

    fn get_access_token_from_code(
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

    fn get_authenticated_user(
        &self,
        access_token: impl Into<String>,
    ) -> RequestBuilder<Effect, Event, impl Future<Output = GitHubAuthenticatedUserResponse>> {
        Http::get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", access_token.into()))
            .header("Accept", GITHUB_JSON_MEDIA_TYPE_NAME)
            .expect_json::<GitHubAuthenticatedUserResponse>()
            .build()
            .map(|x| x.ok().unwrap().body().unwrap().clone())
    }
}

impl From<GitHubAccessTokenResponse> for Tokens {
    fn from(value: GitHubAccessTokenResponse) -> Self {
        let now = Utc::now();

        Self {
            access_token: value.access_token.clone(),
            access_token_expires_at: now + Duration::seconds(value.expires_in as i64),
            refresh_token: value.refresh_token.clone(),
            refresh_token_expires_at: now
                + Duration::seconds(value.refresh_token_expires_in as i64),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GitHubAuthenticatedUserResponse {
    name: String,
    avatar_url: String,
}

#[effect(typegen)]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    Http(HttpRequest),
    Redirect(RedirectOperation),
    KeyValue(KeyValueOperation),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RedirectOperation {
    pub url: String,
}

impl Operation for RedirectOperation {
    type Output = ();
}

pub fn redirect<Effect, Event>(url: Url) -> Command<Effect, Event>
where
    Effect: Send + From<Request<RedirectOperation>> + 'static,
    Event: Send + 'static,
{
    Command::request_from_shell(RedirectOperation { url: url.into() }).build()
}

pub struct App {
    config: Configuration,
}

impl Default for App {
    fn default() -> Self {
        let config = toml::from_str(include_str!("config.toml")).unwrap();

        Self { config }
    }
}

impl crux_core::App for App {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Effect = Effect;

    fn update(&self, msg: Event, model: &mut Model) -> Command<Effect, Event> {
        model.log.push_back(format!("Event: {:?}", msg));

        match msg {
            Event::InitialLoad => {
                model.films = vec![
                    WatchedFilm {
                        title: "Frankenstein".to_string(),
                        rating: Rating::Meh,
                    },
                    WatchedFilm {
                        title: "American Psycho".to_string(),
                        rating: Rating::VeryGood,
                    },
                    WatchedFilm {
                        title: "The Equalizer 2".to_string(),
                        rating: Rating::Good,
                    },
                    WatchedFilm {
                        title: "The Equalizer 3".to_string(),
                        rating: Rating::VeryGood,
                    },
                ];

                Command::event(Event::GetTokensFromStore)
            }
            Event::SetTokensInStore(store) => {
                render().and(model.services.token_store.set_tokens(store).build())
            }
            Event::GetTokensFromStore => model
                .services
                .token_store
                .get_tokens()
                .then_send(|x| Event::GotTokensFromStore(x)),
            Event::GotTokensFromStore(Some(store)) => {
                render().and(Command::event(Event::OnTokensLoaded(store)))
            }
            Event::GotTokensFromStore(None) => render(),
            Event::LoginButtonClicked => {
                #[derive(Serialize)]
                struct QueryParams {
                    client_id: String,
                    redirect_uri: String,
                    state: String,
                }

                let mut rng = StdRng::from_os_rng();
                let state = Alphanumeric.sample_string(&mut rng, 16);

                let mut url = url!("https://github.com/login/oauth/authorize");

                let query_params = QueryParams {
                    client_id: self.config.github.client_id.clone(),
                    redirect_uri: self.config.github.redirect_uri.clone(),
                    state,
                };

                url.set_query(serde_qs::to_string(&query_params).ok().as_deref());

                redirect(url)
            }
            Event::CallbackReceived(url) => {
                let code = Url::parse(&url)
                    .unwrap()
                    .query_pairs()
                    .find_map(|(key, val)| {
                        if key == "code" {
                            Some(val.into_owned())
                        } else {
                            None
                        }
                    });

                Command::event(Event::GetTokensFromGitHub { code })
            }
            Event::GetTokensFromGitHub { code: None } => render(),
            Event::GetTokensFromGitHub { code: Some(code) } => render().and(
                model
                    .services
                    .github_client
                    .get_access_token_from_code(code)
                    .then_send(|x| Event::GotTokensFromGitHub(x)),
            ),
            Event::GotTokensFromGitHub(store) => Command::event(Event::OnTokensLoaded(store)),
            Event::GetGithubUser { access_token } => render().and(
                model
                    .services
                    .github_client
                    .get_authenticated_user(access_token)
                    .then_send(|x| Event::GotGitHubUser(x)),
            ),
            Event::GotGitHubUser(user) => {
                model.user_info = Some(UserInfo {
                    name: user.name.clone(),
                    avatar_url: user.avatar_url.clone(),
                });

                render()
            }
            Event::OnTokensLoaded(store) => render().and(Command::all(vec![
                Command::event(Event::GetGithubUser {
                    access_token: store.access_token.clone(),
                }),
                Command::event(Event::SetTokensInStore(store)),
            ])),
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        Self::ViewModel {
            log: model.log.clone(),
            films: model.films.clone(),
            user_info: model.user_info.clone(),
        }
    }
}
