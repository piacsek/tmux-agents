use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Busy,
    Shell,
    Idle,
    Waiting,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Interactive,
    Bg,
    Daemon,
    DaemonWorker,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub pid: i32,
    pub cwd: PathBuf,
    pub name: Option<String>,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default)]
    pub status: Status,
    pub status_updated_at: Option<u64>,
    pub waiting_for: Option<String>,
    pub tmux: Option<String>,
}

pub fn load(dir: &Path) -> Vec<SessionRecord> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter_map(|json| serde_json::from_str(&json).ok())
        .collect()
}

pub fn sessions_dir(config_dir: Option<PathBuf>, home: &Path) -> PathBuf {
    config_dir
        .unwrap_or_else(|| home.join(".claude"))
        .join("sessions")
}
