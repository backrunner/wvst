use tokio::process::{Child, Command};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct WorkerResourceLimits {
    address_space_bytes: Option<u64>,
}

impl WorkerResourceLimits {
    pub const fn none() -> Self {
        Self {
            address_space_bytes: None,
        }
    }

    pub const fn with_address_space_bytes(mut self, bytes: u64) -> Self {
        self.address_space_bytes = Some(bytes);
        self
    }

    pub const fn address_space_bytes(&self) -> Option<u64> {
        self.address_space_bytes
    }

    pub const fn is_empty(&self) -> bool {
        self.address_space_bytes.is_none()
    }
}

#[derive(Debug)]
pub struct WorkerTerminationTarget {
    #[cfg(unix)]
    process_group_id: Option<u32>,
    #[cfg(windows)]
    job: Option<windows::JobHandle>,
}

#[cfg(unix)]
impl WorkerTerminationTarget {
    pub fn configure_command(command: &mut Command, limits: WorkerResourceLimits) {
        command.process_group(0);
        configure_unix_resource_limits(command, limits);
    }

    pub fn from_child(child: &Child) -> Self {
        Self {
            process_group_id: child.id(),
        }
    }

    pub fn terminate_tree(&self) -> bool {
        self.send_process_group_signal(rustix::process::Signal::TERM)
    }

    pub fn kill_tree(&self) -> bool {
        self.send_process_group_signal(rustix::process::Signal::KILL)
    }

    fn send_process_group_signal(&self, signal: rustix::process::Signal) -> bool {
        let Some(process_group_id) = self.process_group_id else {
            return false;
        };
        let Some(pid) = rustix::process::Pid::from_raw(process_group_id as i32) else {
            return false;
        };

        rustix::process::kill_process_group(pid, signal).is_ok()
    }
}

#[cfg(unix)]
fn configure_unix_resource_limits(command: &mut Command, limits: WorkerResourceLimits) {
    if limits.is_empty() {
        return;
    }

    unsafe {
        // SAFETY: `pre_exec` runs in the child process after fork and before
        // exec. The closure only calls async-signal-safe `setrlimit` through
        // rustix with Copy data captured from the parent; it does not touch
        // shared Rust state, allocate, lock, or perform I/O.
        command.pre_exec(move || apply_unix_resource_limits(limits));
    }
}

#[cfg(unix)]
fn apply_unix_resource_limits(limits: WorkerResourceLimits) -> std::io::Result<()> {
    if let Some(bytes) = limits.address_space_bytes {
        rustix::process::setrlimit(
            rustix::process::Resource::As,
            rustix::process::Rlimit {
                current: Some(bytes),
                maximum: Some(bytes),
            },
        )
        .map_err(std::io::Error::from)?;
    }

    Ok(())
}

#[cfg(windows)]
impl WorkerTerminationTarget {
    pub fn configure_command(_command: &mut Command, _limits: WorkerResourceLimits) {}

    pub fn from_child(child: &Child) -> Self {
        Self {
            job: windows::JobHandle::create_for_child(child),
        }
    }

    pub fn terminate_tree(&self) -> bool {
        self.job
            .as_ref()
            .is_some_and(|job| job.terminate(windows::EXIT_TERMINATE))
    }

    pub fn kill_tree(&self) -> bool {
        self.job
            .as_ref()
            .is_some_and(|job| job.terminate(windows::EXIT_KILL))
    }
}

#[cfg(not(any(unix, windows)))]
impl WorkerTerminationTarget {
    pub fn configure_command(_command: &mut Command, _limits: WorkerResourceLimits) {}

    pub fn from_child(_child: &Child) -> Self {
        Self {}
    }

    pub fn terminate_tree(&self) -> bool {
        false
    }

    pub fn kill_tree(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_limits_default_to_empty() {
        let limits = WorkerResourceLimits::none();

        assert!(limits.is_empty());
        assert_eq!(limits.address_space_bytes(), None);
    }

    #[test]
    fn resource_limits_store_address_space_limit() {
        let limits = WorkerResourceLimits::none().with_address_space_bytes(64 * 1024 * 1024);

        assert!(!limits.is_empty());
        assert_eq!(limits.address_space_bytes(), Some(64 * 1024 * 1024));
    }
}

#[cfg(windows)]
mod windows {
    use tokio::process::Child;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };

    pub const EXIT_TERMINATE: u32 = 1;
    pub const EXIT_KILL: u32 = 9;

    #[derive(Debug)]
    pub struct JobHandle(HANDLE);

    // SAFETY: HANDLE values are opaque kernel handles. JobHandle owns one job
    // handle, only calls thread-safe kernel APIs on it, and closes it exactly
    // once from Drop.
    unsafe impl Send for JobHandle {}

    // SAFETY: TerminateJobObject does not mutate Rust-managed memory and the
    // owned HANDLE remains valid until Drop; sharing references is sound.
    unsafe impl Sync for JobHandle {}

    impl JobHandle {
        pub fn create_for_child(child: &Child) -> Option<Self> {
            let process = child.raw_handle()?;
            let job = unsafe {
                // SAFETY: Null security attributes and name are accepted by
                // CreateJobObjectW. The returned handle is validated below and
                // owned by JobHandle on success.
                CreateJobObjectW(std::ptr::null(), std::ptr::null())
            };
            let handle = Self::from_raw(job)?;

            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let limit_size = u32::try_from(std::mem::size_of_val(&limits)).ok()?;
            let limits_set = unsafe {
                // SAFETY: `limits` points to a properly initialized
                // JOBOBJECT_EXTENDED_LIMIT_INFORMATION value and size matches
                // the pointed value for the selected information class.
                SetInformationJobObject(
                    handle.0,
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    limit_size,
                )
            };
            if limits_set == 0 {
                return None;
            }

            let assigned = unsafe {
                // SAFETY: `handle` is a live job handle and `process` is a
                // borrowed live child process handle from Tokio.
                AssignProcessToJobObject(handle.0, process.cast())
            };
            if assigned == 0 {
                return None;
            }

            Some(handle)
        }

        fn from_raw(handle: HANDLE) -> Option<Self> {
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                return None;
            }
            Some(Self(handle))
        }

        pub fn terminate(&self, exit_code: u32) -> bool {
            unsafe {
                // SAFETY: JobHandle owns a valid job handle until Drop.
                TerminateJobObject(self.0, exit_code) != 0
            }
        }
    }

    impl Drop for JobHandle {
        fn drop(&mut self) {
            unsafe {
                // SAFETY: JobHandle owns this handle and closes it exactly once.
                CloseHandle(self.0);
            }
        }
    }
}
