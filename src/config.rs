use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub picker: Picker,
    pub preview: Preview,
    pub status: StatusLine,
    pub watch: Watch,
    pub new_pane: NewPane,
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Picker {
    pub tick_ms: u64,
    pub stale_after_minutes: u64,
}

impl Default for Picker {
    fn default() -> Self {
        Self {
            tick_ms: 500,
            stale_after_minutes: 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preview {
    pub enabled: bool,
    pub min_width: u16,
    pub split_percent: u16,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            enabled: false,
            min_width: 100,
            split_percent: 50,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StatusLine {
    pub prefix: String,
    pub named_blocked: usize,
    pub max_label: usize,
}

impl Default for StatusLine {
    fn default() -> Self {
        Self {
            prefix: "#[fg=white]\u{F0674}#[default]".to_string(),
            named_blocked: 2,
            max_label: 16,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Watch {
    pub interval_ms: u64,
    pub quiet_ms: u64,
    pub display_ms: u64,
    pub skip_active_client: bool,
}

impl Default for Watch {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            quiet_ms: 3000,
            display_ms: 4000,
            skip_active_client: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NewPane {
    pub command: String,
    pub direction: Direction,
}

impl Default for NewPane {
    fn default() -> Self {
        Self {
            command: "\"${SHELL:-sh}\" -ic claude".to_string(),
            direction: Direction::Horizontal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug)]
pub struct ConfigError {
    pub path: PathBuf,
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for ConfigError {}

impl From<ConfigError> for io::Error {
    fn from(err: ConfigError) -> Self {
        io::Error::other(err.to_string())
    }
}

pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(err) => {
            return Err(ConfigError {
                path: path.to_path_buf(),
                message: err.to_string(),
            });
        }
    };
    let config: Config = toml::from_str(&text).map_err(|err| ConfigError {
        path: path.to_path_buf(),
        message: err.to_string().trim().to_string(),
    })?;
    config.validate().map_err(|message| ConfigError {
        path: path.to_path_buf(),
        message,
    })?;
    Ok(config)
}

impl Config {
    fn validate(&self) -> Result<(), String> {
        let positive = [
            ("picker.tick_ms", self.picker.tick_ms),
            ("watch.interval_ms", self.watch.interval_ms),
        ];
        for (key, value) in positive {
            if value == 0 {
                return Err(format!("{key} must be greater than 0"));
            }
        }
        if !(1..=100).contains(&self.preview.split_percent) {
            return Err("preview.split_percent must be between 1 and 100".to_string());
        }
        Ok(())
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    pub fn label_map(&self, home: &Path) -> HashMap<PathBuf, String> {
        self.labels
            .iter()
            .map(|(cwd, label)| (expand_tilde(cwd, home), label.clone()))
            .collect()
    }
}

fn expand_tilde(path: &str, home: &Path) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None if path == "~" => home.to_path_buf(),
        None => PathBuf::from(path),
    }
}

pub fn path(
    env_override: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
    home: &Path,
) -> PathBuf {
    env_override.unwrap_or_else(|| {
        xdg_config_home
            .unwrap_or_else(|| home.join(".config"))
            .join("tmux-agents")
            .join("config.toml")
    })
}
