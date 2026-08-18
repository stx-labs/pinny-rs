use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use thiserror::Error;

include!("include/macros.rs");

#[derive(Debug)]
pub struct Config {
    pub allowed_tags: Vec<String>,
}

impl Config {
    /// Get the config for the crate currently being expanded, cached per
    /// manifest directory. A single proc-macro server process (e.g.
    /// rust-analyzer's) expands macros for every crate in a workspace, so a
    /// process-wide singleton would leak the first crate's allowed tags into
    /// all the others.
    pub(crate) fn get_instance() -> Result<Arc<Self>, ConfigError> {
        static CACHE: OnceLock<Mutex<HashMap<String, Arc<Config>>>> =
            OnceLock::new();
        let manifest_dir = ConfigFactory::manifest_dir()?;
        let mut cache = CACHE
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(config) = cache.get(&manifest_dir) {
            debug!("Config cache hit for {manifest_dir}: {config:?}");
            return Ok(Arc::clone(config));
        }
        let config = Arc::new(ConfigFactory::create_for(&manifest_dir)?);
        debug!("Config initialized for {manifest_dir}: {config:?}");
        cache.insert(manifest_dir, Arc::clone(&config));
        Ok(config)
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to get CARGO_MANIFEST_DIR: {0}")]
    MissingEnvVar(#[from] std::env::VarError),

    #[error("Failed to read Cargo.toml: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Invalid TOML format: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Missing `allowed` tags in Cargo.toml")]
    MissingTags,

    #[error("`allowed` tags must be an array")]
    InvalidArrayFormat,

    #[error("Each tag should be a string")]
    InvalidStringType,

    #[error("Duplicated tag found: {0}")]
    DuplicateTag(String),

    #[error("Tag is not a valid rust identifier: {0}")]
    InvalidTagFormat(String),
}

impl PartialEq for ConfigError {
    fn eq(&self, other: &Self) -> bool {
        use ConfigError::{
            DuplicateTag, InvalidArrayFormat, InvalidStringType,
            InvalidTagFormat, MissingEnvVar, MissingTags, ParseError,
            ReadError,
        };
        match (self, other) {
            (MissingEnvVar(_), MissingEnvVar(_))
            | (ReadError(_), ReadError(_))
            | (ParseError(_), ParseError(_))
            | (MissingTags, MissingTags)
            | (InvalidArrayFormat, InvalidArrayFormat)
            | (InvalidStringType, InvalidStringType) => true,
            (DuplicateTag(s1), DuplicateTag(s2))
            | (InvalidTagFormat(s1), InvalidTagFormat(s2)) => s1 == s2,
            _ => false,
        }
    }
}

pub struct ConfigFactory {
    _prevent_instance: (),
}

impl ConfigFactory {
    /// Resolve the manifest directory of the crate being expanded.
    pub fn manifest_dir() -> Result<String, ConfigError> {
        // Introduced `PINNY_CARGO_MANIFEST_DIR` as on override for `CARGO_MANIFEST_DIR` to execute tests with trybuilder crate
        // This workaroubnd is suggested even here https://github.com/dtolnay/trybuild/issues/202
        Ok(std::env::var("PINNY_CARGO_MANIFEST_DIR")
            .or_else(|_| std::env::var("CARGO_MANIFEST_DIR"))?)
    }

    /// Create a config for the crate resolved from the environment.
    /// Only used by tests; `get_instance` resolves the manifest directory
    /// itself so it can key its cache by it.
    #[cfg(test)]
    pub fn create() -> Result<Config, ConfigError> {
        Self::create_for(&Self::manifest_dir()?)
    }

    pub fn create_for(manifest_dir: &str) -> Result<Config, ConfigError> {
        let cargo_path = std::path::Path::new(manifest_dir).join("Cargo.toml");
        let cargo_string = std::fs::read_to_string(cargo_path)?;
        let cargo_toml: toml::Value = cargo_string.parse()?;

        let allowed_tags = cargo_toml
            .get("package")
            .and_then(|pkg| pkg.get("metadata"))
            .and_then(|meta| meta.get("pinny"))
            .and_then(|tags| tags.get("allowed"))
            .ok_or(ConfigError::MissingTags)?
            .as_array()
            .ok_or(ConfigError::InvalidArrayFormat)?
            .iter()
            .map(|mode| {
                mode.as_str()
                    .map(std::string::ToString::to_string)
                    .ok_or(ConfigError::InvalidStringType)
            })
            .collect::<Result<Vec<String>, ConfigError>>()?;

        //check for duplication and format (as rust identifier)
        let mut seen = HashSet::new();
        let re = Regex::new(r"^[a-z][a-z0-9_]*$").unwrap();
        for tag in &allowed_tags {
            if !seen.insert(tag) {
                return Err(ConfigError::DuplicateTag(tag.clone()));
            }
            if !re.is_match(tag) {
                return Err(ConfigError::InvalidTagFormat(tag.clone()));
            }
        }
        Ok(Config { allowed_tags })
    }
}
