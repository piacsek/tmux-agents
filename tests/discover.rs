use std::path::PathBuf;
use std::time::Duration;

use tmux_agents::agents::{Agent, discover};
use tmux_agents::registry::{Kind, SessionRecord, Status};
use tmux_agents::tmux::{PaneId, PaneInfo};

fn record(pid: i32, cwd: &str, tmux: Option<&str>) -> SessionRecord {
    SessionRecord {
        pid,
        cwd: PathBuf::from(cwd),
        name: None,
        kind: Kind::Interactive,
        status: Status::Idle,
        status_updated_at: None,
        waiting_for: None,
        tmux: tmux.map(str::to_string),
    }
}

fn pane(id: &str, session: &str, window_index: u32, title: &str) -> PaneInfo {
    PaneInfo {
        id: PaneId(id.to_string()),
        session: session.to_string(),
        window_id: "@1".to_string(),
        window_index,
        current_path: PathBuf::from("/irrelevant"),
        title: title.to_string(),
    }
}

const NOW: u64 = 1_788_804_100_000;

fn alive(_: i32) -> bool {
    true
}

#[test]
fn interactive_record_with_live_pid_and_known_pane_becomes_an_agent() {
    let records = vec![record(42, "/home/me/dotfiles", Some("dotfiles:@7.%53"))];
    let panes = vec![pane("%53", "dotfiles", 2, "✳ Fix the picker")];

    let agents = discover(records, &panes, &alive, NOW);

    assert_eq!(
        agents,
        vec![Agent {
            pid: 42,
            label: "dotfiles".to_string(),
            status: Status::Idle,
            cwd: PathBuf::from("/home/me/dotfiles"),
            pane: PaneId("%53".to_string()),
            session: "dotfiles".to_string(),
            window_index: 2,
            title: Some("Fix the picker".to_string()),
            status_age: None,
            waiting_for: None,
        }]
    );
}

#[test]
fn non_interactive_records_are_dropped() {
    let mut bg = record(1, "/home/me/a", Some("s:@1.%1"));
    bg.kind = Kind::Bg;
    let mut unknown = record(2, "/home/me/b", Some("s:@1.%2"));
    unknown.kind = Kind::Unknown;
    let panes = vec![pane("%1", "s", 1, ""), pane("%2", "s", 1, "")];

    let agents = discover(vec![bg, unknown], &panes, &alive, NOW);

    assert!(agents.is_empty());
}

#[test]
fn records_without_tmux_field_are_dropped() {
    let panes = vec![pane("%1", "s", 1, "")];

    let agents = discover(vec![record(1, "/home/me/a", None)], &panes, &alive, NOW);

    assert!(agents.is_empty());
}

#[test]
fn records_whose_pane_is_not_in_this_server_are_dropped() {
    let panes = vec![pane("%1", "s", 1, "")];

    let agents = discover(
        vec![record(1, "/home/me/a", Some("s:@1.%99"))],
        &panes,
        &alive,
        NOW,
    );

    assert!(agents.is_empty());
}

#[test]
fn records_with_dead_pids_are_dropped() {
    let panes = vec![pane("%1", "s", 1, ""), pane("%2", "s", 2, "")];
    let records = vec![
        record(7, "/home/me/a", Some("s:@1.%1")),
        record(8, "/home/me/b", Some("s:@2.%2")),
    ];

    let agents = discover(records, &panes, &|pid| pid != 7, NOW);

    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].pid, 8);
}

#[test]
fn labels_stay_plain_basenames_even_when_they_collide() {
    let panes = vec![
        pane("%1", "work", 3, ""),
        pane("%2", "work", 3, ""),
        pane("%3", "home", 1, ""),
    ];
    let records = vec![
        record(1, "/a/webapp", Some("work:@1.%1")),
        record(2, "/b/webapp", Some("work:@1.%2")),
        record(3, "/c/dotfiles", Some("home:@3.%3")),
    ];

    let labels: Vec<String> = discover(records, &panes, &alive, NOW)
        .into_iter()
        .map(|a| a.label)
        .collect();

    assert_eq!(labels, vec!["dotfiles", "webapp", "webapp"]);
}

#[test]
fn agents_are_sorted_by_session_then_window_index() {
    let panes = vec![
        pane("%1", "work", 5, ""),
        pane("%2", "home", 2, ""),
        pane("%3", "work", 1, ""),
    ];
    let records = vec![
        record(1, "/a", Some("work:@1.%1")),
        record(2, "/b", Some("home:@2.%2")),
        record(3, "/c", Some("work:@3.%3")),
    ];

    let order: Vec<(String, u32)> = discover(records, &panes, &alive, NOW)
        .into_iter()
        .map(|a| (a.session, a.window_index))
        .collect();

    assert_eq!(
        order,
        vec![
            ("home".to_string(), 2),
            ("work".to_string(), 1),
            ("work".to_string(), 5)
        ]
    );
}

#[test]
fn agents_are_grouped_blocked_then_working_then_idle_before_session_order() {
    let panes = vec![
        pane("%1", "a", 1, ""),
        pane("%2", "a", 2, ""),
        pane("%3", "b", 1, ""),
        pane("%4", "b", 2, ""),
    ];
    let mut idle = record(1, "/idle", Some("a:@1.%1"));
    idle.status = Status::Idle;
    let mut working = record(2, "/working", Some("a:@2.%2"));
    working.status = Status::Busy;
    let mut blocked = record(3, "/blocked", Some("b:@3.%3"));
    blocked.status = Status::Waiting;
    let mut shell = record(4, "/shell", Some("b:@4.%4"));
    shell.status = Status::Shell;

    let labels: Vec<String> = discover(vec![idle, working, blocked, shell], &panes, &alive, NOW)
        .into_iter()
        .map(|a| a.label)
        .collect();

    assert_eq!(labels, vec!["blocked", "working", "shell", "idle"]);
}

#[test]
fn any_leading_glyph_is_stripped_from_the_title_but_plain_titles_are_ignored() {
    let panes = vec![
        pane("%1", "s", 1, "◐ Spinning summary"),
        pane("%2", "s", 2, "✳ Static summary"),
        pane("%3", "s", 3, "my-macbook.local"),
    ];
    let records = vec![
        record(1, "/a", Some("s:@1.%1")),
        record(2, "/b", Some("s:@2.%2")),
        record(3, "/c", Some("s:@3.%3")),
    ];

    let titles: Vec<Option<String>> = discover(records, &panes, &alive, NOW)
        .into_iter()
        .map(|a| a.title)
        .collect();

    assert_eq!(
        titles,
        vec![
            Some("Spinning summary".to_string()),
            Some("Static summary".to_string()),
            None
        ]
    );
}

#[test]
fn status_age_is_now_minus_status_updated_at() {
    let panes = vec![pane("%1", "s", 1, ""), pane("%2", "s", 2, "")];
    let mut dated = record(1, "/a", Some("s:@1.%1"));
    dated.status_updated_at = Some(NOW - 90_000);
    let undated = record(2, "/b", Some("s:@2.%2"));

    let ages: Vec<Option<Duration>> = discover(vec![dated, undated], &panes, &alive, NOW)
        .into_iter()
        .map(|a| a.status_age)
        .collect();

    assert_eq!(ages, vec![Some(Duration::from_secs(90)), None]);
}

#[test]
fn waiting_for_is_carried_onto_the_agent() {
    let panes = vec![pane("%1", "s", 1, "")];
    let mut blocked = record(1, "/a", Some("s:@1.%1"));
    blocked.status = Status::Waiting;
    blocked.waiting_for = Some("input needed".to_string());

    let agents = discover(vec![blocked], &panes, &alive, NOW);

    assert_eq!(agents[0].waiting_for.as_deref(), Some("input needed"));
}
