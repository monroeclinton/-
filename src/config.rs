use serde::Deserialize;
use std::{fs, net::IpAddr};

#[derive(Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_debug")]
    pub debug: bool,
    #[serde(default = "default_ip_addr")]
    pub ip_addr: IpAddr,
    #[serde(default = "default_port")]
    pub port: u16,
    pub apps: Vec<App>,
}

#[derive(Clone, Deserialize)]
pub struct App {
    pub uuid: String,
    pub ip_addr: IpAddr,
    pub targets: Vec<AppTarget>,
}

#[derive(Clone, Deserialize)]
pub struct AppTarget {
    pub ip_addr: IpAddr,
    pub weight: u8,
}

pub fn get_config() -> anyhow::Result<Config> {
    let toml_string = fs::read_to_string("config.toml")?;
    let config = toml::from_str(&toml_string)?;
    Ok(config)
}

// Defaults
fn default_debug() -> bool {
    match std::env::var("ENV") {
        Ok(e) => e == "development",
        _ => false,
    }
}

fn default_ip_addr() -> IpAddr {
    "0.0.0.0".parse().unwrap()
}

fn default_port() -> u16 {
    8080
}
