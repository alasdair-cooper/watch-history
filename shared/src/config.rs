use crux_http::http::convert::{Deserialize, Serialize};
use crate::github::GitHubConfiguration;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Configuration {
    pub github: GitHubConfiguration,
}