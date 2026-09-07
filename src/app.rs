use std::io;

use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

use crate::agents::Agent;
use crate::tmux::{PaneId, Tmux};
use crate::ui;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Key(KeyEvent),
    Tick,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Continue,
    Quit,
    Focus(PaneId),
    NewClaudePane,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Filter(String),
    Help,
}

pub struct App {
    pub agents: Vec<Agent>,
    pub list: ListState,
    pub mode: Mode,
    pending_g: bool,
}

impl App {
    pub fn new(agents: Vec<Agent>) -> Self {
        let list = ListState::default().with_selected(Some(0));
        Self {
            agents,
            list,
            mode: Mode::Normal,
            pending_g: false,
        }
    }

    pub fn refresh(&mut self, agents: Vec<Agent>) {
        let selected_pid = self.selected_agent().map(|agent| agent.pid);
        self.agents = merge_keeping_order(std::mem::take(&mut self.agents), agents);
        let index = selected_pid
            .and_then(|pid| self.visible().iter().position(|agent| agent.pid == pid))
            .unwrap_or(0);
        self.list.select(Some(index));
    }

    fn selected_agent(&self) -> Option<&Agent> {
        self.list
            .selected()
            .and_then(|i| self.visible().get(i).copied())
    }

    pub fn filter(&self) -> Option<&str> {
        match &self.mode {
            Mode::Filter(query) => Some(query),
            _ => None,
        }
    }

    pub fn visible(&self) -> Vec<&Agent> {
        visible_agents(&self.agents, self.filter())
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Action::Quit;
        }
        match self.mode {
            Mode::Help => {
                self.mode = Mode::Normal;
                Action::Continue
            }
            Mode::Filter(_) => self.handle_filter_key(key),
            Mode::Normal => self.handle_normal_key(key),
        }
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char(c) => self.edit_filter(|q| q.push(c)),
            KeyCode::Backspace => self.edit_filter(|q| {
                q.pop();
            }),
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                Action::Continue
            }
            _ => self.handle_normal_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> Action {
        let pending_g = std::mem::take(&mut self.pending_g);
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Action::Quit,
            KeyCode::Char('/') => self.mode = Mode::Filter(String::new()),
            KeyCode::Char('n') => return Action::NewClaudePane,
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('g') if pending_g => self.list.select_first(),
            KeyCode::Char('g') => self.pending_g = true,
            KeyCode::Enter => {
                if let Some(agent) = self.selected_agent() {
                    return Action::Focus(agent.pane.clone());
                }
            }
            KeyCode::Char(digit @ '1'..='9') => {
                let index = digit.to_digit(10).unwrap_or(1) as usize - 1;
                if let Some(agent) = self.visible().get(index) {
                    return Action::Focus(agent.pane.clone());
                }
            }
            KeyCode::Char('j') | KeyCode::Down => self.list.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.list.select_previous(),
            KeyCode::Char('G') => self.list.select_last(),
            _ => {}
        }
        Action::Continue
    }

    fn edit_filter(&mut self, edit: impl FnOnce(&mut String)) -> Action {
        if let Mode::Filter(query) = &mut self.mode {
            edit(query);
        }
        self.list.select_first();
        Action::Continue
    }
}

fn merge_keeping_order(current: Vec<Agent>, fresh: Vec<Agent>) -> Vec<Agent> {
    let mut fresh = fresh;
    let mut merged: Vec<Agent> = current
        .iter()
        .filter_map(|old| {
            let index = fresh.iter().position(|new| new.pid == old.pid)?;
            Some(fresh.remove(index))
        })
        .collect();
    merged.extend(fresh);
    merged
}

pub fn visible_agents<'a>(agents: &'a [Agent], filter: Option<&str>) -> Vec<&'a Agent> {
    let query = filter.unwrap_or("").to_lowercase();
    agents
        .iter()
        .filter(|agent| matches(agent, &query))
        .collect()
}

fn matches(agent: &Agent, query: &str) -> bool {
    let haystacks = [Some(agent.label.as_str()), agent.title.as_deref()];
    haystacks
        .into_iter()
        .flatten()
        .any(|text| text.to_lowercase().contains(query))
}

pub fn run<B, T, S>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    inputs: impl Iterator<Item = io::Result<Input>>,
    tmux: &T,
    mut source: S,
) -> io::Result<()>
where
    B: Backend,
    B::Error: Send + Sync + 'static,
    T: Tmux,
    S: FnMut() -> io::Result<Vec<Agent>>,
{
    let mut inputs = inputs;
    loop {
        terminal
            .draw(|frame| ui::draw(frame, app))
            .map_err(io::Error::other)?;
        let Some(input) = inputs.next() else {
            return Ok(());
        };
        match input? {
            Input::Tick => app.refresh(source()?),
            Input::Key(key) => match app.handle_key(key) {
                Action::Quit => return Ok(()),
                Action::Focus(pane) => return tmux.focus(&pane),
                Action::NewClaudePane => return tmux.new_claude_pane(),
                Action::Continue => {}
            },
        }
    }
}
