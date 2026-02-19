use crate::github::{GitHubApiError, GitHubAuthenticatedUserResponse, GitHubClient};
use crate::redirect::{redirect, RedirectOperation};
use crate::tokens::{TokenStore, Tokens};
use crux_core::{
    macros::effect,
    render::{render, RenderOperation},
    Command,
};
use crux_http::protocol::HttpRequest;
use crux_http::{HttpError, Response};
use crux_kv::KeyValueOperation;
use markdown::mdast::Node;
use markdown::mdast::Node::Paragraph;
use rand::distr::{Alphanumeric, SampleString};
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
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
    config: Configuration,
}

impl Default for Services {
    fn default() -> Self {
        #[cfg(target_os = "android")]
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Trace)
                .with_tag("core"),
        );

        let config: Configuration =
            toml::from_str(include_str!("config.toml")).expect("failed parsing configuration");

        let token_store = TokenStore;
        let github_client = GitHubClient::new(
            token_store.clone(),
            "https://api.github.com",
            config.github.client_id.clone(),
            config.github.client_secret.clone(),
            config.github.redirect_uri.clone(),
        );

        Self {
            github_client,
            token_store,
            config,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WatchedFilm {
    title: String,
    rating: Rating,
    year_watched: i16,
    month_of_year_watched: i8,
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
                struct Film {
                    title: String,
                    rating: Rating,
                }

                struct Month {
                    month_of_year: i8,
                    films: Vec<Film>,
                }

                struct Year {
                    name: i16,
                    months: Vec<Month>,
                }

                let root = markdown::to_mdast(file.as_str(), &markdown::ParseOptions::default())
                    .unwrap_or_else(|err| panic!("Failed parsing markdown: {:?}", err));

                fn parse_films_from_list(root: Node) -> Vec<WatchedFilm> {
                    use markdown::mdast::*;

                    let mut years: Vec<Box<Year>> = vec![];

                    for curr in root.children().unwrap() {
                        match curr {
                            Node::Heading(Heading {
                                depth: 2, children, ..
                            }) if let Some(Node::Text(Text { value, .. })) = children.first()
                                && let Ok(year) = i16::from_str(value.trim()) =>
                            {
                                let new_year = Year {
                                    name: year,
                                    months: vec![],
                                };

                                years.push(Box::new(new_year));
                            }
                            Node::Heading(Heading {
                                depth: 3, children, ..
                            }) if let Some(Node::Text(Text {
                                value: month_str, ..
                            })) = children.first()
                                && let Some(month) = month_of_year_from_str(month_str)
                                && let Some(current_year) = years.last_mut() =>
                            {
                                let new_month = Month {
                                    month_of_year: month,
                                    films: vec![],
                                };

                                current_year.months.push(new_month);
                            }
                            Node::List(List { children, .. })
                                if let Some(current_year) = years.last_mut()
                                    && let Some(current_month) = current_year.months.last_mut() =>
                            {
                                for child in children {
                                    match child {
                                        Node::ListItem(ListItem { children, .. })
                                            if let Some(Node::Paragraph(Paragraph {
                                                children,
                                                ..
                                            })) = children.first()
                                                && let Some(Node::Text(Text { value, .. })) =
                                                    children.first()
                                                && let Some((film, rating_str)) =
                                                    value.split_once('-')
                                                && let Some(rating) =
                                                    rating_from_str(rating_str) =>
                                        {
                                            let film = Film {
                                                title: film.trim().to_string(),
                                                rating,
                                            };

                                            current_month.films.push(film);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    fn month_of_year_from_str(s: &str) -> Option<i8> {
                        match s.trim().to_lowercase().as_str() {
                            "january" => Some(1),
                            "february" => Some(2),
                            "march" => Some(3),
                            "april" => Some(4),
                            "may" => Some(5),
                            "june" => Some(6),
                            "july" => Some(7),
                            "august" => Some(8),
                            "september" => Some(9),
                            "october" => Some(10),
                            "november" => Some(11),
                            "december" => Some(12),
                            _ => None,
                        }
                    }

                    fn rating_from_str(s: &str) -> Option<Rating> {
                        match s.trim().to_lowercase().as_str() {
                            "very bad" => Some(Rating::VeryBad),
                            "bad" => Some(Rating::Bad),
                            "meh" => Some(Rating::Meh),
                            "good" => Some(Rating::Good),
                            "very good" => Some(Rating::VeryGood),
                            "goat" => Some(Rating::Goat),
                            _ => None,
                        }
                    }

                    years
                        .iter()
                        .flat_map(|year| {
                            year.months.iter().flat_map(|month| {
                                month.films.iter().map(|film| WatchedFilm {
                                    title: film.title.clone(),
                                    rating: film.rating.clone(),
                                    year_watched: year.name,
                                    month_of_year_watched: month.month_of_year,
                                })
                            })
                        })
                        .collect()
                }

                model.films = parse_films_from_list(root);

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
