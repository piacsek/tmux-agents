use std::fs;
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

struct Server {
    socket: String,
}

impl Server {
    fn start(name: &str) -> Self {
        Self::start_with_width(name, "80")
    }

    fn start_with_width(name: &str, width: &str) -> Self {
        let server = Self {
            socket: format!("tmux-agents-e2e-{name}"),
        };
        let status = server
            .tmux(&["new-session", "-d", "-s", "live", "-x", width, "-y", "12"])
            .status()
            .expect("tmux binary on PATH");
        assert!(status.success(), "could not start tmux e2e server");
        server
    }

    fn tmux(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("tmux");
        cmd.arg("-L").arg(&self.socket).args(args);
        cmd
    }

    fn pane_id(&self) -> String {
        let out = self
            .tmux(&["list-panes", "-a", "-F", "#{pane_id}"])
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    fn socket_path(&self) -> String {
        let out = self
            .tmux(&["display", "-p", "#{socket_path}"])
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    fn respawn(&self, env: &[(&str, &str)], command: &str) {
        let mut args = vec!["respawn-pane", "-k"];
        let pairs: Vec<String> = env.iter().map(|(k, v)| format!("{k}={v}")).collect();
        for pair in &pairs {
            args.push("-e");
            args.push(pair);
        }
        args.push(command);
        assert!(self.tmux(&args).status().unwrap().success());
    }

    fn wait_for_screen(&self, needle: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut screen = String::new();
        while Instant::now() < deadline {
            let out = self.tmux(&["capture-pane", "-p"]).output().unwrap();
            screen = String::from_utf8_lossy(&out.stdout).into_owned();
            if screen.contains(needle) {
                return screen;
            }
            sleep(Duration::from_millis(100));
        }
        screen
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.tmux(&["kill-server"]).status();
    }
}

fn fixture_home(server: &Server) -> tempfile::TempDir {
    fixture_home_for(&server.pane_id())
}

fn fixture_home_for(pane: &str) -> tempfile::TempDir {
    let home = tempfile::tempdir().unwrap();
    let sessions = home.path().join(".claude/sessions");
    fs::create_dir_all(&sessions).unwrap();
    let project = home.path().join("fixture-project");
    fs::create_dir_all(&project).unwrap();
    let pid = std::process::id();
    fs::write(
        sessions.join(format!("{pid}.json")),
        format!(
            r#"{{"pid":{pid},"cwd":"{}","kind":"interactive","status":"busy","tmux":"live:@0.{pane}"}}"#,
            project.display()
        ),
    )
    .unwrap();
    home
}

#[test]
#[ignore = "needs a tmux binary; run with --ignored"]
fn binary_lists_a_live_session_without_any_keypress() {
    let server = Server::start("tui");
    let home = fixture_home(&server);

    server.respawn(
        &[("HOME", home.path().to_str().unwrap())],
        env!("CARGO_BIN_EXE_tmux-agents"),
    );
    let screen = server.wait_for_screen("fixture-project");

    assert!(screen.contains("> ● working  fixture-project"), "{screen}");
}

#[test]
#[ignore = "needs a tmux binary; run with --ignored"]
fn status_subcommand_prints_tmux_markup_for_the_live_session() {
    let server = Server::start("status");
    let home = fixture_home(&server);

    let out = Command::new(env!("CARGO_BIN_EXE_tmux-agents"))
        .arg("status")
        .env("HOME", home.path())
        .env("TMUX", format!("{},0,0", server.socket_path()))
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "#[fg=white]󰙴#[default]  #[fg=yellow]● 1#[default]\n"
    );
}

#[test]
#[ignore = "needs a tmux binary; run with --ignored"]
fn status_subcommand_prints_none_when_no_session_is_registered() {
    let server = Server::start("none");
    let home = tempfile::tempdir().unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_tmux-agents"))
        .arg("status")
        .env("HOME", home.path())
        .env("TMUX", format!("{},0,0", server.socket_path()))
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "#[fg=white]\u{F0674}#[default]  #[dim]none#[default]\n"
    );
}

#[test]
#[ignore = "needs a tmux binary; run with --ignored"]
fn a_wide_binary_previews_the_registered_panes_content() {
    let server = Server::start_with_width("preview", "140");
    let picker_pane = server.pane_id();
    assert!(
        server
            .tmux(&["split-window", "-d", "-v", "echo PREVIEW-MARKER; sleep 30"])
            .status()
            .unwrap()
            .success()
    );
    let agent_pane = server
        .tmux(&["list-panes", "-F", "#{pane_id}"])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap()
        .lines()
        .find(|id| *id != picker_pane)
        .unwrap()
        .to_string();
    let home = fixture_home_for(&agent_pane);
    let config = home.path().join("config.toml");
    fs::write(&config, "[preview]\nenabled = true\n").unwrap();

    server.respawn(
        &[
            ("HOME", home.path().to_str().unwrap()),
            ("TMUX_AGENTS_CONFIG", config.to_str().unwrap()),
        ],
        env!("CARGO_BIN_EXE_tmux-agents"),
    );
    let screen = server.wait_for_screen("PREVIEW-MARKER");

    assert!(screen.contains("> ● working  fixture-project"), "{screen}");
    assert!(screen.contains("│ PREVIEW-MARKER"), "{screen}");
}

#[test]
#[ignore = "needs a tmux binary; run with --ignored"]
fn watch_holds_a_single_lock_and_exits_when_the_server_dies() {
    let server = Server::start("watch");
    let home = fixture_home(&server);
    let tmux_env = format!("{},0,0", server.socket_path());
    let spawn = || {
        Command::new(env!("CARGO_BIN_EXE_tmux-agents"))
            .arg("watch")
            .env("HOME", home.path())
            .env("XDG_CACHE_HOME", home.path().join("cache"))
            .env("TMUX", &tmux_env)
            .spawn()
            .unwrap()
    };

    let mut first = spawn();
    let lock_dir = home.path().join("cache/tmux-agents");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !lock_dir.exists() {
        sleep(Duration::from_millis(50));
    }
    let locks: Vec<_> = fs::read_dir(&lock_dir).unwrap().flatten().collect();
    assert_eq!(locks.len(), 1, "{locks:?}");
    assert_eq!(
        fs::read_to_string(locks[0].path()).unwrap().trim(),
        first.id().to_string()
    );

    let second = spawn().wait().unwrap();
    assert!(second.success());
    assert!(
        first.try_wait().unwrap().is_none(),
        "first watcher exited early"
    );

    drop(server);
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && first.try_wait().unwrap().is_none() {
        sleep(Duration::from_millis(100));
    }
    assert!(
        first.try_wait().unwrap().is_some(),
        "watcher outlived the server"
    );
}
