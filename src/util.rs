use core::str::FromStr;
use lazy_static::lazy_static;
use regex::Regex;
use std::{
    error::Error as StdError,
    fmt::Display,
    fs,
    io::{self, Read},
    path::Path,
};

#[derive(Debug)]
pub enum Error {
    MalformedArgument(&'static str, String, String),
}

impl StdError for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::MalformedArgument(name, unexp, exp) => {
                write!(f, "{} was {} but must match {}", name, unexp, exp)
            }
        }
    }
}

pub enum InputMode {
    Buffer(Vec<u8>),
    File(fs::File),
}

impl InputMode {
    pub fn from_stdin() -> Result<Self, io::Error> {
        let mut buf = Vec::new();
        let mut stdin = io::stdin();
        let _ = stdin.read_to_end(&mut buf)?;
        InputMode::from_buffer(buf)
    }
    pub fn from_buffer(buf: Vec<u8>) -> Result<Self, io::Error> {
        Ok(InputMode::Buffer(buf))
    }
    pub fn from_filename(name: impl AsRef<Path>) -> Result<Self, io::Error> {
        Ok(InputMode::File(fs::File::open(name)?))
    }

    pub fn size(&self) -> u64 {
        match self {
            InputMode::Buffer(buf) => buf.len() as u64,
            InputMode::File(f) => f.metadata().expect("file has no size").len(),
        }
    }
}

pub struct Prefix(pub String);

impl FromStr for Prefix {
    type Err = Error;

    fn from_str(prefix: &str) -> Result<Self, Self::Err> {
        const PREFIX_PATTERN: &str = r"^[a-zA-Z0-9._\-:|]{1,64}$";
        lazy_static! {
            static ref PREFIX_RE: Regex = Regex::new(PREFIX_PATTERN).unwrap();
        }
        let text = prefix.trim();
        if PREFIX_RE.is_match(text) {
            Ok(Self(text.into()))
        } else {
            Err(Error::MalformedArgument(
                "prefix",
                prefix.into(),
                PREFIX_PATTERN.into(),
            ))
        }
    }
}

pub struct Lifetime(pub String);

impl FromStr for Lifetime {
    type Err = Error;

    fn from_str(lifetime: &str) -> Result<Self, Self::Err> {
        const LIFETIME_PATTERN: &str = r"^\d+(h|m|s)$";
        lazy_static! {
            static ref LIFETIME_RE: Regex = Regex::new(LIFETIME_PATTERN).unwrap();
        }
        let text = lifetime.trim();
        if LIFETIME_RE.is_match(text) {
            Ok(Self(text.into()))
        } else {
            Err(Error::MalformedArgument(
                "lifetime",
                lifetime.into(),
                LIFETIME_PATTERN.into(),
            ))
        }
    }
}

pub enum ResponseFormat {
    TextJavascript,
    TextPlain,
}

impl Default for ResponseFormat {
    fn default() -> Self {
        Self::TextPlain
    }
}

impl FromStr for ResponseFormat {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "txt" => Ok(ResponseFormat::TextPlain),
            "text" => Ok(ResponseFormat::TextPlain),
            "text/plain" => Ok(ResponseFormat::TextPlain),
            "js" => Ok(ResponseFormat::TextJavascript),
            "json" => Ok(ResponseFormat::TextJavascript),
            "javascript" => Ok(ResponseFormat::TextJavascript),
            "text/javascript" => Ok(ResponseFormat::TextJavascript),
            _ => Err(Error::MalformedArgument(
                "response format",
                s.into(),
                "either of text/plain, text/javascript".into(),
            )),
        }
    }
}

impl ResponseFormat {
    pub fn to_api_name(&self) -> &'static str {
        match self {
            ResponseFormat::TextJavascript => "text/javascript",
            ResponseFormat::TextPlain => "text/plain",
        }
    }
}
