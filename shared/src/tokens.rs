use crate::{Effect, Event};
use chrono::{DateTime, Utc};
use crux_core::command::RequestBuilder;
use crux_http::http::convert::{Deserialize, Serialize};
use crux_kv::KeyValue;
use std::future::Future;

const GITHUB_TOKENS_STORAGE_KEY: &str = "github_tokens";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Tokens {
    pub access_token: Token,
    pub refresh_token: Token,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub token_type: String,
    pub access_token: String,
    pub expires_at: DateTime<Utc>,
}

impl Token {
    pub fn new(token_type: String, access_token: String, expires_at: DateTime<Utc>) -> Self {
        Self {
            token_type,
            access_token,
            expires_at,
        }
    }

    pub fn is_valid(&self) -> bool {
        Utc::now() < self.expires_at
    }

    pub fn to_authorization_header_value(&self) -> String {
        format!("{} {}", self.token_type, self.access_token)
    }
}

#[derive(Clone)]
pub struct TokenStore;

impl TokenStore {
    pub fn get_tokens(
        &self,
    ) -> RequestBuilder<Effect, Event, impl Future<Output = Option<Tokens>>> {
        KeyValue::get(GITHUB_TOKENS_STORAGE_KEY).map(|x| {
            x.ok()
                .flatten()
                .and_then(|data| bincode::deserialize::<Tokens>(&data).ok())
        })
    }

    pub fn set_tokens(
        &self,
        tokens: Tokens,
    ) -> RequestBuilder<Effect, Event, impl Future<Output = ()>> {
        KeyValue::set(
            GITHUB_TOKENS_STORAGE_KEY,
            bincode::serialize(&tokens).unwrap(),
        )
        .map(|_| ())
    }

    pub fn remove_tokens(&self) -> RequestBuilder<Effect, Event, impl Future<Output = ()>> {
        KeyValue::delete(GITHUB_TOKENS_STORAGE_KEY).map(|_| ())
    }
}
