use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Default engine name (e.g. "say", "edge", "openai").
    #[serde(default = "default_engine")]
    pub engine: String,

    /// Default voice for each engine. Key = engine name, value = voice id/name.
    #[serde(default)]
    pub voices: std::collections::HashMap<String, String>,

    /// Default speech rate multiplier (1.0 = normal).
    #[serde(default)]
    pub rate: Option<f32>,
}

fn default_engine() -> String {
    "say".to_string()
}

#[derive(Debug)]
pub struct ConfigLoadError {
    path: PathBuf,
    kind: ConfigLoadErrorKind,
}

#[derive(Debug)]
enum ConfigLoadErrorKind {
    Read(std::io::Error),
    Parse(toml::de::Error),
}

#[derive(Debug)]
pub struct ConfigSaveError {
    path: PathBuf,
    kind: ConfigSaveErrorKind,
}

#[derive(Debug)]
enum ConfigSaveErrorKind {
    CreateDir(std::io::Error),
    Serialize(toml::ser::Error),
    Write(std::io::Error),
}

impl Default for Config {
    fn default() -> Self {
        Self {
            engine: default_engine(),
            voices: std::collections::HashMap::new(),
            rate: None,
        }
    }
}

impl Config {
    /// Config file path: platform config dir + `/agent-speak/config.toml`
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("agent-speak")
            .join("config.toml")
    }

    /// Load config from disk, or return defaults when the file is missing.
    pub fn load() -> Result<Self, ConfigLoadError> {
        Self::load_from_path(&Self::path())
    }

    /// Save config to disk.
    pub fn save(&self) -> Result<(), ConfigSaveError> {
        self.save_to_path(&Self::path())
    }

    fn load_from_path(path: &Path) -> Result<Self, ConfigLoadError> {
        match fs::read_to_string(path) {
            Ok(contents) => toml::from_str(&contents)
                .map_err(|source| ConfigLoadError::parse(path.to_path_buf(), source)),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(ConfigLoadError::read(path.to_path_buf(), source)),
        }
    }

    fn save_to_path(&self, path: &Path) -> Result<(), ConfigSaveError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|source| ConfigSaveError::create_dir(path.to_path_buf(), source))?;
        }

        let contents = toml::to_string_pretty(self)
            .map_err(|source| ConfigSaveError::serialize(path.to_path_buf(), source))?;
        fs::write(path, contents)
            .map_err(|source| ConfigSaveError::write(path.to_path_buf(), source))?;
        Ok(())
    }
}

impl ConfigLoadError {
    fn read(path: PathBuf, source: std::io::Error) -> Self {
        Self {
            path,
            kind: ConfigLoadErrorKind::Read(source),
        }
    }

    fn parse(path: PathBuf, source: toml::de::Error) -> Self {
        Self {
            path,
            kind: ConfigLoadErrorKind::Parse(source),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl std::fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ConfigLoadErrorKind::Read(source) => {
                write!(f, "failed to read {}: {source}", self.path.display())
            }
            ConfigLoadErrorKind::Parse(source) => {
                write!(f, "failed to parse {}: {source}", self.path.display())
            }
        }
    }
}

impl std::error::Error for ConfigLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ConfigLoadErrorKind::Read(source) => Some(source),
            ConfigLoadErrorKind::Parse(source) => Some(source),
        }
    }
}

impl ConfigSaveError {
    fn create_dir(path: PathBuf, source: std::io::Error) -> Self {
        Self {
            path,
            kind: ConfigSaveErrorKind::CreateDir(source),
        }
    }

    fn serialize(path: PathBuf, source: toml::ser::Error) -> Self {
        Self {
            path,
            kind: ConfigSaveErrorKind::Serialize(source),
        }
    }

    fn write(path: PathBuf, source: std::io::Error) -> Self {
        Self {
            path,
            kind: ConfigSaveErrorKind::Write(source),
        }
    }
}

impl std::fmt::Display for ConfigSaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ConfigSaveErrorKind::CreateDir(source) => {
                write!(
                    f,
                    "failed to create config directory for {}: {source}",
                    self.path.display()
                )
            }
            ConfigSaveErrorKind::Serialize(source) => {
                write!(f, "failed to serialize {}: {source}", self.path.display())
            }
            ConfigSaveErrorKind::Write(source) => {
                write!(f, "failed to write {}: {source}", self.path.display())
            }
        }
    }
}

impl std::error::Error for ConfigSaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ConfigSaveErrorKind::CreateDir(source) => Some(source),
            ConfigSaveErrorKind::Serialize(source) => Some(source),
            ConfigSaveErrorKind::Write(source) => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use tempfile::tempdir;

    use super::Config;

    #[test]
    fn default_config_values_are_stable() {
        let config = Config::default();
        assert_eq!(config.engine, "say");
        assert!(config.voices.is_empty());
        assert_eq!(config.rate, None);
    }

    #[test]
    fn load_reads_valid_toml() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(
            &path,
            r#"
engine = "edge"
rate = 1.25

[voices]
say = "Kyoko"
"#,
        )
        .unwrap();

        let config = Config::load_from_path(&path).unwrap();
        assert_eq!(config.engine, "edge");
        assert_eq!(config.rate, Some(1.25));
        assert_eq!(config.voices.get("say"), Some(&"Kyoko".to_string()));
    }

    #[test]
    fn load_returns_parse_error_for_invalid_toml() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(&path, "engine = [").unwrap();

        let error = Config::load_from_path(&path).unwrap_err();
        assert_eq!(error.path(), path.as_path());
        assert!(
            error
                .to_string()
                .contains(&format!("failed to parse {}", path.display()))
        );
    }

    #[test]
    fn save_round_trips_through_disk() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("nested").join("config.toml");
        let mut voices = HashMap::new();
        voices.insert("say".to_string(), "Samantha".to_string());
        let config = Config {
            engine: "say".to_string(),
            voices,
            rate: Some(0.9),
        };

        config.save_to_path(&path).unwrap();

        let loaded = Config::load_from_path(&path).unwrap();
        assert_eq!(loaded.engine, "say");
        assert_eq!(loaded.rate, Some(0.9));
        assert_eq!(loaded.voices.get("say"), Some(&"Samantha".to_string()));
    }
}
