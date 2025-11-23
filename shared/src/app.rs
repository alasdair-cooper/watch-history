use crux_core::{
    macros::effect,
    render::{render, RenderOperation},
    Command,
};
use crux_http::protocol::HttpRequest;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize)]
pub struct Model {
    count: Count,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq)]
pub struct Count {
    value: isize,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ViewModel {
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Increment,
    Decrement,
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
            Event::Increment => {
                model.count = Count {
                    value: model.count.value + 1,
                };

                render()
            }
            Event::Decrement => {
                model.count = Count {
                    value: model.count.value - 1,
                };

                render()
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        Self::ViewModel {
            text: model.count.value.to_string(),
        }
    }
}
