use crate::github::{GitHubApiError, GitHubAuthenticatedUserResponse, GitHubClient};
use crate::tokens::{TokenStore, Tokens};
use crux_core::capability::Operation;
use crux_core::{
    macros::effect,
    render::{render, RenderOperation},
    Command, Request,
};
use crux_http::protocol::HttpRequest;
use crux_http::{HttpError, Response};
use crux_kv::KeyValueOperation;
use rand::distr::{Alphanumeric, SampleString};
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use url::Url;
use url_macro::url;

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
}

pub struct Services {
    github_client: GitHubClient,
    token_store: TokenStore,
    logger: Logger,
    config: Configuration,
}

impl Default for Services {
    fn default() -> Self {
        let config: Configuration =
            toml::from_str(include_str!("config.toml")).expect("failed parsing configuration");

        let token_store = TokenStore;
        let github_client = GitHubClient::new(
            token_store.clone(),
            config.github.client_id.clone(),
            config.github.client_secret.clone(),
            config.github.redirect_uri.clone(),
        );
        let logger = Logger::default();

        Self {
            github_client,
            token_store,
            logger,
            config,
        }
    }
}

#[derive(Default)]
pub struct Logger {
    current: Vec<LogEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LogEntry {
    level: LogLevel,
    message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

impl Logger {
    pub fn info(&mut self, message: String) {
        self.current.push(LogEntry {
            level: LogLevel::Info,
            message,
        });
    }

    pub fn warning(&mut self, message: String) {
        self.current.push(LogEntry {
            level: LogLevel::Warning,
            message,
        });
    }

    pub fn error(&mut self, message: String) {
        self.current.push(LogEntry {
            level: LogLevel::Error,
            message,
        });
    }

    pub fn clear(&mut self) {
        self.current.clear();
    }

    pub fn pop_all(&mut self) -> Vec<LogEntry> {
        let entries = self.current.clone();
        self.current.clear();
        entries
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
    LogoutButtonClicked,
    CallbackReceived(String),

    // Local core events
    #[serde(skip)]
    RedirectToLogin,
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
    GetGithubUser,
    #[serde(skip)]
    GotGitHubUser(GitHubAuthenticatedUserResponse),
    #[serde(skip)]
    OnTokensLoaded {
        tokens: Tokens,
        suppress_store: bool,
    },
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

#[effect(typegen)]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    Http(HttpRequest),
    Redirect(RedirectOperation),
    KeyValue(KeyValueOperation),
    Log(LogOperation),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RedirectOperation {
    pub url: String,
}

impl Operation for RedirectOperation {
    type Output = ();
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LogOperation {
    pub entries: Vec<LogEntry>,
}

impl Operation for LogOperation {
    type Output = ();
}

pub fn redirect<Effect, Event>(url: Url) -> Command<Effect, Event>
where
    Effect: Send + From<Request<RedirectOperation>> + 'static,
    Event: Send + 'static,
{
    Command::request_from_shell(RedirectOperation { url: url.into() }).build()
}

pub fn log<Effect, Event>(entries: Vec<LogEntry>) -> Command<Effect, Event>
where
    Effect: Send + From<Request<LogOperation>> + 'static,
    Event: Send + 'static,
{
    Command::request_from_shell(LogOperation { entries }).build()
}

#[derive(Default)]
pub struct App;

impl crux_core::App for App {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Effect = Effect;

    fn update(&self, msg: Event, model: &mut Model) -> Command<Effect, Event> {
        model
            .services
            .logger
            .info(format!("Event handling started: {:?}", msg));

        let cmd = match msg {
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

                render().and(Command::event(Event::GetGithubUser))
            }
            Event::SetTokensInStore(store) => {
                render().and(model.services.token_store.set_tokens(store).build())
            }
            Event::GetTokensFromStore => render().and(
                model
                    .services
                    .token_store
                    .get_tokens()
                    .then_send(Event::GotTokensFromStore),
            ),
            Event::GotTokensFromStore(Some(store)) => {
                render().and(Command::event(Event::OnTokensLoaded {
                    tokens: store,
                    suppress_store: true,
                }))
            }
            Event::GotTokensFromStore(None) => render(),
            Event::LoginButtonClicked => render().and(Command::event(Event::RedirectToLogin)),
            Event::LogoutButtonClicked => {
                model.user_info = None;
                render().then(model.services.token_store.remove_tokens().build())
            }
            Event::RedirectToLogin => {
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
                    client_id: model.services.config.github.client_id.clone(),
                    redirect_uri: model.services.config.github.redirect_uri.clone(),
                    state,
                };

                url.set_query(serde_qs::to_string(&query_params).ok().as_deref());

                redirect(url)
            }
            Event::CallbackReceived(url) => {
                let code = Url::parse(&url)
                    .expect("invalid callback URL")
                    .query_pairs()
                    .find_map(|(key, val)| {
                        if key == "code" {
                            Some(val.into_owned())
                        } else {
                            None
                        }
                    });

                render().and(Command::event(Event::GetTokensFromGitHub { code }))
            }
            Event::GetTokensFromGitHub { code: None } => render(),
            Event::GetTokensFromGitHub { code: Some(code) } => render().and(
                model
                    .services
                    .github_client
                    .get_access_token_from_code(code)
                    .then_send(Event::GotTokensFromGitHub),
            ),
            Event::GotTokensFromGitHub(store) => {
                render().and(Command::event(Event::OnTokensLoaded {
                    tokens: store,
                    suppress_store: false,
                }))
            }
            Event::GetGithubUser => render().and(
                model
                    .services
                    .github_client
                    .get_authenticated_user()
                    .then_send(|x| {
                        x.map_or_else(
                            |err| match err {
                                GitHubApiError::HttpError(err) => panic!("{}", err.to_string()),
                                GitHubApiError::ReAuthenticationRequired => panic!(),
                            },
                            Event::GotGitHubUser,
                        )
                    }),
            ),
            Event::GotGitHubUser(user) => {
                model.user_info = Some(UserInfo {
                    name: user.name.clone(),
                    avatar_url: user.avatar_url.clone(),
                });

                render()
            }
            Event::OnTokensLoaded {
                tokens,
                suppress_store,
            } => render().and(Command::all(vec![if !suppress_store {
                Command::event(Event::SetTokensInStore(tokens))
            } else {
                Command::done()
            }
            .then(Command::event(Event::GetGithubUser))])),
        };

        cmd.and(log(model.services.logger.pop_all()))
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        Self::ViewModel {
            films: model.films.clone(),
            user_info: model.user_info.clone(),
        }
    }
}
