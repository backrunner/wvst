use tokio::process::{Child, Command};

#[derive(Debug, Clone, Copy)]
pub struct WorkerTerminationTarget {
    #[cfg(unix)]
    process_group_id: Option<u32>,
}

#[cfg(unix)]
impl WorkerTerminationTarget {
    pub fn configure_command(command: &mut Command) {
        command.process_group(0);
    }

    pub fn from_child(child: &Child) -> Self {
        Self {
            process_group_id: child.id(),
        }
    }

    pub fn terminate_tree(self) -> bool {
        self.send_process_group_signal(rustix::process::Signal::TERM)
    }

    pub fn kill_tree(self) -> bool {
        self.send_process_group_signal(rustix::process::Signal::KILL)
    }

    fn send_process_group_signal(self, signal: rustix::process::Signal) -> bool {
        let Some(process_group_id) = self.process_group_id else {
            return false;
        };
        let Some(pid) = rustix::process::Pid::from_raw(process_group_id as i32) else {
            return false;
        };

        rustix::process::kill_process_group(pid, signal).is_ok()
    }
}

#[cfg(not(unix))]
impl WorkerTerminationTarget {
    pub fn configure_command(_command: &mut Command) {}

    pub fn from_child(_child: &Child) -> Self {
        Self {}
    }

    pub fn terminate_tree(self) -> bool {
        false
    }

    pub fn kill_tree(self) -> bool {
        false
    }
}
