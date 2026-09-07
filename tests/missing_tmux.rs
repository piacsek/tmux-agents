use std::process::Command;

fn binary_without_tmux(args: &[&str]) -> std::process::Output {
    let dir = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_tmux-agents"))
        .args(args)
        .env("PATH", dir.path())
        .env("HOME", dir.path())
        .env("XDG_CACHE_HOME", dir.path())
        .env("TMUX", format!("{}/sock,1,0", dir.path().display()))
        .output()
        .unwrap()
}

#[test]
fn a_missing_tmux_binary_is_named_on_stderr() {
    for args in [&[][..], &["status"][..], &["watch"][..]] {
        let out = binary_without_tmux(args);
        assert!(!out.status.success(), "{args:?} should fail");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("tmux not found on PATH"),
            "{args:?}: {stderr}"
        );
        assert!(stderr.contains("install tmux"), "{args:?}: {stderr}");
    }
}
