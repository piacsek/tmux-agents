use std::cell::RefCell;
use std::io;
use std::time::{Duration, SystemTime};

use tmux_agents::cached::{Run, run};

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

struct Counting {
    calls: RefCell<usize>,
    output: String,
}

impl Counting {
    fn new(output: &str) -> Self {
        Self {
            calls: RefCell::new(0),
            output: output.to_string(),
        }
    }

    fn runner(&self) -> impl FnMut(&[String]) -> io::Result<Run> + '_ {
        move |_| {
            *self.calls.borrow_mut() += 1;
            Ok(Run {
                stdout: self.output.clone(),
                ok: true,
            })
        }
    }

    fn calls(&self) -> usize {
        *self.calls.borrow()
    }
}

const TTL: Duration = Duration::from_secs(5);

fn epoch(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

#[test]
fn a_cold_cache_runs_the_command_and_returns_its_output() {
    let dir = tempfile::tempdir().unwrap();
    let widget = Counting::new("dirty\n");

    let out = run(
        dir.path(),
        TTL,
        &argv(&["git", "status"]),
        epoch(1_000),
        &mut widget.runner(),
    )
    .unwrap();

    assert_eq!(out, "dirty");
    assert_eq!(widget.calls(), 1);
}

#[test]
fn a_second_call_within_the_ttl_serves_the_cache_without_running_again() {
    let dir = tempfile::tempdir().unwrap();
    let widget = Counting::new("dirty\n");
    let command = argv(&["git", "status"]);

    run(
        dir.path(),
        TTL,
        &command,
        epoch(1_000),
        &mut widget.runner(),
    )
    .unwrap();
    let out = run(
        dir.path(),
        TTL,
        &command,
        epoch(1_004),
        &mut widget.runner(),
    )
    .unwrap();

    assert_eq!(out, "dirty");
    assert_eq!(widget.calls(), 1);
}

#[test]
fn a_call_after_the_ttl_runs_again_and_refreshes_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let command = argv(&["git", "status"]);
    let first = Counting::new("dirty\n");
    let second = Counting::new("clean\n");

    run(dir.path(), TTL, &command, epoch(1_000), &mut first.runner()).unwrap();
    let out = run(
        dir.path(),
        TTL,
        &command,
        epoch(1_005),
        &mut second.runner(),
    )
    .unwrap();
    let again = run(dir.path(), TTL, &command, epoch(1_006), &mut first.runner()).unwrap();

    assert_eq!(out, "clean");
    assert_eq!(again, "clean");
    assert_eq!(second.calls(), 1);
    assert_eq!(first.calls(), 1);
}

#[test]
fn different_commands_do_not_share_a_cache_entry() {
    let dir = tempfile::tempdir().unwrap();
    let git = Counting::new("dirty\n");
    let kube = Counting::new("prod\n");

    run(
        dir.path(),
        TTL,
        &argv(&["git", "status"]),
        epoch(1_000),
        &mut git.runner(),
    )
    .unwrap();
    let out = run(
        dir.path(),
        TTL,
        &argv(&["kube_status"]),
        epoch(1_001),
        &mut kube.runner(),
    )
    .unwrap();

    assert_eq!(out, "prod");
    assert_eq!(kube.calls(), 1);
}

#[test]
fn the_binary_caches_a_real_command_under_xdg_cache_home() {
    let cache = tempfile::tempdir().unwrap();
    let counter = cache.path().join("counter");
    let script = format!("echo x >> {0}; wc -l < {0}", counter.display());
    let invoke = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_tmux-agents"))
            .args(["cached", "60", "--", "sh", "-c", &script])
            .env("XDG_CACHE_HOME", cache.path())
            .output()
            .unwrap()
    };

    let first = invoke();
    let second = invoke();

    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&first.stdout).trim(), "1");
    assert_eq!(String::from_utf8_lossy(&second.stdout).trim(), "1");
    assert!(cache.path().join("tmux-agents").is_dir());

    let failing = format!("{script}; exit 1");
    let invoke_failing = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_tmux-agents"))
            .args(["cached", "60", "--", "sh", "-c", &failing])
            .env("XDG_CACHE_HOME", cache.path())
            .output()
            .unwrap()
    };
    invoke_failing();
    let rerun = invoke_failing();
    assert!(rerun.status.success());
    assert_eq!(String::from_utf8_lossy(&rerun.stdout).trim(), "3");
}

#[test]
fn the_cache_dir_is_private_to_the_user() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("tmux-agents");
    std::fs::create_dir(&cache).unwrap();
    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o755)).unwrap();
    let widget = Counting::new("dirty\n");

    run(
        &cache,
        TTL,
        &argv(&["git", "status"]),
        epoch(1_000),
        &mut widget.runner(),
    )
    .unwrap();

    let mode = std::fs::metadata(&cache).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700, "{mode:o}");
}

#[test]
fn a_failed_command_is_returned_but_never_cached() {
    let dir = tempfile::tempdir().unwrap();
    let command = argv(&["git", "status"]);
    let mut calls = 0;
    let mut failing = |_: &[String]| {
        calls += 1;
        Ok(Run {
            stdout: "fatal: not a repo\n".to_string(),
            ok: false,
        })
    };

    let first = run(dir.path(), TTL, &command, epoch(1_000), &mut failing).unwrap();
    let second = run(dir.path(), TTL, &command, epoch(1_001), &mut failing).unwrap();

    assert_eq!(first, "fatal: not a repo");
    assert_eq!(second, "fatal: not a repo");
    assert_eq!(calls, 2);
}
