use core::str::FromStr;
use serde::{
    de::{self, Deserializer},
    Deserialize,
};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use toml;

use super::util::{Lifetime, Prefix, ResponseFormat};

pub const DEFAULT_CONFIG_PATH: &str = "~/.kilobytetools/config.toml";

pub fn load(config_path: &str) -> Result<ConfigFile, ErrorKind> {
    let cfg_str = fs::read_to_string(expand_tilde(config_path))?;
    Ok(toml::from_str(&cfg_str)?)
}

pub fn exists(config_path: &str) -> bool {
    fs::metadata(expand_tilde(config_path)).is_ok()
}

pub fn write(config_path: &str, data: String) -> io::Result<()> {
    let path = expand_tilde(config_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, data)?;
    Ok(())
}

fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~") {
        let rest = &path[1..];
        if rest.is_empty() || rest.starts_with("/") {
            let home = dirs_next::home_dir();
            return Path::new(format!("{}{}", home.unwrap().display(), rest).as_str()).into();
        }
    }
    path.into()
}

pub enum ErrorKind {
    IoError(std::io::Error),
    DeError(toml::de::Error),
}
impl From<std::io::Error> for ErrorKind {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}
impl From<toml::de::Error> for ErrorKind {
    fn from(e: toml::de::Error) -> Self {
        Self::DeError(e)
    }
}

#[derive(Deserialize, Default)]
pub struct ConfigFile {
    #[serde(rename = "endpoint")]
    pub endpoint: Option<String>,

    #[serde(rename = "api_key")]
    pub api_key: Option<String>,

    #[serde(default, rename = "response")]
    pub response: ResponseConfig,

    #[serde(default, rename = "scratch-push")]
    pub push: PushConfig,
}

#[derive(Deserialize, Default)]
pub struct ResponseConfig {
    #[serde(default)]
    pub format: Option<ResponseFormat>,
}

#[derive(Deserialize, Default)]
pub struct PushConfig {
    #[serde(rename = "burn")]
    pub burn: Option<bool>,

    #[serde(rename = "lifetime", default)]
    pub lifetime: Option<Lifetime>,

    #[serde(rename = "prefix", default)]
    pub prefix: Option<Prefix>,

    #[serde(rename = "private")]
    pub private: Option<bool>,
}

impl<'de> Deserialize<'de> for Lifetime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for Prefix {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for ResponseFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}
