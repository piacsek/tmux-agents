use ratatui::style::{Color, Modifier, Style};

use crate::registry::Status;

pub const WORD_WIDTH: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    Blocked,
    Working,
    Idle,
    Unknown,
}

impl From<Status> for State {
    fn from(status: Status) -> Self {
        match status {
            Status::Busy | Status::Shell => State::Working,
            Status::Waiting => State::Blocked,
            Status::Idle => State::Idle,
            Status::Unknown => State::Unknown,
        }
    }
}

impl State {
    pub fn word(self) -> &'static str {
        match self {
            State::Working => "working",
            State::Blocked => "blocked",
            State::Idle => "idle",
            State::Unknown => "?",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            State::Blocked => "◉",
            State::Working => "●",
            State::Idle | State::Unknown => "○",
        }
    }

    pub fn style(self) -> Style {
        match self {
            State::Working => Style::default().fg(Color::Yellow),
            State::Blocked => Style::default().fg(Color::Red),
            State::Idle => Style::default().add_modifier(Modifier::DIM),
            State::Unknown => Style::default().fg(Color::DarkGray),
        }
    }

    pub fn tmux_style(self) -> &'static str {
        match self {
            State::Working => "fg=yellow",
            State::Blocked => "fg=red,bold",
            State::Idle => "dim",
            State::Unknown => "fg=brightblack",
        }
    }
}
