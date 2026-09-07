use std::time::Duration;

pub const USAGE: &str = "usage: tmux-agents [status | cached <ttl-seconds> -- <command> [args...]]";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Tui,
    Status,
    Cached { ttl: Duration, command: Vec<String> },
}

pub fn parse<I, S>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut args = args.into_iter().map(|arg| arg.as_ref().to_string());
    match args.next().as_deref() {
        None => Ok(Command::Tui),
        Some("status") => Ok(Command::Status),
        Some("cached") => parse_cached(args),
        Some(arg) => Err(format!("unknown argument '{arg}'\n{USAGE}")),
    }
}

fn parse_cached(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let ttl = args
        .next()
        .and_then(|ttl| ttl.parse().ok())
        .map(Duration::from_secs)
        .ok_or_else(|| format!("cached needs a ttl in seconds\n{USAGE}"))?;
    let mut command: Vec<String> = args.collect();
    if command.first().is_some_and(|arg| arg == "--") {
        command.remove(0);
    }
    if command.is_empty() {
        return Err(format!("cached needs a command\n{USAGE}"));
    }
    Ok(Command::Cached { ttl, command })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_means_tui_and_anything_else_is_a_usage_error() {
        assert_eq!(parse(Vec::<String>::new()), Ok(Command::Tui));
        let err = parse(vec!["bogus".to_string()]).unwrap_err();
        assert!(err.contains("usage"), "{err}");
    }

    #[test]
    fn cached_takes_a_ttl_and_a_command_after_a_double_dash() {
        assert_eq!(
            parse(["cached", "5", "--", "git", "status"]),
            Ok(Command::Cached {
                ttl: Duration::from_secs(5),
                command: vec!["git".to_string(), "status".to_string()],
            })
        );
        assert!(parse(["cached", "5", "--"]).unwrap_err().contains("usage"));
        assert!(
            parse(["cached", "x", "--", "git"])
                .unwrap_err()
                .contains("usage")
        );
        assert!(parse(["cached"]).unwrap_err().contains("usage"));
    }

    #[test]
    fn status_subcommand_is_recognised() {
        assert_eq!(parse(vec!["status".to_string()]), Ok(Command::Status));
    }
}
