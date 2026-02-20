use crate::config::Configuration;
use crate::github::GitHubClient;
use crate::tokens::TokenStore;

pub struct Services {
    pub github_client: GitHubClient,
    pub token_store: TokenStore,
    pub config: Configuration,
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