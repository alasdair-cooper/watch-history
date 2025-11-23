use crux_core::{
    macros::effect,
    render::{render, RenderOperation},
    Command,
};
use crux_http::protocol::HttpRequest;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize)]
pub struct Model {
    films: Vec<WatchedFilm>,
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
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Event {
    InitialLoad,
}

#[effect(typegen)]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    Http(HttpRequest),
}

#[derive(Default)]
pub struct App;

impl crux_core::App for App {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Effect = Effect;

    fn update(&self, msg: Event, model: &mut Model) -> Command<Effect, Event> {
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

                render()
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        Self::ViewModel {
            films: model.films.clone(),
        }
    }
}
