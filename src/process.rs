use nix::errno::Errno;
use nix::sys::signal::kill;
use nix::unistd::Pid;

pub fn is_alive(pid: i32) -> bool {
    match kill(Pid::from_raw(pid), None) {
        Ok(()) => true,
        Err(Errno::EPERM) => true,
        Err(_) => false,
    }
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
}
