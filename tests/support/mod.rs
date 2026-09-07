#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::rc::Rc;
use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tmux_agents::agents::Agent;
use tmux_agents::app::{App, Input, run};
use tmux_agents::config::Config;
use tmux_agents::registry::Status;
use tmux_agents::tmux::{Client, PaneId, PaneInfo, Tmux};

#[derive(Default)]
pub struct FakeTmux {
    focused: RefCell<Vec<PaneId>>,
    new_panes: RefCell<usize>,
    killed: RefCell<Vec<PaneId>>,
    captures: RefCell<HashMap<PaneId, Vec<String>>>,
    capture_fails: RefCell<bool>,
    clients: RefCell<Vec<Client>>,
    messages: RefCell<Vec<(String, String, u64)>>,
}

impl FakeTmux {
    pub fn focused(&self) -> Vec<PaneId> {
        self.focused.borrow().clone()
    }

    pub fn new_panes_requested(&self) -> usize {
        *self.new_panes.borrow()
    }

    pub fn killed(&self) -> Vec<PaneId> {
        self.killed.borrow().clone()
    }

    pub fn fail_captures(&self) {
        *self.capture_fails.borrow_mut() = true;
    }

    pub fn set_clients(&self, clients: &[(&str, &str)]) {
        *self.clients.borrow_mut() = clients
            .iter()
            .map(|(name, pane)| Client {
                name: name.to_string(),
                active_pane: PaneId(pane.to_string()),
            })
            .collect();
    }

    pub fn messages(&self) -> Vec<(String, String, u64)> {
        self.messages.borrow().clone()
    }

    pub fn set_capture(&self, pane: &str, lines: &[&str]) {
        self.captures.borrow_mut().insert(
            PaneId(pane.to_string()),
            lines.iter().map(|l| l.to_string()).collect(),
        );
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

    fn kill_pane(&self, pane: &PaneId) -> io::Result<()> {
        self.killed.borrow_mut().push(pane.clone());
        Ok(())
    }

    fn clients(&self) -> io::Result<Vec<Client>> {
        Ok(self.clients.borrow().clone())
    }

    fn display_message(&self, client: &str, text: &str, duration: Duration) -> io::Result<()> {
        self.messages.borrow_mut().push((
            client.to_string(),
            text.to_string(),
            duration.as_millis() as u64,
        ));
        Ok(())
    }

    fn capture(&self, pane: &PaneId) -> io::Result<Vec<String>> {
        if *self.capture_fails.borrow() {
            return Err(io::Error::other("can't find pane"));
        }
        Ok(self
            .captures
            .borrow()
            .get(pane)
            .cloned()
            .unwrap_or_default())
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
        Self::with_size(agents, 60, 8)
    }

    pub fn with_size(agents: Vec<Agent>, width: u16, height: u16) -> Self {
        Self::with_config(agents, width, height, Config::default())
    }

    pub fn with_config(agents: Vec<Agent>, width: u16, height: u16, config: Config) -> Self {
        Self {
            terminal: Terminal::new(TestBackend::new(width, height)).unwrap(),
            app: App::new(agents.clone(), config),
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
