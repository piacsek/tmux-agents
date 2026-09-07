mod support;

use support::agent_with_status;
use tmux_agents::registry::Status;
use tmux_agents::status::render;

#[test]
fn one_working_agent_renders_a_yellow_dot_with_its_count() {
    let agents = vec![agent_with_status("a", "%1", Status::Busy)];

    assert_eq!(
        render(&agents),
        "#[fg=white]󰙴#[default]  #[fg=yellow]● 1#[default]"
    );
}

#[test]
fn states_are_ordered_blocked_working_idle_and_blocked_is_bold() {
    let agents = vec![
        agent_with_status("a", "%1", Status::Idle),
        agent_with_status("b", "%2", Status::Busy),
        agent_with_status("c", "%3", Status::Waiting),
        agent_with_status("d", "%4", Status::Shell),
        agent_with_status("e", "%5", Status::Idle),
    ];

    assert_eq!(
        render(&agents),
        "#[fg=white]󰙴#[default]  #[fg=red,bold]◉ 1#[default] #[fg=yellow]● 2#[default] #[dim]○ 2#[default]"
    );
}

#[test]
fn zero_counts_are_hidden_and_no_agents_prints_none() {
    let only_idle = vec![agent_with_status("a", "%1", Status::Idle)];

    assert_eq!(
        render(&only_idle),
        "#[fg=white]󰙴#[default]  #[dim]○ 1#[default]"
    );
    assert_eq!(render(&[]), "#[fg=white]󰙴#[default]  #[dim]none#[default]");
}

#[test]
fn unknown_status_is_a_grey_hollow_dot_after_idle() {
    let agents = vec![
        agent_with_status("a", "%1", Status::Unknown),
        agent_with_status("b", "%2", Status::Idle),
    ];

    assert_eq!(
        render(&agents),
        "#[fg=white]󰙴#[default]  #[dim]○ 1#[default] #[fg=brightblack]○ 1#[default]"
    );
}
