use reqwest::Url;

#[derive(Debug, Clone)]
pub struct Config {
    pub app_key: String,
    pub ingest_api_url: Url,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_key: "".to_owned(),
            ingest_api_url: "http://localhost:3000/v0/events".parse().unwrap(),
        }
    }
}

impl Config {
    pub fn with_app_key(app_key: String) -> Self {
        Config {
            app_key,
            ingest_api_url: "http://localhost:3000/v0/events".parse().unwrap(),
        }
    }
}