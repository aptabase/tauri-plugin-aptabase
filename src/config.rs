use reqwest::Url;

#[derive(Debug, Clone)]
pub struct Config {
    pub app_key: String,
    pub ingest_api_url: Url,
}

static LOCAL: &str = "http://localhost:5251";
static US_REGION: &str = "https://api-eu.aptabase.com";
static EU_REGION: &str = "https://api-eu.aptabase.com";


impl Config {
    pub fn with_app_key(app_key: String) -> Self {
        let parts = app_key.split("-").collect::<Vec<&str>>();
        if parts.len() != 3 {
            panic!("Invalid Aptabase App Key format");
        }

        let base_url = match parts[1] {
            "EU" => EU_REGION,
            "US" => US_REGION,
            "DEV" => LOCAL,
            _ => LOCAL,
        };

        Config {
            app_key,
            ingest_api_url: format!("{}/v0/event", base_url).parse().unwrap(),
        }
    }
}