use std::path::PathBuf;
use std::time::Duration;

use crate::registry::{Kind, SessionRecord, Status};
use crate::state::State;
use crate::tmux::{PaneId, PaneInfo, parse_pane_ref};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    pub pid: i32,
    pub label: String,
    pub status: Status,
    pub cwd: PathBuf,
    pub pane: PaneId,
    pub session: String,
    pub window_index: u32,
    pub title: Option<String>,
    pub status_age: Option<Duration>,
    pub waiting_for: Option<String>,
}

pub fn discover(
    records: Vec<SessionRecord>,
    panes: &[PaneInfo],
    alive: &dyn Fn(i32) -> bool,
    now_ms: u64,
) -> Vec<Agent> {
    let mut agents: Vec<Agent> = records
        .into_iter()
        .filter(|record| record.kind == Kind::Interactive && alive(record.pid))
        .filter_map(|record| {
            let pane_id = parse_pane_ref(record.tmux.as_deref()?)?;
            let pane = panes.iter().find(|p| p.id == pane_id)?;
            Some(Agent {
                pid: record.pid,
                label: basename(&record.cwd),
                status: record.status,
                cwd: record.cwd,
                pane: pane.id.clone(),
                session: pane.session.clone(),
                window_index: pane.window_index,
                title: summary(&pane.title),
                status_age: record
                    .status_updated_at
                    .map(|at| Duration::from_millis(now_ms.saturating_sub(at))),
                waiting_for: record.waiting_for,
            })
        })
        .collect();
    agents.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
    agents
}

fn sort_key(agent: &Agent) -> (State, &str, u32) {
    (
        State::from(agent.status),
        &agent.session,
        agent.window_index,
    )
}

fn summary(title: &str) -> Option<String> {
    let mut chars = title.chars();
    let glyph = chars.next()?;
    let space = chars.next()?;
    (!glyph.is_alphanumeric() && space == ' ').then(|| chars.as_str().to_string())
}

fn basename(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
