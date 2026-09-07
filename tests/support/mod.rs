#![allow(dead_code)]

use std::cell::RefCell;
use std::io;
use std::rc::Rc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tmux_agents::agents::Agent;
use tmux_agents::app::{App, Input, run};
use tmux_agents::registry::Status;
use tmux_agents::tmux::{PaneId, PaneInfo, Tmux};

#[derive(Default)]
pub struct FakeTmux {
    focused: RefCell<Vec<PaneId>>,
    new_panes: RefCell<usize>,
}

impl FakeTmux {
    pub fn focused(&self) -> Vec<PaneId> {
        self.focused.borrow().clone()
    }

    pub fn new_panes_requested(&self) -> usize {
        *self.new_panes.borrow()
    }
}

impl Tmux for FakeTmux {
    fn list_panes(&self) -> io::Result<Vec<PaneInfo>> {
        Ok(Vec::new())
    }

    fn focus(&self, pane: &PaneId) -> io::Result<()> {
        self.focused.borrow_mut().push(pane.clone());
        Ok(())
    }

    fn new_claude_pane(&self) -> io::Result<()> {
        *self.new_panes.borrow_mut() += 1;
        Ok(())
    }
}

pub fn key(code: KeyCode) -> io::Result<Input> {
    Ok(Input::Key(KeyEvent::new(code, KeyModifiers::NONE)))
}

pub fn ctrl(c: char) -> io::Result<Input> {
    Ok(Input::Key(KeyEvent::new(
        KeyCode::Char(c),
        KeyModifiers::CONTROL,
    )))
}

pub fn tick() -> io::Result<Input> {
    Ok(Input::Tick)
}

pub struct Picker {
    pub terminal: Terminal<TestBackend>,
    pub app: App,
    pub tmux: FakeTmux,
    pub source: Rc<RefCell<Vec<Agent>>>,
}

impl Picker {
    pub fn new(agents: Vec<Agent>) -> Self {
        Self {
            terminal: Terminal::new(TestBackend::new(60, 8)).unwrap(),
            app: App::new(agents.clone()),
            tmux: FakeTmux::default(),
            source: Rc::new(RefCell::new(agents)),
        }
    }

    pub fn next_refresh_returns(&self, agents: Vec<Agent>) {
        *self.source.borrow_mut() = agents;
    }

    pub fn run(&mut self, inputs: Vec<io::Result<Input>>) -> io::Result<()> {
        let source = Rc::clone(&self.source);
        run(
            &mut self.terminal,
            &mut self.app,
            inputs.into_iter(),
            &self.tmux,
            || Ok(source.borrow().clone()),
        )
    }

    pub fn screen(&self) -> String {
        let buffer = self.terminal.backend().buffer();
        let width = buffer.area.width as usize;
        buffer
            .content
            .chunks(width)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn agent(label: &str, pane: &str) -> Agent {
    Agent {
        pid: 1,
        label: label.to_string(),
        status: Status::Idle,
        cwd: format!("/home/me/{label}").into(),
        pane: PaneId(pane.to_string()),
        session: "main".to_string(),
        window_index: 1,
        title: None,
        status_age: None,
        waiting_for: None,
    }
}

pub fn agent_with_status(label: &str, pane: &str, status: Status) -> Agent {
    Agent {
        status,
        ..agent(label, pane)
    }
}

impl Picker {
    pub fn cell(&self, x: u16, y: u16) -> &ratatui::buffer::Cell {
        &self.terminal.backend().buffer()[(x, y)]
    }
}
