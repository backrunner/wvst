use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use tokio::process::{Child, Command};

pub const DEFAULT_LINUX_CGROUP_CPU_PERIOD_MICROS: u64 = 100_000;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct WorkerResourceLimits {
    address_space_bytes: Option<u64>,
    cpu_time_seconds: Option<u64>,
    linux_cgroup: Option<LinuxCgroupLimits>,
}

impl WorkerResourceLimits {
    pub fn none() -> Self {
        Self {
            address_space_bytes: None,
            cpu_time_seconds: None,
            linux_cgroup: None,
        }
    }

    pub fn with_address_space_bytes(mut self, bytes: u64) -> Self {
        self.address_space_bytes = Some(bytes);
        self
    }

    pub fn with_cpu_time_seconds(mut self, seconds: u64) -> Self {
        self.cpu_time_seconds = Some(seconds);
        self
    }

    pub fn with_linux_cgroup(mut self, cgroup: LinuxCgroupLimits) -> Self {
        self.linux_cgroup = Some(cgroup);
        self
    }

    pub const fn address_space_bytes(&self) -> Option<u64> {
        self.address_space_bytes
    }

    pub const fn cpu_time_seconds(&self) -> Option<u64> {
        self.cpu_time_seconds
    }

    pub fn linux_cgroup(&self) -> Option<&LinuxCgroupLimits> {
        self.linux_cgroup.as_ref()
    }

    pub fn is_empty(&self) -> bool {
        self.address_space_bytes.is_none()
            && self.cpu_time_seconds.is_none()
            && self
                .linux_cgroup
                .as_ref()
                .is_none_or(LinuxCgroupLimits::is_empty)
    }

    fn has_unix_rlimits(&self) -> bool {
        self.address_space_bytes.is_some() || self.cpu_time_seconds.is_some()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LinuxCgroupLimits {
    parent: PathBuf,
    memory_max_bytes: Option<u64>,
    cpu_quota_micros: Option<u64>,
    cpu_period_micros: u64,
}

impl LinuxCgroupLimits {
    pub fn new(parent: impl Into<PathBuf>) -> Self {
        Self {
            parent: parent.into(),
            memory_max_bytes: None,
            cpu_quota_micros: None,
            cpu_period_micros: DEFAULT_LINUX_CGROUP_CPU_PERIOD_MICROS,
        }
    }

    pub fn with_memory_max_bytes(mut self, bytes: u64) -> Self {
        self.memory_max_bytes = Some(bytes.max(1));
        self
    }

    pub fn with_cpu_max_micros(mut self, quota_micros: u64, period_micros: u64) -> Self {
        self.cpu_quota_micros = Some(quota_micros.max(1));
        self.cpu_period_micros = period_micros.max(1);
        self
    }

    pub fn parent(&self) -> &Path {
        &self.parent
    }

    pub const fn memory_max_bytes(&self) -> Option<u64> {
        self.memory_max_bytes
    }

    pub const fn cpu_max_micros(&self) -> Option<(u64, u64)> {
        match self.cpu_quota_micros {
            Some(quota) => Some((quota, self.cpu_period_micros)),
            None => None,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.memory_max_bytes.is_none() && self.cpu_quota_micros.is_none()
    }
}

#[derive(Debug)]
pub enum WorkerSupervisionError {
    MissingChildId,
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl Display for WorkerSupervisionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingChildId => write!(formatter, "worker child pid is unavailable"),
            Self::Io { path, source } => {
                write!(formatter, "{}: {source}", path.display())
            }
        }
    }
}

impl Error for WorkerSupervisionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingChildId => None,
            Self::Io { source, .. } => Some(source),
        }
    }
}

#[derive(Debug)]
pub struct WorkerTerminationTarget {
    #[cfg(unix)]
    process_group_id: Option<u32>,
    #[cfg(target_os = "linux")]
    cgroup: Option<linux::CgroupHandle>,
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
        Self::from_child_with_limits(child, WorkerResourceLimits::none())
    }

    pub fn from_child_with_limits(child: &Child, limits: WorkerResourceLimits) -> Self {
        Self::try_from_child_with_limits(child, limits).unwrap_or_else(|_| Self {
            process_group_id: child.id(),
            #[cfg(target_os = "linux")]
            cgroup: None,
        })
    }

    pub fn try_from_child_with_limits(
        child: &Child,
        limits: WorkerResourceLimits,
    ) -> Result<Self, WorkerSupervisionError> {
        #[cfg(target_os = "linux")]
        let cgroup = linux::CgroupHandle::create_for_child(child, limits.linux_cgroup())?;
        #[cfg(not(target_os = "linux"))]
        let _ = limits;

        Ok(Self {
            process_group_id: child.id(),
            #[cfg(target_os = "linux")]
            cgroup,
        })
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
    if !limits.has_unix_rlimits() {
        return;
    }
    let rlimits = UnixResourceLimitValues {
        address_space_bytes: limits.address_space_bytes,
        cpu_time_seconds: limits.cpu_time_seconds,
    };

    unsafe {
        // SAFETY: `pre_exec` runs in the child process after fork and before
        // exec. The closure only calls async-signal-safe `setrlimit` through
        // rustix with Copy data captured from the parent; it does not touch
        // shared Rust state, allocate, lock, or perform I/O.
        command.pre_exec(move || apply_unix_resource_limits(rlimits));
    }
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy)]
struct UnixResourceLimitValues {
    address_space_bytes: Option<u64>,
    cpu_time_seconds: Option<u64>,
}

#[cfg(unix)]
fn apply_unix_resource_limits(limits: UnixResourceLimitValues) -> std::io::Result<()> {
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
    if let Some(seconds) = limits.cpu_time_seconds {
        rustix::process::setrlimit(
            rustix::process::Resource::Cpu,
            rustix::process::Rlimit {
                current: Some(seconds),
                maximum: Some(seconds),
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
        Self::from_child_with_limits(child, WorkerResourceLimits::none())
    }

    pub fn from_child_with_limits(child: &Child, limits: WorkerResourceLimits) -> Self {
        Self {
            job: windows::JobHandle::create_for_child(child, limits),
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

    pub fn from_child(child: &Child) -> Self {
        Self::from_child_with_limits(child, WorkerResourceLimits::none())
    }

    pub fn from_child_with_limits(_child: &Child, _limits: WorkerResourceLimits) -> Self {
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
        assert_eq!(limits.cpu_time_seconds(), None);
    }

    #[test]
    fn resource_limits_store_address_space_limit() {
        let limits = WorkerResourceLimits::none().with_address_space_bytes(64 * 1024 * 1024);

        assert!(!limits.is_empty());
        assert_eq!(limits.address_space_bytes(), Some(64 * 1024 * 1024));
    }

    #[test]
    fn resource_limits_store_cpu_time_limit() {
        let limits = WorkerResourceLimits::none().with_cpu_time_seconds(30);

        assert!(!limits.is_empty());
        assert_eq!(limits.cpu_time_seconds(), Some(30));
    }

    #[test]
    fn linux_cgroup_limits_store_memory_and_cpu_max() {
        let cgroup = LinuxCgroupLimits::new("/sys/fs/cgroup/wvst")
            .with_memory_max_bytes(128 * 1024 * 1024)
            .with_cpu_max_micros(50_000, 100_000);
        let limits = WorkerResourceLimits::none().with_linux_cgroup(cgroup.clone());

        assert!(!limits.is_empty());
        assert_eq!(limits.linux_cgroup(), Some(&cgroup));
        assert_eq!(cgroup.parent(), Path::new("/sys/fs/cgroup/wvst"));
        assert_eq!(cgroup.memory_max_bytes(), Some(128 * 1024 * 1024));
        assert_eq!(cgroup.cpu_max_micros(), Some((50_000, 100_000)));
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use tokio::process::Child;

    use super::{LinuxCgroupLimits, WorkerSupervisionError};

    #[derive(Debug)]
    pub struct CgroupHandle {
        path: PathBuf,
    }

    impl CgroupHandle {
        pub fn create_for_child(
            child: &Child,
            limits: Option<&LinuxCgroupLimits>,
        ) -> Result<Option<Self>, WorkerSupervisionError> {
            let Some(limits) = limits.filter(|limits| !limits.is_empty()) else {
                return Ok(None);
            };
            let pid = child.id().ok_or(WorkerSupervisionError::MissingChildId)?;
            let path = worker_cgroup_path(limits.parent(), pid);

            create_dir(&path)?;
            if let Some(bytes) = limits.memory_max_bytes() {
                write_cgroup_file(&path.join("memory.max"), &bytes.to_string())?;
            }
            if let Some((quota, period)) = limits.cpu_max_micros() {
                write_cgroup_file(&path.join("cpu.max"), &format!("{quota} {period}"))?;
            }
            write_cgroup_file(&path.join("cgroup.procs"), &pid.to_string())?;

            Ok(Some(Self { path }))
        }
    }

    impl Drop for CgroupHandle {
        fn drop(&mut self) {
            let _ = fs::remove_dir(&self.path);
        }
    }

    fn worker_cgroup_path(parent: &Path, pid: u32) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        parent.join(format!("wvst-worker-{pid}-{nonce}"))
    }

    fn create_dir(path: &Path) -> Result<(), WorkerSupervisionError> {
        fs::create_dir(path).map_err(|source| WorkerSupervisionError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn write_cgroup_file(path: &Path, value: &str) -> Result<(), WorkerSupervisionError> {
        fs::write(path, value).map_err(|source| WorkerSupervisionError::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}

#[cfg(windows)]
mod windows {
    use super::WorkerResourceLimits;
    use tokio::process::Child;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_JOB_MEMORY,
        JOB_OBJECT_LIMIT_JOB_TIME, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };

    pub const EXIT_TERMINATE: u32 = 1;
    pub const EXIT_KILL: u32 = 9;
    const WINDOWS_100NS_PER_SECOND: u64 = 10_000_000;

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
        pub fn create_for_child(child: &Child, limits: WorkerResourceLimits) -> Option<Self> {
            let process = child.raw_handle()?;
            let job = unsafe {
                // SAFETY: Null security attributes and name are accepted by
                // CreateJobObjectW. The returned handle is validated below and
                // owned by JobHandle on success.
                CreateJobObjectW(std::ptr::null(), std::ptr::null())
            };
            let handle = Self::from_raw(job)?;

            let mut job_limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            apply_resource_limits(&mut job_limits, limits);
            let limit_size = u32::try_from(std::mem::size_of_val(&job_limits)).ok()?;
            let limits_set = unsafe {
                // SAFETY: `job_limits` points to a properly initialized
                // JOBOBJECT_EXTENDED_LIMIT_INFORMATION value and size matches
                // the pointed value for the selected information class.
                SetInformationJobObject(
                    handle.0,
                    JobObjectExtendedLimitInformation,
                    (&job_limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
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

    fn apply_resource_limits(
        job_limits: &mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        resource_limits: WorkerResourceLimits,
    ) {
        job_limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if let Some(bytes) = resource_limits.address_space_bytes() {
            if let Ok(bytes) = usize::try_from(bytes) {
                job_limits.JobMemoryLimit = bytes;
                job_limits.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
            }
        }
        if let Some(seconds) = resource_limits.cpu_time_seconds() {
            if let Some(ticks) = seconds.checked_mul(WINDOWS_100NS_PER_SECOND) {
                if let Ok(ticks) = i64::try_from(ticks) {
                    job_limits.BasicLimitInformation.PerJobUserTimeLimit = ticks;
                    job_limits.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_JOB_TIME;
                }
            }
        }
    }
}
