mod support;

use std::time::{Duration, Instant};

use support::{agent, agent_with_status};
use tmux_agents::config::Watch;
use tmux_agents::registry::Status;
use tmux_agents::tmux::PaneId;
use tmux_agents::watch::{Alert, Watcher};

fn at(t0: Instant, secs: u64) -> Instant {
    t0 + Duration::from_secs(secs)
}

fn pid(mut a: tmux_agents::agents::Agent, pid: i32) -> tmux_agents::agents::Agent {
    a.pid = pid;
    a
}

#[test]
fn the_first_observation_is_a_baseline_and_a_later_transition_to_blocked_alerts() {
    let t0 = Instant::now();
    let mut watcher = Watcher::default();
    let blocked_from_start = pid(agent_with_status("old", "%1", Status::Waiting), 1);
    let busy = pid(agent_with_status("dotfiles", "%2", Status::Busy), 2);

    let first = watcher.observe(&[blocked_from_start.clone(), busy.clone()], at(t0, 0));
    assert!(first.is_empty(), "{first:?}");

    let mut now_blocked = busy;
    now_blocked.status = Status::Waiting;
    now_blocked.waiting_for = Some("permission prompt".to_string());
    let second = watcher.observe(&[blocked_from_start, now_blocked], at(t0, 1));
    assert_eq!(
        second,
        vec![Alert {
            pane: PaneId("%2".to_string()),
            text: "◉ dotfiles: permission prompt".to_string(),
        }]
    );
}

#[test]
fn a_pid_that_stays_blocked_or_flaps_inside_the_quiet_window_does_not_realert() {
    let t0 = Instant::now();
    let mut watcher = Watcher::default();
    let busy = pid(agent_with_status("dotfiles", "%2", Status::Busy), 2);
    let blocked = pid(agent_with_status("dotfiles", "%2", Status::Waiting), 2);

    watcher.observe(std::slice::from_ref(&busy), at(t0, 0));
    assert_eq!(
        watcher
            .observe(std::slice::from_ref(&blocked), at(t0, 1))
            .len(),
        1
    );
    assert!(
        watcher
            .observe(std::slice::from_ref(&blocked), at(t0, 2))
            .is_empty()
    );
    assert!(
        watcher
            .observe(std::slice::from_ref(&busy), at(t0, 3))
            .is_empty()
    );
    assert!(
        watcher
            .observe(std::slice::from_ref(&blocked), at(t0, 3))
            .is_empty()
    );
    assert!(
        watcher
            .observe(std::slice::from_ref(&busy), at(t0, 4))
            .is_empty()
    );
    assert_eq!(watcher.observe(&[blocked], at(t0, 5)).len(), 1);
}

#[test]
fn a_new_pid_that_is_already_blocked_alerts_with_a_generic_reason() {
    let t0 = Instant::now();
    let mut watcher = Watcher::default();
    let existing = pid(agent("scintilla", "%1"), 1);
    let newcomer = pid(agent_with_status("api", "%9", Status::Waiting), 9);

    watcher.observe(std::slice::from_ref(&existing), at(t0, 0));
    let alerts = watcher.observe(&[existing, newcomer], at(t0, 1));

    assert_eq!(
        alerts,
        vec![Alert {
            pane: PaneId("%9".to_string()),
            text: "◉ api needs input".to_string(),
        }]
    );
}

#[test]
fn the_loop_notifies_every_client_not_already_looking_at_the_blocked_pane() {
    let tmux = support::FakeTmux::default();
    tmux.set_clients(&[("/dev/ttys003", "%1"), ("/dev/ttys009", "%2")]);
    let busy = pid(agent_with_status("dotfiles", "%2", Status::Busy), 2);
    let mut blocked = busy.clone();
    blocked.status = Status::Waiting;
    blocked.waiting_for = Some("permission prompt".to_string());
    let mut rounds = vec![vec![busy], vec![blocked]].into_iter();
    let t0 = Instant::now();
    let ticks = [at(t0, 0), at(t0, 1)].into_iter().map(Ok);

    tmux_agents::watch::run(
        &tmux,
        || Ok(rounds.next().unwrap_or_default()),
        ticks,
        &Watch::default(),
    )
    .unwrap();

    assert_eq!(
        tmux.messages(),
        vec![(
            "/dev/ttys003".to_string(),
            "◉ dotfiles: permission prompt".to_string(),
            4000
        )]
    );
}

#[test]
fn the_loop_can_notify_every_client_with_a_configured_duration_and_quiet_window() {
    let tmux = support::FakeTmux::default();
    tmux.set_clients(&[("/dev/ttys003", "%1"), ("/dev/ttys009", "%2")]);
    let busy = pid(agent_with_status("dotfiles", "%2", Status::Busy), 2);
    let mut blocked = busy.clone();
    blocked.status = Status::Waiting;
    let mut rounds = vec![
        vec![busy.clone()],
        vec![blocked.clone()],
        vec![busy],
        vec![blocked],
    ]
    .into_iter();
    let t0 = Instant::now();
    let ticks = [at(t0, 0), at(t0, 1), at(t0, 2), at(t0, 3)]
        .into_iter()
        .map(Ok);
    let config = Watch {
        quiet_ms: 10_000,
        display_ms: 1500,
        skip_active_client: false,
        ..Watch::default()
    };

    tmux_agents::watch::run(
        &tmux,
        || Ok(rounds.next().unwrap_or_default()),
        ticks,
        &config,
    )
    .unwrap();

    let text = "◉ dotfiles needs input".to_string();
    assert_eq!(
        tmux.messages(),
        vec![
            ("/dev/ttys003".to_string(), text.clone(), 1500),
            ("/dev/ttys009".to_string(), text, 1500),
        ]
    );
}

#[test]
fn the_pid_lock_is_claimed_unless_a_live_watcher_holds_it() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("watch.pid");
    let alive = |pid: i32| pid == 100;

    assert!(tmux_agents::watch::claim(&lock, 100, &alive).unwrap());
    assert_eq!(std::fs::read_to_string(&lock).unwrap().trim(), "100");
    assert!(!tmux_agents::watch::claim(&lock, 200, &alive).unwrap());
    assert_eq!(std::fs::read_to_string(&lock).unwrap().trim(), "100");

    std::fs::write(&lock, "999\n").unwrap();
    assert!(tmux_agents::watch::claim(&lock, 200, &alive).unwrap());
    assert_eq!(std::fs::read_to_string(&lock).unwrap().trim(), "200");

    std::fs::write(&lock, "garbage").unwrap();
    assert!(tmux_agents::watch::claim(&lock, 300, &alive).unwrap());
}
