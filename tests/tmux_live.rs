use std::process::Command;

use tmux_agents::tmux::{CliTmux, Tmux};

const SOCKET: &str = "tmux-agents-test";

struct Server;

impl Server {
    fn start() -> Self {
        let status = Command::new("tmux")
            .args(["-L", SOCKET, "new-session", "-d", "-s", "live"])
            .status()
            .expect("tmux binary on PATH");
        assert!(status.success(), "could not start tmux test server");
        Self
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = Command::new("tmux")
            .args(["-L", SOCKET, "kill-server"])
            .status();
    }
}

#[test]
#[ignore = "needs a tmux binary; run with --ignored"]
fn list_panes_reads_a_real_tmux_server() {
    let _server = Server::start();

    let panes = CliTmux::with_socket(SOCKET).list_panes().unwrap();

    assert_eq!(panes.len(), 1, "{panes:?}");
    assert!(panes[0].id.0.starts_with('%'));
    assert_eq!(panes[0].session, "live");
    assert!(panes[0].window_id.starts_with('@'));
}
