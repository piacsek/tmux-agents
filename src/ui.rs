use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};

use crate::agents::Agent;
use crate::app::{App, Mode, visible_agents};
use crate::state::{State, WORD_WIDTH};

const HELP_HINT: &str = "press ? for keybindings";
const HIGHLIGHT: &str = "> ";

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [body, footer] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
    if app.mode == Mode::Help {
        draw_help(frame, body);
    } else if app.agents.is_empty() {
        draw_empty(frame, body);
    } else {
        draw_list(frame, body, app);
    }
    draw_footer(frame, footer, app);
}

fn draw_empty(frame: &mut Frame, area: Rect) {
    let text = vec![
        Line::from("No Claude Code sessions in this tmux server"),
        Line::from(""),
        Line::from(Span::styled("n new Claude pane  q close", dim())),
    ];
    frame.render_widget(Paragraph::new(text), area);
}

const KEYS: [(&str, &str); 8] = [
    ("j/k ↓/↑", "move"),
    ("1-9", "focus row directly"),
    ("gg / G", "first / last"),
    ("/", "filter, Esc clears"),
    ("Enter", "focus pane"),
    ("n", "new Claude pane"),
    ("q / Esc", "close"),
    ("?", "toggle this help"),
];

fn draw_help(frame: &mut Frame, area: Rect) {
    let width = KEYS
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0);
    let lines: Vec<Line> = KEYS
        .iter()
        .map(|(key, what)| {
            Line::from(vec![
                Span::styled(
                    format!("{key:<width$}"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(*what, dim()),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let visible = visible_agents(&app.agents, app.filter());
    let label_width = visible
        .iter()
        .map(|agent| agent.label.chars().count())
        .max()
        .unwrap_or(0);
    let row_width = area.width.saturating_sub(HIGHLIGHT.chars().count() as u16) as usize;
    let items: Vec<ListItem> = visible
        .into_iter()
        .map(|agent| row(agent, label_width, row_width))
        .collect();
    let list = List::new(items).highlight_symbol(HIGHLIGHT);
    frame.render_stateful_widget(list, area, &mut app.list);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let hint_width = HELP_HINT.chars().count() as u16;
    let [left, right] =
        Layout::horizontal([Constraint::Min(1), Constraint::Length(hint_width)]).areas(area);
    match &app.mode {
        Mode::Help => {
            let version = format!("tmux-agents v{}", env!("CARGO_PKG_VERSION"));
            frame.render_widget(Paragraph::new(Span::styled(version, dim())), left);
        }
        Mode::Filter(query) => frame.render_widget(Paragraph::new(format!("/{query}")), left),
        Mode::Normal => {}
    }
    frame.render_widget(
        Paragraph::new(Span::styled(HELP_HINT, dim())).right_aligned(),
        right,
    );
}

fn row(agent: &Agent, label_width: usize, row_width: usize) -> ListItem<'_> {
    let state = State::from(agent.status);
    let mut spans = vec![
        Span::styled(state.glyph(), state.style()),
        Span::raw(" "),
        Span::styled(format!("{:<WORD_WIDTH$}", state.word()), dim()),
        Span::raw("  "),
        Span::styled(
            format!("{:<label_width$}", agent.label),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
    ];
    let right = agent.status_age.map(format_age).unwrap_or_default();
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let room = row_width.saturating_sub(used + right.chars().count() + 2);
    spans.extend(middle(agent, room));
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let gap = row_width
        .saturating_sub(used + right.chars().count())
        .max(1);
    spans.push(Span::raw(" ".repeat(gap)));
    spans.push(Span::styled(right, dim()));
    ListItem::new(Line::from(spans))
}

fn middle(agent: &Agent, room: usize) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let mut room = room;
    if let Some(reason) = &agent.waiting_for {
        let text = truncate(reason, room);
        room = room.saturating_sub(text.chars().count());
        spans.push(Span::styled(text, Style::default().fg(Color::Red)));
        if agent.title.is_some() && room >= 3 {
            spans.push(Span::styled(" · ", dim()));
            room -= 3;
        }
    }
    let detail = match &agent.title {
        Some(title) => title.clone(),
        None if agent.waiting_for.is_none() => tilde(&agent.cwd),
        None => return spans,
    };
    spans.push(Span::styled(truncate(&detail, room), dim()));
    spans
}

fn truncate(text: &str, room: usize) -> String {
    let len = text.chars().count();
    if len <= room {
        return text.to_string();
    }
    if room == 0 {
        return String::new();
    }
    let mut out: String = text.chars().take(room - 1).collect();
    out.push('…');
    out
}

fn tilde(path: &std::path::Path) -> String {
    let parts: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    match parts.as_slice() {
        [root, home, _user, rest @ ..] if root == "/" && (home == "home" || home == "Users") => {
            let mut out = String::from("~");
            for part in rest {
                out.push('/');
                out.push_str(part);
            }
            out
        }
        _ => path.display().to_string(),
    }
}

fn format_age(age: std::time::Duration) -> String {
    let secs = age.as_secs();
    match secs {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m", s / 60),
        s if s < 86_400 => format!("{}h", s / 3600),
        s => format!("{}d", s / 86_400),
    }
}

fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn age_is_rendered_in_the_largest_whole_unit() {
        assert_eq!(format_age(Duration::from_secs(45)), "45s");
        assert_eq!(format_age(Duration::from_secs(90)), "1m");
        assert_eq!(format_age(Duration::from_secs(3 * 3600 + 59 * 60)), "3h");
        assert_eq!(format_age(Duration::from_secs(2 * 86_400)), "2d");
    }
}
