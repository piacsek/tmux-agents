use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ratatui::crossterm::event::{self, Event};
use tmux_agents::agents::{Agent, discover};
use tmux_agents::app::{App, Input, run};
use tmux_agents::cli::{self, Command};
use tmux_agents::config::{self, Config};
use tmux_agents::process::is_alive;
use tmux_agents::registry::{load, sessions_dir};
use tmux_agents::status::render;
use tmux_agents::tmux::{CliTmux, PaneInfo, Tmux};

const WATCH_INTERVAL: Duration = Duration::from_secs(1);

fn main() -> ExitCode {
    let command = match cli::parse(env::args().skip(1)) {
        Ok(command) => command,
        Err(err) => return fail(&err),
    };
    let config = match config::load(&config_path()) {
        Ok(config) => config,
        Err(err) => return fail(&err.to_string()),
    };
    let result = match command {
        Command::Tui => tui(&config),
        Command::Status => status(&config),
        Command::Watch => watch(&config),
        Command::Config => {
            print!("{}", config.to_toml());
            Ok(())
        }
        Command::Cached { ttl, command } => cached(ttl, &command),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => fail(&err.to_string()),
    }
}

fn config_path() -> PathBuf {
    config::path(
        env::var_os("TMUX_AGENTS_CONFIG").map(PathBuf::from),
        env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        &home(),
    )
}

fn home() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn fail(message: &str) -> ExitCode {
    eprintln!("tmux-agents: {message}");
    ExitCode::FAILURE
}

fn tui(config: &Config) -> std::io::Result<()> {
    let tmux = CliTmux::default();
    let mut source = agent_source(&tmux);
    let mut app = App::new(source()?, config.clone());
    let tick = Duration::from_millis(config.picker.tick_ms);
    ratatui::run(|terminal| {
        run(
            terminal,
            &mut app,
            std::iter::from_fn(|| Some(next_input(tick))),
            &tmux,
            &mut source,
        )
    })
}

fn status(config: &Config) -> std::io::Result<()> {
    let tmux = CliTmux::default();
    let agents = agent_source(&tmux)()?;
    println!("{}", render(&agents, &config.status));
    Ok(())
}

fn watch(_config: &Config) -> std::io::Result<()> {
    let lock = cache_dir().join(format!("watch-{}.pid", socket_key()));
    if !tmux_agents::watch::claim(&lock, std::process::id() as i32, &is_alive)? {
        return Ok(());
    }
    let tmux = CliTmux::default();
    let source = agent_source(&tmux);
    let ticks = std::iter::from_fn(|| {
        std::thread::sleep(WATCH_INTERVAL);
        Some(Ok(std::time::Instant::now()))
    });
    tmux_agents::watch::run(&tmux, source, ticks)
}

fn socket_key() -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let socket = env::var("TMUX").unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    socket
        .split(',')
        .next()
        .unwrap_or_default()
        .hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn cache_dir() -> PathBuf {
    env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_default()
        .join("tmux-agents")
}

fn cached(ttl: Duration, command: &[String]) -> std::io::Result<()> {
    let output = tmux_agents::cached::run(
        &cache_dir(),
        ttl,
        command,
        SystemTime::now(),
        &mut tmux_agents::cached::shell_out,
    )?;
    println!("{output}");
    Ok(())
}

fn agent_source(tmux: &CliTmux) -> impl FnMut() -> std::io::Result<Vec<Agent>> + '_ {
    let config_dir = env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from);
    let sessions = sessions_dir(config_dir, &home());
    move || {
        let panes: Vec<PaneInfo> = tmux.list_panes()?;
        Ok(discover(load(&sessions), &panes, &is_alive, now_ms()))
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn next_input(tick: Duration) -> std::io::Result<Input> {
    if !event::poll(tick)? {
        return Ok(Input::Tick);
    }
    match event::read()? {
        Event::Key(key) => Ok(Input::Key(key)),
        _ => Ok(Input::Tick),
    }
}
