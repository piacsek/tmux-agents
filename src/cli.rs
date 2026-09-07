pub const USAGE: &str = "usage: tmux-agents [status]";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Tui,
    Status,
}

pub fn parse<I, S>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    match args.into_iter().next() {
        None => Ok(Command::Tui),
        Some(arg) if arg.as_ref() == "status" => Ok(Command::Status),
        Some(arg) => Err(format!("unknown argument '{}'\n{USAGE}", arg.as_ref())),
    }
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
    fn status_subcommand_is_recognised() {
        assert_eq!(parse(vec!["status".to_string()]), Ok(Command::Status));
    }
}
