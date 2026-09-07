use std::io;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PaneId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneInfo {
    pub id: PaneId,
    pub session: String,
    pub window_id: String,
    pub window_index: u32,
    pub current_path: PathBuf,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    pub name: String,
    pub active_pane: PaneId,
}

pub trait Tmux {
    fn list_panes(&self) -> io::Result<Vec<PaneInfo>>;
    fn focus(&self, pane: &PaneId) -> io::Result<()>;
    fn new_claude_pane(&self) -> io::Result<()>;
    fn kill_pane(&self, pane: &PaneId) -> io::Result<()>;
    fn capture(&self, pane: &PaneId) -> io::Result<Vec<String>>;
    fn clients(&self) -> io::Result<Vec<Client>>;
    fn display_message(&self, client: &str, text: &str) -> io::Result<()>;
}

const PANE_FORMAT: &str = "#{pane_id}\t#{session_name}\t#{window_id}\t#{window_index}\t#{pane_current_path}\t#{pane_title}";

const CLIENT_FORMAT: &str = "#{client_name}\t#{pane_id}";

#[derive(Debug, Default, Clone)]
pub struct CliTmux {
    socket_name: Option<String>,
}

impl CliTmux {
    pub fn with_socket(name: &str) -> Self {
        Self {
            socket_name: Some(name.to_string()),
        }
    }

    pub fn list_panes_args(&self) -> Vec<String> {
        self.args(&["list-panes", "-a", "-F", PANE_FORMAT])
    }

    pub fn focus_args(&self, pane: &PaneId) -> Vec<String> {
        self.args(&["switch-client", "-Z", "-t", &pane.0])
    }

    pub fn new_claude_pane_args(&self) -> Vec<String> {
        self.args(&[
            "split-window",
            "-h",
            "-c",
            "#{pane_current_path}",
            "zsh -ic claude",
        ])
    }

    pub fn kill_pane_args(&self, pane: &PaneId) -> Vec<String> {
        self.args(&["kill-pane", "-t", &pane.0])
    }

    pub fn list_clients_args(&self) -> Vec<String> {
        self.args(&["list-clients", "-F", CLIENT_FORMAT])
    }

    pub fn display_message_args(&self, client: &str, text: &str) -> Vec<String> {
        self.args(&["display-message", "-d", "4000", "-c", client, text])
    }

    pub fn capture_args(&self, pane: &PaneId) -> Vec<String> {
        self.args(&["capture-pane", "-p", "-t", &pane.0])
    }

    fn args(&self, command: &[&str]) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(socket) = &self.socket_name {
            args.push("-L".to_string());
            args.push(socket.clone());
        }
        args.extend(command.iter().map(|s| s.to_string()));
        args
    }

    fn run(&self, args: &[String]) -> io::Result<String> {
        let output = Command::new("tmux").args(args).output()?;
        if !output.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

impl Tmux for CliTmux {
    fn list_panes(&self) -> io::Result<Vec<PaneInfo>> {
        Ok(parse_list_panes(&self.run(&self.list_panes_args())?))
    }

    fn focus(&self, pane: &PaneId) -> io::Result<()> {
        self.run(&self.focus_args(pane)).map(drop)
    }

    fn new_claude_pane(&self) -> io::Result<()> {
        self.run(&self.new_claude_pane_args()).map(drop)
    }

    fn kill_pane(&self, pane: &PaneId) -> io::Result<()> {
        self.run(&self.kill_pane_args(pane)).map(drop)
    }

    fn capture(&self, pane: &PaneId) -> io::Result<Vec<String>> {
        let stdout = self.run(&self.capture_args(pane))?;
        Ok(stdout.lines().map(str::to_string).collect())
    }

    fn clients(&self) -> io::Result<Vec<Client>> {
        Ok(parse_list_clients(&self.run(&self.list_clients_args())?))
    }

    fn display_message(&self, client: &str, text: &str) -> io::Result<()> {
        self.run(&self.display_message_args(client, text)).map(drop)
    }
}

pub fn parse_list_clients(stdout: &str) -> Vec<Client> {
    stdout
        .lines()
        .filter_map(|line| {
            let (name, pane) = line.split_once('\t')?;
            Some(Client {
                name: name.to_string(),
                active_pane: PaneId(pane.to_string()),
            })
        })
        .collect()
}

pub fn parse_pane_ref(s: &str) -> Option<PaneId> {
    let (_, pane) = s.rsplit_once('.')?;
    pane.starts_with('%').then(|| PaneId(pane.to_string()))
}

pub fn parse_list_panes(stdout: &str) -> Vec<PaneInfo> {
    stdout.lines().filter_map(parse_pane_line).collect()
}

fn parse_pane_line(line: &str) -> Option<PaneInfo> {
    let mut fields = line.splitn(6, '\t');
    let id = fields.next()?;
    let session = fields.next()?;
    let window_id = fields.next()?;
    let window_index = fields.next()?.parse().ok()?;
    let current_path = fields.next()?;
    let title = fields.next().unwrap_or_default();
    Some(PaneInfo {
        id: PaneId(id.to_string()),
        session: session.to_string(),
        window_id: window_id.to_string(),
        window_index,
        current_path: PathBuf::from(current_path),
        title: title.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pane_ref_extracts_pane_id_or_rejects_garbage() {
        assert_eq!(
            parse_pane_ref("dotfiles:@7.%53"),
            Some(PaneId("%53".to_string()))
        );
        assert_eq!(parse_pane_ref("garbage"), None);
        assert_eq!(parse_pane_ref("a.b"), None);
        assert_eq!(parse_pane_ref(""), None);
    }

    #[test]
    fn parse_list_panes_reads_tab_separated_rows_with_title_last() {
        let stdout = "%53\tdotfiles\t@7\t2\t/Users/me/dotfiles\t✳ Fix the picker now\n\
                      %3\twebapp\t@2\t1\t/Users/me/webapp\tmy-macbook.local\n";

        let panes = parse_list_panes(stdout);

        assert_eq!(
            panes,
            vec![
                PaneInfo {
                    id: PaneId("%53".to_string()),
                    session: "dotfiles".to_string(),
                    window_id: "@7".to_string(),
                    window_index: 2,
                    current_path: PathBuf::from("/Users/me/dotfiles"),
                    title: "✳ Fix the picker now".to_string(),
                },
                PaneInfo {
                    id: PaneId("%3".to_string()),
                    session: "webapp".to_string(),
                    window_id: "@2".to_string(),
                    window_index: 1,
                    current_path: PathBuf::from("/Users/me/webapp"),
                    title: "my-macbook.local".to_string(),
                },
            ]
        );
    }

    #[test]
    fn cli_tmux_builds_focus_and_list_panes_argv() {
        let default = CliTmux::default();
        assert_eq!(
            default.focus_args(&PaneId("%53".to_string())),
            vec!["switch-client", "-Z", "-t", "%53"]
        );
        let list = default.list_panes_args();
        assert_eq!(list[0], "list-panes");
        assert!(list.contains(&"-a".to_string()));
        assert_eq!(
            list.last().unwrap(),
            "#{pane_id}\t#{session_name}\t#{window_id}\t#{window_index}\t#{pane_current_path}\t#{pane_title}"
        );

        let scoped = CliTmux::with_socket("ci");
        assert_eq!(&scoped.list_panes_args()[..2], &["-L", "ci"]);
        assert_eq!(
            &scoped.focus_args(&PaneId("%1".to_string()))[..2],
            &["-L", "ci"]
        );
    }

    #[test]
    fn list_clients_reports_each_clients_active_pane_and_display_message_targets_one() {
        let tmux = CliTmux::default();
        assert_eq!(
            tmux.list_clients_args(),
            vec!["list-clients", "-F", "#{client_name}\t#{pane_id}"]
        );
        assert_eq!(
            parse_list_clients("/dev/ttys003\t%53\n/dev/ttys009\t%2\n"),
            vec![
                Client {
                    name: "/dev/ttys003".to_string(),
                    active_pane: PaneId("%53".to_string()),
                },
                Client {
                    name: "/dev/ttys009".to_string(),
                    active_pane: PaneId("%2".to_string()),
                },
            ]
        );
        assert_eq!(
            tmux.display_message_args("/dev/ttys003", "◉ dotfiles needs input"),
            vec![
                "display-message",
                "-d",
                "4000",
                "-c",
                "/dev/ttys003",
                "◉ dotfiles needs input"
            ]
        );
    }

    #[test]
    fn capture_prints_the_given_panes_visible_content() {
        assert_eq!(
            CliTmux::default().capture_args(&PaneId("%7".to_string())),
            vec!["capture-pane", "-p", "-t", "%7"]
        );
    }

    #[test]
    fn kill_pane_targets_the_given_pane() {
        assert_eq!(
            CliTmux::default().kill_pane_args(&PaneId("%7".to_string())),
            vec!["kill-pane", "-t", "%7"]
        );
    }

    #[test]
    fn new_claude_pane_splits_the_callers_window_running_claude() {
        assert_eq!(
            CliTmux::default().new_claude_pane_args(),
            vec![
                "split-window",
                "-h",
                "-c",
                "#{pane_current_path}",
                "zsh -ic claude",
            ]
        );
    }
}
