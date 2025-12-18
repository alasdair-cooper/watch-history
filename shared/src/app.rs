use crux_core::capability::Operation;
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

#[derive(Default, Serialize)]
pub struct Model {
    user_info: Option<UserInfo>,
    films: Vec<WatchedFilm>,
    log: VecDeque<String>,
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

    #[serde(skip)]
    GetStoredTokens,
    #[serde(skip)]
    GotStoredTokens {
        tokens: Option<GitHubAccessTokenResponse>,
    },
    #[serde(skip)]
    GetAccessToken {
        code: Option<String>,
    },
    #[serde(skip)]
    GotAccessToken(HttpResult<Response<GitHubAccessTokenResponse>, HttpError>),
    #[serde(skip)]
    GetGithubUser {
        access_token: String,
    },
    #[serde(skip)]
    GotGitHubUser(HttpResult<Response<GitHubAuthenticatedUserResponse>, HttpError>),
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
    expires_in: Option<u64>,
    refresh_token: Option<String>,
    refresh_token_expires_in: Option<u64>,
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
        match msg {
            Event::InitialLoad => {
                model.log.push_back("Init".into());

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

                Command::event(Event::GetStoredTokens)
            }
            Event::GetStoredTokens => KeyValue::get("github_tokens").then_send(|bytes| {
                bytes
                    .ok()
                    .flatten()
                    .and_then(|data| bincode::deserialize::<GitHubAccessTokenResponse>(&data).ok())
                    .map_or_else(
                        || Event::GotStoredTokens { tokens: None },
                        |tokens| Event::GotStoredTokens {
                            tokens: Some(tokens),
                        },
                    )
            }),
            Event::GotStoredTokens {
                tokens: Some(tokens),
            } => Command::event(Event::GetGithubUser {
                access_token: tokens.access_token,
            }),
            Event::GotStoredTokens { tokens: None } => {
                render()
                // todo!("Show login page");
            }
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

                model
                    .log
                    .push_back(format!("Callback received with code {:?}", code));

                Command::event(Event::GetAccessToken { code })
            }
            Event::GetAccessToken { code: None } => {
                model.log.push_back("Missing access token".into());
                render()
            }
            Event::GetAccessToken { code: Some(code) } => {
                model.log.push_back("Getting access token".into());

                let get_access_token = {
                    #[derive(Serialize)]
                    struct QueryParams {
                        client_id: String,
                        client_secret: String,
                        redirect_uri: String,
                        code: String,
                    }

                    let url = url!("https://github.com/login/oauth/access_token");

                    let query_params = QueryParams {
                        client_id: self.config.github.client_id.clone(),
                        client_secret: self.config.github.client_secret.clone(),
                        code,
                        redirect_uri: self.config.github.redirect_uri.clone(),
                    };

                    Http::post(url)
                        .header("Accept", GITHUB_JSON_MEDIA_TYPE_NAME)
                        .query(&query_params)
                        .unwrap()
                        .expect_json()
                        .build()
                        .map(Into::into)
                        .then_send(Event::GotAccessToken)
                };

                render().and(get_access_token)
            }
            Event::GotAccessToken(res) => match res {
                HttpResult::Ok(response) => {
                    let access_token_response = response.body().unwrap();

                    let data = bincode::serialize(access_token_response).unwrap();

                    KeyValue::set("github_tokens", data)
                        .build()
                        .then(Command::event(Event::GetGithubUser {
                            access_token: access_token_response.access_token.clone(),
                        }))
                }
                HttpResult::Err(res) => {
                    model
                        .log
                        .push_back(format!("Failed fetching access token: {:?}", res));

                    render()
                }
            },
            Event::GetGithubUser { access_token } => {
                let get_github_user = {
                    Http::get("https://api.github.com/user")
                        .header("Authorization", format!("Bearer {access_token}"))
                        .header("Accept", GITHUB_JSON_MEDIA_TYPE_NAME)
                        .expect_json()
                        .build()
                        .map(Into::into)
                        .then_send(Event::GotGitHubUser)
                };

                render().and(get_github_user)
            }
            Event::GotGitHubUser(res) => match res {
                HttpResult::Ok(response) => {
                    let user = response.body().unwrap();

                    model.log.push_back("Got user".into());

                    model.user_info = Some(UserInfo {
                        name: user.name.clone(),
                        avatar_url: user.avatar_url.clone(),
                    });

                    render()
                }
                HttpResult::Err(_) => {
                    panic!()
                }
            },
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
