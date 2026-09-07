use std::io;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
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

pub trait Tmux {
    fn list_panes(&self) -> io::Result<Vec<PaneInfo>>;
    fn focus(&self, pane: &PaneId) -> io::Result<()>;
    fn new_claude_pane(&self) -> io::Result<()>;
}

const PANE_FORMAT: &str = "#{pane_id}\t#{session_name}\t#{window_id}\t#{window_index}\t#{pane_current_path}\t#{pane_title}";

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
