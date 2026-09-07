use std::fs;
use std::path::PathBuf;

use tmux_agents::registry::{Kind, SessionRecord, Status, load, sessions_dir};

const REAL_SAMPLE: &str = r#"{"pid":27756,"sessionId":"0f1e2d3c-4b5a-6978-8a9b-0c1d2e3f4a5b","cwd":"/Users/me/dotfiles","startedAt":1788798199062,"procStart":"Mon Sep  7 16:23:16 2026","version":"2.1.263","peerProtocol":1,"peerFeatures":["notify_idle","reply_across_default_dirs","artifact_yield"],"kind":"interactive","entrypoint":"cli","pidDomain":"darwin","tmux":"dotfiles:@7.%53","messagingSocketPath":"/tmp/cc-socks/27756.sock","name":"dotfiles-d8","nameSource":"derived","nameSince":1788798199063,"status":"busy","updatedAt":1788804089018,"statusUpdatedAt":1788804089018}"#;

#[test]
fn load_parses_a_real_session_file_ignoring_unknown_fields() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("27756.json"), REAL_SAMPLE).unwrap();

    let records = load(dir.path());

    assert_eq!(
        records,
        vec![SessionRecord {
            pid: 27756,
            cwd: PathBuf::from("/Users/me/dotfiles"),
            name: Some("dotfiles-d8".to_string()),
            kind: Kind::Interactive,
            status: Status::Busy,
            status_updated_at: Some(1788804089018),
            waiting_for: None,
            tmux: Some("dotfiles:@7.%53".to_string()),
        }]
    );
}

#[test]
fn load_skips_non_json_files_and_malformed_json() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("27756.json.tmp"), REAL_SAMPLE).unwrap();
    fs::write(
        dir.path().join("27756.abc.key"),
        r#"{"peerToken":"x","procStart":"y","pidDomain":"darwin"}"#,
    )
    .unwrap();
    fs::write(dir.path().join("99.json"), "{not json").unwrap();
    fs::write(dir.path().join("1.json"), REAL_SAMPLE).unwrap();

    let records = load(dir.path());

    assert_eq!(records.len(), 1);
}

#[test]
fn unknown_or_missing_status_and_kind_become_unknown() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("1.json"),
        r#"{"pid":1,"cwd":"/x","kind":"hologram","status":"meditating"}"#,
    )
    .unwrap();
    fs::write(dir.path().join("2.json"), r#"{"pid":2,"cwd":"/y"}"#).unwrap();

    let mut records = load(dir.path());
    records.sort_by_key(|r| r.pid);

    assert_eq!(records[0].kind, Kind::Unknown);
    assert_eq!(records[0].status, Status::Unknown);
    assert_eq!(records[1].kind, Kind::Unknown);
    assert_eq!(records[1].status, Status::Unknown);
}

#[test]
fn sessions_dir_honors_config_dir_and_defaults_to_home_dot_claude() {
    let home = PathBuf::from("/home/me");

    assert_eq!(
        sessions_dir(Some(PathBuf::from("/cfg")), &home),
        PathBuf::from("/cfg/sessions")
    );
    assert_eq!(
        sessions_dir(None, &home),
        PathBuf::from("/home/me/.claude/sessions")
    );
}

#[test]
fn waiting_for_is_read_when_present() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("1.json"),
        r#"{"pid":1,"cwd":"/x","kind":"interactive","status":"waiting","waitingFor":"permission prompt"}"#,
    )
    .unwrap();

    let records = load(dir.path());

    assert_eq!(records[0].status, Status::Waiting);
    assert_eq!(records[0].waiting_for.as_deref(), Some("permission prompt"));
}
