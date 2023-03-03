use crate::config::Config;

#[derive(Default)]
pub struct AptabaseState {
    pub config: Config,
    pub http_client: reqwest::Client
}


impl AptabaseState {
    pub fn with_config(config: Config) -> Self {
        AptabaseState {
            config,
            http_client: reqwest::Client::new()
        }
    }
}