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

const VALID_REGIONS: &'static [&'static str] = &["US", "EU", "DEV", "SH"];

impl Config {
    pub fn new(app_key: String, host: Option<String>) -> Self {
        let parts = app_key.split("-").collect::<Vec<&str>>();
        if parts.len() != 3 || !VALID_REGIONS.contains(&parts[1]) {
            debug!("The Aptabase App Key '{}' is invalid. Tracking will be disabled.", app_key);
            return Config::default();
        }

        let base_url: String = match parts[1] {
            "EU" => EU_REGION.into(),
            "US" => US_REGION.into(),
            "DEV" => LOCAL.into(),
            "SH" => {
                if let Some(host) = host {
                    host
                } else {
                    debug!("Host parameter must be defined when using Self-Hosted App Key. Tracking will be disabled.");
                    return Config::default();
                }
            },
            _ => return Config::default(),
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
