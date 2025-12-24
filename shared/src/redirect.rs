use crux_core::capability::Operation;
use crux_core::{Command, Request};
use crux_http::http::convert::{Deserialize, Serialize};
use url::Url;

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