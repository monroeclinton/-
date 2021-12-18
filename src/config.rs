use serde::Deserialize;
use std::fs;
use std::io::Error;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub control_socket_path: String,
    pub ip_addr: String,
    pub ports: Vec<u16>,
    #[serde(default = "default_debug")]
    pub debug: bool,
    pub apps: Vec<App>,
}

#[derive(Clone, Deserialize)]
pub struct App {
    pub ip_addr: String,
    pub targets: Vec<AppTarget>,
}

#[derive(Clone, Deserialize)]
pub struct AppTarget {
    pub ip_addr: String,
    pub weight: u8,
}

pub fn get_config() -> Result<Config, Error> {
    let toml_string = fs::read_to_string("config.toml")?;

    let config = toml::from_str(&toml_string)?;

    Ok(config)
}

// Defaults
fn default_debug() -> bool { false }
