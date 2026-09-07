use std::process::Command;

fn binary(args: &[&str]) -> std::process::Output {
    let dir = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_tmux-agents"))
        .args(args)
        .env("HOME", dir.path())
        .output()
        .unwrap()
}

#[test]
fn help_flags_print_usage_on_stdout_and_exit_zero() {
    for flag in ["--help", "-h", "help"] {
        let out = binary(&[flag]);
        assert!(out.status.success(), "{flag}");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("usage: tmux-agents"), "{flag}: {stdout}");
        assert!(stdout.contains("status"), "{flag}: {stdout}");
        assert!(stdout.contains("cached <ttl-seconds>"), "{flag}: {stdout}");
    }
}

#[test]
fn version_flags_print_the_crate_version() {
    for flag in ["--version", "-V", "version"] {
        let out = binary(&[flag]);
        assert!(out.status.success(), "{flag}");
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            format!("tmux-agents {}", env!("CARGO_PKG_VERSION")),
            "{flag}"
        );
    }
}
