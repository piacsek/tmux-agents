use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::agents::Agent;
use crate::config::Watch;
use crate::state::State;
use crate::tmux::{Client, PaneId, Tmux, escape};

pub const QUIET: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub pane: PaneId,
    pub text: String,
}

pub struct Watcher {
    quiet: Duration,
    seen: Option<HashMap<i32, State>>,
    alerted_at: HashMap<i32, Instant>,
}

impl Default for Watcher {
    fn default() -> Self {
        Self::new(QUIET)
    }
}

impl Watcher {
    pub fn new(quiet: Duration) -> Self {
        Self {
            quiet,
            seen: None,
            alerted_at: HashMap::new(),
        }
    }

    pub fn observe(&mut self, agents: &[Agent], now: Instant) -> Vec<Alert> {
        let current: HashMap<i32, State> = agents
            .iter()
            .map(|agent| (agent.pid, State::from(agent.status)))
            .collect();
        let Some(previous) = self.seen.replace(current) else {
            return Vec::new();
        };
        self.alerted_at.retain(|pid, _| previous.contains_key(pid));
        let mut alerts = Vec::new();
        for agent in agents {
            let newly_blocked = State::from(agent.status) == State::Blocked
                && previous.get(&agent.pid) != Some(&State::Blocked);
            if newly_blocked && self.quiet_elapsed(agent.pid, now) {
                self.alerted_at.insert(agent.pid, now);
                alerts.push(alert(agent));
            }
        }
        alerts
    }

    fn quiet_elapsed(&self, pid: i32, now: Instant) -> bool {
        self.alerted_at
            .get(&pid)
            .is_none_or(|last| now.duration_since(*last) >= self.quiet)
    }
}

fn alert(agent: &Agent) -> Alert {
    let label = escape(&agent.label);
    let text = match &agent.waiting_for {
        Some(reason) => format!("◉ {label}: {}", escape(reason)),
        None => format!("◉ {label} needs input"),
    };
    Alert {
        pane: agent.pane.clone(),
        text,
    }
}

pub fn run<T, S>(
    tmux: &T,
    mut source: S,
    ticks: impl Iterator<Item = io::Result<Instant>>,
    config: &Watch,
) -> io::Result<()>
where
    T: Tmux,
    S: FnMut() -> io::Result<Vec<Agent>>,
{
    let mut watcher = Watcher::new(Duration::from_millis(config.quiet_ms));
    let duration = Duration::from_millis(config.display_ms);
    for tick in ticks {
        let now = tick?;
        let alerts = watcher.observe(&source()?, now);
        if alerts.is_empty() {
            continue;
        }
        let clients = tmux.clients()?;
        for alert in alerts {
            let looking_away =
                |client: &&Client| !config.skip_active_client || client.active_pane != alert.pane;
            for client in clients.iter().filter(looking_away) {
                tmux.display_message(&client.name, &alert.text, duration)?;
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum Claim {
    Acquired(Lock),
    HeldBy(i32),
}

#[derive(Debug)]
pub struct Lock(PathBuf);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn claim(lock: &Path, pid: i32, alive: &dyn Fn(i32) -> bool) -> io::Result<Claim> {
    let holder = fs::read_to_string(lock)
        .ok()
        .and_then(|text| text.trim().parse::<i32>().ok());
    if let Some(holder) = holder.filter(|holder| *holder != pid && alive(*holder)) {
        return Ok(Claim::HeldBy(holder));
    }
    if let Some(dir) = lock.parent() {
        crate::cached::private_dir(dir)?;
    }
    fs::write(lock, format!("{pid}\n"))?;
    Ok(Claim::Acquired(Lock(lock.to_path_buf())))
}
