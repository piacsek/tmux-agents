use crate::agents::Agent;
use crate::state::State;

const ORDER: [State; 4] = [State::Blocked, State::Working, State::Idle, State::Unknown];
const CLAUDE_GLYPH: &str = "#[fg=white]\u{F0674}#[default]";

pub fn render(agents: &[Agent]) -> String {
    if agents.is_empty() {
        return format!("{CLAUDE_GLYPH}  #[dim]none#[default]");
    }
    let segments = ORDER
        .into_iter()
        .filter_map(|state| {
            let count = agents
                .iter()
                .filter(|agent| State::from(agent.status) == state)
                .count();
            (count > 0).then(|| segment(state, count))
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("{CLAUDE_GLYPH}  {segments}")
}

fn segment(state: State, count: usize) -> String {
    format!(
        "#[{}]{} {count}#[default]",
        state.tmux_style(),
        state.glyph()
    )
}
