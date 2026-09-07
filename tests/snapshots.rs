mod support;

use ratatui::crossterm::event::KeyCode;
use support::{Picker, agent, agent_with_status, key};
use tmux_agents::registry::Status;

fn sample() -> Vec<tmux_agents::agents::Agent> {
    let mut blocked = agent_with_status("webapp", "%3", Status::Waiting);
    blocked.title = Some("Remove PR comments".to_string());
    let mut working = agent_with_status("dotfiles", "%53", Status::Busy);
    working.title = Some("Tmux Claude Code session picker".to_string());
    let idle = agent_with_status("scintilla.nvim", "%17", Status::Idle);
    vec![blocked, working, idle]
}

#[test]
fn list_layout() {
    let mut picker = Picker::new(sample());
    picker.run(vec![key(KeyCode::Char('j'))]).unwrap();
    insta::assert_snapshot!(picker.screen());
}

#[test]
fn filter_layout() {
    let mut picker = Picker::new(sample());
    picker
        .run(vec![key(KeyCode::Char('/')), key(KeyCode::Char('s'))])
        .unwrap();
    insta::assert_snapshot!(picker.screen());
}

#[test]
fn help_layout() {
    let mut picker = Picker::new(sample());
    picker.run(vec![key(KeyCode::Char('?'))]).unwrap();
    insta::assert_snapshot!(picker.screen());
}

#[test]
fn empty_layout() {
    let mut picker = Picker::new(vec![agent("unused", "%1")]);
    picker.next_refresh_returns(Vec::new());
    picker.run(vec![support::tick()]).unwrap();
    insta::assert_snapshot!(picker.screen());
}
