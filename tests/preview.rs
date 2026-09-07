mod support;

use ratatui::crossterm::event::KeyCode;
use support::{Picker, agent, key};
use tmux_agents::config::Config;

fn previewing() -> Config {
    let mut config = Config::default();
    config.preview.enabled = true;
    config
}

fn wide() -> Picker {
    let picker = Picker::with_config(
        vec![agent("dotfiles", "%1"), agent("webapp", "%2")],
        120,
        8,
        previewing(),
    );
    picker.tmux.set_capture("%1", &["$ cargo test", "ok"]);
    picker
        .tmux
        .set_capture("%2", &["Allow Bash(rm -rf)?", "  1. Yes", "  2. No"]);
    picker
}

fn right_half(picker: &Picker) -> Vec<String> {
    picker
        .screen()
        .lines()
        .map(|line| {
            line.chars()
                .skip(60)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

#[test]
fn a_wide_popup_previews_the_selected_pane_on_the_right() {
    let mut picker = wide();

    picker.run(Vec::new()).unwrap();
    let right = right_half(&picker);
    assert!(
        right.iter().any(|l| l.contains("cargo test")),
        "{}",
        picker.screen()
    );
    assert!(
        picker.screen().contains("> 1 ○ idle     dotfiles"),
        "{}",
        picker.screen()
    );

    picker.run(vec![key(KeyCode::Char('j'))]).unwrap();
    let right = right_half(&picker);
    assert!(
        right.iter().any(|l| l.contains("Allow Bash(rm -rf)?")),
        "{}",
        picker.screen()
    );
    assert!(
        right.iter().any(|l| l.contains("2. No")),
        "{}",
        picker.screen()
    );
    assert!(
        !picker.screen().contains("cargo test"),
        "{}",
        picker.screen()
    );
}

#[test]
fn p_toggles_the_preview_off_and_back_on() {
    let mut picker = wide();

    picker.run(vec![key(KeyCode::Char('p'))]).unwrap();
    let screen = picker.screen();
    assert!(!screen.contains("cargo test"), "{screen}");
    assert!(!screen.contains('│'), "{screen}");
    assert!(screen.contains("> 1 ○ idle     dotfiles"), "{screen}");

    picker.run(vec![key(KeyCode::Char('p'))]).unwrap();
    assert!(
        picker.screen().contains("cargo test"),
        "{}",
        picker.screen()
    );
}

#[test]
fn a_narrow_popup_never_previews() {
    let mut picker = Picker::new(vec![agent("dotfiles", "%1")]);
    picker.tmux.set_capture("%1", &["$ cargo test"]);

    picker.run(Vec::new()).unwrap();

    let screen = picker.screen();
    assert!(!screen.contains("cargo test"), "{screen}");
    assert!(!screen.contains('│'), "{screen}");
}

#[test]
fn the_preview_shows_the_last_lines_and_drops_trailing_blanks() {
    let mut picker = Picker::with_config(vec![agent("dotfiles", "%1")], 120, 5, previewing());
    let lines: Vec<String> = (1..=10).map(|i| format!("line {i}")).collect();
    let mut with_blanks: Vec<&str> = lines.iter().map(String::as_str).collect();
    with_blanks.extend(["", "   ", ""]);
    picker.tmux.set_capture("%1", &with_blanks);

    picker.run(Vec::new()).unwrap();

    let right = right_half(&picker);
    assert_eq!(right[0], "│ line 7", "{}", picker.screen());
    assert_eq!(right[3], "│ line 10", "{}", picker.screen());
    assert!(!picker.screen().contains("line 6"), "{}", picker.screen());
}

#[test]
fn the_help_lists_the_preview_toggle() {
    let mut picker = Picker::with_size(vec![agent("dotfiles", "%1")], 120, 12);

    picker.run(vec![key(KeyCode::Char('?'))]).unwrap();

    assert!(
        picker.screen().contains("toggle preview"),
        "{}",
        picker.screen()
    );
}

#[test]
fn a_failed_capture_leaves_the_list_up_without_a_preview() {
    let mut picker = wide();
    picker.tmux.fail_captures();

    picker
        .run(vec![key(KeyCode::Char('j')), key(KeyCode::Enter)])
        .unwrap();

    let screen = picker.screen();
    assert!(screen.contains("> 2 ○ idle     webapp"), "{screen}");
    assert!(!screen.contains('│'), "{screen}");
    assert_eq!(picker.tmux.focused().len(), 1);
}

#[test]
fn the_preview_is_off_by_default_until_enabled_in_config_or_by_p() {
    let mut picker = Picker::with_size(vec![agent("dotfiles", "%1")], 120, 8);
    picker.tmux.set_capture("%1", &["$ cargo test"]);

    picker.run(Vec::new()).unwrap();
    assert!(
        !picker.screen().contains("cargo test"),
        "{}",
        picker.screen()
    );

    picker.run(vec![key(KeyCode::Char('p'))]).unwrap();
    assert!(
        picker.screen().contains("cargo test"),
        "{}",
        picker.screen()
    );
}

#[test]
fn min_width_and_split_percent_come_from_config() {
    let mut config = previewing();
    config.preview.min_width = 80;
    config.preview.split_percent = 30;
    let mut picker = Picker::with_config(vec![agent("dotfiles", "%1")], 90, 8, config);
    picker.tmux.set_capture("%1", &["$ cargo test"]);

    picker.run(Vec::new()).unwrap();

    let screen = picker.screen();
    let first = screen.lines().next().unwrap();
    assert_eq!(first.chars().nth(27), Some('│'), "{screen}");
    assert!(first.contains("cargo test"), "{screen}");
}
