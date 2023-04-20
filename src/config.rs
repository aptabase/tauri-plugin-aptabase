use log::debug;
use reqwest::Url;

#[derive(Debug, Clone)]
pub struct Config {
    pub app_key: String,
    pub ingest_api_url: Url,
}

static LOCAL: &str = "http://localhost:3000";
static US_REGION: &str = "https://us.aptabase.com";
static EU_REGION: &str = "https://eu.aptabase.com";


impl Config {
    pub fn with_app_key(app_key: String) -> Self {
        let parts = app_key.split("-").collect::<Vec<&str>>();
        if parts.len() != 3 {
            debug!("The Aptabase App Key '{}' is invalid. Tracking will be disabled.", app_key);
            return Config::default();
        }

        let base_url = match parts[1] {
            "EU" => EU_REGION,
            "US" => US_REGION,
            "DEV" => LOCAL,
            _ => LOCAL,
        };

        Config {
            app_key,
            ingest_api_url: format!("{}/api/v0/event", base_url).parse().unwrap(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        return Config {
            app_key: String::new(),
            ingest_api_url: Url::parse(LOCAL).unwrap(),
        };
    }
}
