use crate::film::WatchedFilm;
use crate::github::{GitHubApiError, GitHubAuthenticatedUserResponse, GITHUB_OAUTH_AUTHORIZE_URL};
use crate::markdown::parse_films_from_markdown;
use crate::redirect::{redirect, RedirectOperation};
use crate::services::Services;
use crate::tokens::Tokens;
use crux_core::{
    macros::effect,
    render::{render, RenderOperation},
    Command,
};
use crux_http::protocol::HttpRequest;
use crux_kv::KeyValueOperation;
use rand::distr::{Alphanumeric, SampleString};
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Default)]
pub struct Model {
    services: Services,
    user_info: Option<UserInfo>,
    films: Vec<WatchedFilm>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ViewModel {
    pub films: Vec<WatchedFilm>,
    pub user_info: Option<UserInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct UserInfo {
    login: String,
    name: String,
    avatar_url: String,
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
    GetWatchHistoryFile {
        user_info: UserInfo,
    },
    #[serde(skip)]
    GotWatchHistoryFile(String),

    // Lifecycle events
    #[serde(skip)]
    OnTokensLoaded {
        tokens: Tokens,
        suppress_store: bool,
    },
}

#[effect(typegen)]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    Http(HttpRequest),
    Redirect(RedirectOperation),
    KeyValue(KeyValueOperation),
}

#[derive(Default)]
pub struct App;

trait IntoEvent<T> {
    fn into_event(self, map: fn(T) -> Event) -> Event;
}

impl<T> IntoEvent<T> for Result<T, GitHubApiError> {
    fn into_event(self, map: fn(T) -> Event) -> Event {
        self.map_or_else(
            |err| match err {
                err @ GitHubApiError::HttpError(_) => panic!("{:?}", err),
                GitHubApiError::ReAuthenticationRequired => Event::RedirectToLogin,
            },
            map,
        )
    }
}

impl crux_core::App for App {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Effect = Effect;

    fn update(&self, msg: Event, model: &mut Model) -> Command<Effect, Event> {
        info!("Event handling started: {:?}", msg);

        match msg {
            Event::InitialLoad => render().and(Command::event(Event::GetGithubUser)),
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

                let mut url = GITHUB_OAUTH_AUTHORIZE_URL.clone();

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
                    .then_send(|x| x.into_event(Event::GotTokensFromGitHub)),
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
                    .then_send(|x| x.into_event(Event::GotGitHubUser)),
            ),
            Event::GotGitHubUser(user) => {
                let user_info = UserInfo {
                    login: user.login.clone(),
                    name: user.name.clone(),
                    avatar_url: user.avatar_url.clone(),
                };

                model.user_info = Some(user_info.clone());

                render().then(Command::event(Event::GetWatchHistoryFile { user_info }))
            }
            Event::OnTokensLoaded {
                tokens,
                suppress_store,
            } => render().and(
                if !suppress_store {
                    Command::event(Event::SetTokensInStore(tokens))
                } else {
                    Command::done()
                }
                .then(Command::event(Event::GetGithubUser)),
            ),
            Event::GetWatchHistoryFile { user_info } => model
                .services
                .github_client
                .get_file_contents(user_info.login, "notes", "watch_history.md")
                .then_send(|x| x.into_event(Event::GotWatchHistoryFile)),
            Event::GotWatchHistoryFile(file) => {
                model.films = parse_films_from_markdown(file);
                render()
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        Self::ViewModel {
            films: model.films.clone(),
            user_info: model.user_info.clone(),
        }
    }
}
