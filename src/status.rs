use crate::agents::Agent;
use crate::state::State;

const ORDER: [State; 4] = [State::Blocked, State::Working, State::Idle, State::Unknown];
const CLAUDE_GLYPH: &str = "#[fg=white]\u{F0674}#[default]";
const NAMED_BLOCKED: usize = 2;

pub fn render(agents: &[Agent]) -> String {
    if agents.is_empty() {
        return format!("{CLAUDE_GLYPH}  #[dim]none#[default]");
    }
    let segments = ORDER
        .into_iter()
        .filter_map(|state| {
            let members: Vec<&Agent> = agents
                .iter()
                .filter(|agent| State::from(agent.status) == state)
                .collect();
            (!members.is_empty()).then(|| segment(state, &members))
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("{CLAUDE_GLYPH}  {segments}")
}

fn segment(state: State, members: &[&Agent]) -> String {
    let body = match state {
        State::Blocked => blocked_names(members),
        _ => members.len().to_string(),
    };
    format!("#[{}]{} {body}#[default]", state.tmux_style(), state.glyph())
}

fn blocked_names(members: &[&Agent]) -> String {
    let mut parts: Vec<String> = members
        .iter()
        .take(NAMED_BLOCKED)
        .map(|agent| agent.label.clone())
        .collect();
    if members.len() > NAMED_BLOCKED {
        parts.push(format!("+{}", members.len() - NAMED_BLOCKED));
    }
    parts.join(" ")
}
