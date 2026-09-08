use tmux_agents::config::{Config, load};

#[test]
fn a_missing_file_yields_defaults_with_the_preview_off() {
    let dir = tempfile::tempdir().unwrap();

    let config = load(&dir.path().join("config.toml")).unwrap();

    assert_eq!(config, Config::default());
    assert!(!config.preview.enabled);
    assert_eq!(config.preview.min_width, 100);
    assert_eq!(config.preview.split_percent, 50);
    assert_eq!(config.picker.tick_ms, 500);
    assert_eq!(config.picker.stale_after_minutes, 30);
    assert_eq!(config.status.named_blocked, 2);
    assert_eq!(config.status.max_label, 16);
    assert_eq!(config.status.prefix, "#[fg=white]\u{F0674}#[default]");
    assert_eq!(config.watch.interval_ms, 1000);
    assert_eq!(config.watch.quiet_ms, 3000);
    assert_eq!(config.watch.display_ms, 4000);
    assert!(config.watch.skip_active_client);
    assert_eq!(config.new_pane.command, "\"${SHELL:-sh}\" -ic claude");
    assert_eq!(
        config.new_pane.direction,
        tmux_agents::config::Direction::Horizontal
    );
    assert!(config.labels.is_empty());
}

fn write(dir: &tempfile::TempDir, text: &str) -> std::path::PathBuf {
    let path = dir.path().join("config.toml");
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn a_partial_file_overrides_only_the_keys_it_names() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(
        &dir,
        "[preview]\nenabled = true\n\n[labels]\n\"/a/b\" = \"bee\"\n",
    );

    let config = load(&path).unwrap();

    assert!(config.preview.enabled);
    assert_eq!(config.preview.min_width, 100);
    assert_eq!(config.picker.tick_ms, 500);
    assert_eq!(config.labels.get("/a/b").map(String::as_str), Some("bee"));
}

#[test]
fn an_unknown_key_or_bad_value_fails_naming_the_file() {
    let dir = tempfile::tempdir().unwrap();

    let err = load(&write(&dir, "[preview]\nenable = true\n")).unwrap_err();
    let text = err.to_string();
    assert!(text.contains("config.toml"), "{text}");
    assert!(text.contains("enable"), "{text}");

    let err = load(&write(&dir, "[new_pane]\ndirection = \"sideways\"\n")).unwrap_err();
    assert!(err.to_string().contains("sideways"), "{err}");

    let err = load(&write(&dir, "not toml at all [[[")).unwrap_err();
    assert!(err.to_string().contains("config.toml"), "{err}");
}

#[test]
fn the_effective_config_prints_as_toml_that_loads_back_identically() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.preview.enabled = true;
    config.new_pane.direction = tmux_agents::config::Direction::Vertical;
    config.labels.insert("/x".to_string(), "ex".to_string());

    let text = config.to_toml();
    let reloaded = load(&write(&dir, &text)).unwrap();

    assert_eq!(reloaded, config);
    assert!(text.contains("[preview]"), "{text}");
    assert!(text.contains("enabled = true"), "{text}");
    assert!(text.contains("direction = \"vertical\""), "{text}");
}

#[test]
fn the_path_prefers_the_env_override_then_xdg_then_home() {
    use std::path::{Path, PathBuf};
    use tmux_agents::config::path;
    let home = Path::new("/home/me");

    assert_eq!(
        path(
            Some(PathBuf::from("/etc/ta.toml")),
            Some(PathBuf::from("/xdg")),
            home
        ),
        PathBuf::from("/etc/ta.toml")
    );
    assert_eq!(
        path(None, Some(PathBuf::from("/xdg")), home),
        PathBuf::from("/xdg/tmux-agents/config.toml")
    );
    assert_eq!(
        path(None, None, home),
        PathBuf::from("/home/me/.config/tmux-agents/config.toml")
    );
}

fn binary(config: &std::path::Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_tmux-agents"))
        .args(args)
        .env("TMUX_AGENTS_CONFIG", config)
        .output()
        .unwrap()
}

#[test]
fn the_binary_prints_the_effective_config_and_rejects_a_broken_file() {
    let dir = tempfile::tempdir().unwrap();

    let good = write(&dir, "[preview]\nmin_width = 80\n");
    let out = binary(&good, &["config"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let printed: Config = toml::from_str(&String::from_utf8_lossy(&out.stdout)).unwrap();
    assert_eq!(printed.preview.min_width, 80);
    assert!(!printed.preview.enabled);

    let bad = write(&dir, "[preview]\nenable = true\n");
    let out = binary(&bad, &["config"]);
    assert!(!out.status.success(), "config should fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("config.toml"), "{stderr}");
    assert!(stderr.contains("enable"), "{stderr}");
}

#[test]
fn the_status_line_flags_a_broken_config_instead_of_going_blank() {
    let dir = tempfile::tempdir().unwrap();
    let bad = write(&dir, "[preview]\nenable = true\n");

    let out = binary(&bad, &["status"]);

    assert!(out.status.success(), "status should still print");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "#[fg=red,bold]⚠ config#[default]\n"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("config.toml"), "{stderr}");
    assert!(stderr.contains("enable"), "{stderr}");
}

#[test]
fn label_keys_expand_a_leading_tilde_against_home() {
    let mut config = Config::default();
    config
        .labels
        .insert("~/projects/x".to_string(), "ex".to_string());
    config
        .labels
        .insert("/abs/y".to_string(), "why".to_string());

    let map = config.label_map(std::path::Path::new("/home/me"));

    assert_eq!(
        map.get(std::path::Path::new("/home/me/projects/x"))
            .map(String::as_str),
        Some("ex")
    );
    assert_eq!(
        map.get(std::path::Path::new("/abs/y")).map(String::as_str),
        Some("why")
    );
}

#[test]
fn out_of_range_values_fail_naming_the_key() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        ("[picker]\ntick_ms = 0\n", "tick_ms"),
        ("[watch]\ninterval_ms = 0\n", "interval_ms"),
        ("[preview]\nsplit_percent = 150\n", "split_percent"),
        ("[preview]\nsplit_percent = 0\n", "split_percent"),
    ];

    for (text, key) in cases {
        let err = load(&write(&dir, text)).unwrap_err().to_string();
        assert!(err.contains("config.toml"), "{text}: {err}");
        assert!(err.contains(key), "{text}: {err}");
    }
    assert!(load(&write(&dir, "[preview]\nsplit_percent = 100\n")).is_ok());
}

#[test]
fn a_config_error_summarises_to_one_line_with_the_file_name_and_reason() {
    let dir = tempfile::tempdir().unwrap();

    let err = load(&write(&dir, "[preview]\nenable = true\n")).unwrap_err();
    let summary = err.summary();

    assert!(summary.starts_with("config.toml: "), "{summary}");
    assert!(summary.contains("enable"), "{summary}");
    assert!(!summary.contains('\n'), "{summary}");
    assert!(!summary.contains(dir.path().to_str().unwrap()), "{summary}");
}
