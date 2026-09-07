use crate::agents::Agent;
use crate::config::StatusLine;
use crate::state::State;
use crate::tmux::escape;

const ORDER: [State; 4] = [State::Blocked, State::Working, State::Idle, State::Unknown];

pub fn render(agents: &[Agent], config: &StatusLine) -> String {
    let body = if agents.is_empty() {
        "#[dim]none#[default]".to_string()
    } else {
        ORDER
            .into_iter()
            .filter_map(|state| {
                let members: Vec<&Agent> = agents
                    .iter()
                    .filter(|agent| State::from(agent.status) == state)
                    .collect();
                (!members.is_empty()).then(|| segment(state, &members, config.named_blocked))
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    if config.prefix.is_empty() {
        body
    } else {
        format!("{}  {body}", config.prefix)
    }
}

fn segment(state: State, members: &[&Agent], named_blocked: usize) -> String {
    let body = match state {
        State::Blocked if named_blocked > 0 => blocked_names(members, named_blocked),
        _ => members.len().to_string(),
    };
    format!(
        "#[{}]{} {body}#[default]",
        state.tmux_style(),
        state.glyph()
    )
}

fn blocked_names(members: &[&Agent], named: usize) -> String {
    let mut parts: Vec<String> = members
        .iter()
        .take(named)
        .map(|agent| escape(&agent.label))
        .collect();
    if members.len() > named {
        parts.push(format!("+{}", members.len() - named));
    }
    parts.join(" ")
}
