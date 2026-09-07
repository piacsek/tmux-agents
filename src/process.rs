use nix::sys::signal::kill;
use nix::unistd::Pid;

pub fn is_alive(pid: i32) -> bool {
    kill(Pid::from_raw(pid), None).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_pid_is_alive_and_a_reaped_child_is_not() {
        assert!(is_alive(std::process::id() as i32));

        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id() as i32;
        child.wait().unwrap();

        assert!(!is_alive(pid));
    }

    #[test]
    fn another_users_process_is_not_one_of_ours() {
        assert!(!is_alive(1));
    }
}
