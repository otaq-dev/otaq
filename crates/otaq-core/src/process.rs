//! Process ancestry: who started a process, and who started that one (CLI-01, d0031).
//!
//! The service tells an agent's `otaq` commands from the user's by walking the caller's parents up to one of
//! its agent sessions. macOS reads a process's parent with `proc_pidinfo` (`PROC_PIDTBSDINFO`); Linux reads
//! `/proc/<pid>/stat`; Windows takes a ToolHelp process snapshot. Windows reuses process IDs freely and keeps a
//! child's parent ID after the parent ended, so a "parent" that started after its child is no parent.

/// The longest chain walked, so a corrupt or racing process table can't loop forever.
const MAX_DEPTH: usize = 128;

/// The parent of `pid`, or `None` when the process is gone or has no parent (PID 0 and 1 end every chain).
pub fn parent_of(pid: u32) -> Option<u32> {
    let parent = os::parent_of(pid)?;
    (parent != 0 && parent != pid).then_some(parent)
}

/// `pid` itself, then its parent, grandparent and so on up to the first process (`launchd` or `init`).
pub fn ancestry(pid: u32) -> Vec<u32> {
    let mut chain = vec![pid];
    let mut current = pid;
    while chain.len() < MAX_DEPTH {
        match parent_of(current) {
            Some(parent) if !chain.contains(&parent) => {
                chain.push(parent);
                current = parent;
            }
            _ => break,
        }
    }
    chain
}

#[cfg(target_os = "macos")]
mod os {
    pub fn parent_of(pid: u32) -> Option<u32> {
        let pid = i32::try_from(pid).ok()?;
        let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
        let size = i32::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
        // SAFETY: the buffer is a zeroed `proc_bsdinfo` of exactly `size` bytes; the kernel writes at most that.
        let written = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTBSDINFO,
                0,
                info.as_mut_ptr().cast(),
                size,
            )
        };
        if written != size {
            return None;
        }
        // SAFETY: the kernel filled the whole structure (checked above).
        Some(unsafe { info.assume_init() }.pbi_ppid)
    }
}

#[cfg(target_os = "linux")]
mod os {
    pub fn parent_of(pid: u32) -> Option<u32> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // "pid (comm) state ppid …": the command name may hold spaces and parentheses, so read after the last ')'.
        let rest = &stat[stat.rfind(')')? + 1..];
        rest.split_whitespace().nth(1)?.parse().ok()
    }
}

#[cfg(windows)]
mod os {
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, FILETIME, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
        STILL_ACTIVE,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    pub fn parent_of(pid: u32) -> Option<u32> {
        let parent = snapshot_parent(pid)?;
        // The parent ID outlives the parent; a process that started later only reuses its number.
        match (started_at(parent), started_at(pid)) {
            (Some(parent_start), Some(child_start)) if parent_start > child_start => None,
            _ => Some(parent),
        }
    }

    fn snapshot_parent(pid: u32) -> Option<u32> {
        // SAFETY: the snapshot handle is closed below.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).expect("small");
        let mut found = None;
        // SAFETY: `entry` has its size set, as the API requires.
        let mut more = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
        while more {
            if entry.th32ProcessID == pid {
                found = Some(entry.th32ParentProcessID);
                break;
            }
            // SAFETY: as above.
            more = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
        }
        // SAFETY: the handle came from CreateToolhelp32Snapshot.
        unsafe { CloseHandle(snapshot) };
        found
    }

    /// When a process started (100 ns ticks since 1601), if it can be read.
    fn started_at(pid: u32) -> Option<u64> {
        let process = open(pid)?;
        let zero = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
        // SAFETY: a valid process handle and four out parameters.
        let ok =
            unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) };
        // SAFETY: the handle came from OpenProcess.
        unsafe { CloseHandle(process) };
        (ok != 0)
            .then(|| (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }

    fn open(pid: u32) -> Option<HANDLE> {
        // SAFETY: plain handle-returning call; callers close the handle.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        (!process.is_null()).then_some(process)
    }

    pub fn alive(pid: u32) -> bool {
        let Some(process) = open(pid) else {
            // It exists but belongs to someone else (or is protected).
            // SAFETY: reads this thread's last error.
            return unsafe { GetLastError() } == ERROR_ACCESS_DENIED;
        };
        let mut code = 0u32;
        // SAFETY: a valid process handle and an out parameter.
        let ok = unsafe { GetExitCodeProcess(process, &mut code) };
        // SAFETY: the handle came from OpenProcess.
        unsafe { CloseHandle(process) };
        ok != 0 && code == STILL_ACTIVE as u32
    }
}

/// Whether a process with this ID is running (possibly as another user).
pub fn alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let Some(pid) = i32::try_from(pid)
            .ok()
            .and_then(rustix::process::Pid::from_raw)
        else {
            return false;
        };
        match rustix::process::test_kill_process(pid) {
            Ok(()) => true,
            // It exists but belongs to someone else.
            Err(e) => e == rustix::io::Errno::PERM,
        }
    }
    #[cfg(windows)]
    {
        os::alive(pid)
    }
}

/// Windows process creation flags (`CREATE_NO_WINDOW`, …), for programs that must not open a console window or
/// must outlive the console that started them.
#[cfg(windows)]
pub use windows_sys::Win32::System::Threading::{
    CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, DETACHED_PROCESS,
};

/// Windows: makes this process's standard input, output and error handles non-inheritable. Windows hands every
/// inheritable handle to a child, not only the ones set as its standard handles; a long-lived child (the service
/// started by `otaq daemon start`) would otherwise keep the caller's output pipe open, and whoever reads
/// that pipe (a script, a test) would wait for it forever.
#[cfg(windows)]
pub fn keep_std_handles_from_children() {
    use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};
    use windows_sys::Win32::System::Console::{
        GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    for which in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: a standard handle (possibly null or invalid, which the call then refuses) and plain flags.
        unsafe {
            SetHandleInformation(GetStdHandle(which), HANDLE_FLAG_INHERIT, 0);
        }
    }
}

/// A session's program and everything it starts, stopped together. On macOS and Linux that is the program's
/// process group (a PTY's program leads its own session and group). Windows has no process groups to signal: the
/// program is put in a job object, and stopping it ends every process in the job. The job is also closed (and its
/// processes ended) if the service itself ends, as closing a PTY hangs up its group on macOS and Linux.
pub struct Group {
    #[cfg(unix)]
    leader: Option<rustix::process::Pid>,
    #[cfg(windows)]
    job: Option<job::Job>,
}

impl Group {
    /// The group led by `pid`, the program of a new PTY.
    pub fn of_leader(pid: Option<u32>) -> Self {
        #[cfg(unix)]
        {
            Self {
                leader: pid
                    .and_then(|p| i32::try_from(p).ok())
                    .and_then(rustix::process::Pid::from_raw),
            }
        }
        #[cfg(windows)]
        {
            let job = pid.and_then(|pid| match job::Job::with_process(pid) {
                Ok(job) => Some(job),
                Err(e) => {
                    tracing::warn!(pid, error = %e, "can't put the process in a job; Stop ends only the program");
                    None
                }
            });
            Self { job }
        }
    }

    /// Asks the group to end: SIGHUP, as closing a terminal does. On Windows the job is ended.
    pub fn hang_up(&self) {
        #[cfg(unix)]
        self.signal(rustix::process::Signal::HUP);
        #[cfg(windows)]
        self.end();
    }

    /// Ends the group at once: SIGKILL. On Windows the job is ended.
    pub fn kill(&self) {
        #[cfg(unix)]
        self.signal(rustix::process::Signal::KILL);
        #[cfg(windows)]
        self.end();
    }

    /// Whether `pid` runs in this group's job (Windows). A job holds every process started inside it, however it
    /// was started, so this also finds what Git Bash starts through its own `exec`, which doesn't keep the Windows
    /// parent chain. macOS and Linux answer by ancestry alone (`false` here).
    pub fn contains(&self, pid: u32) -> bool {
        #[cfg(windows)]
        {
            self.job.as_ref().is_some_and(|job| job.contains(pid))
        }
        #[cfg(unix)]
        {
            let _ = pid;
            false
        }
    }

    #[cfg(unix)]
    fn signal(&self, signal: rustix::process::Signal) {
        if let Some(pid) = self.leader {
            let _ = rustix::process::kill_process_group(pid, signal);
        }
    }

    #[cfg(windows)]
    fn end(&self) {
        if let Some(job) = &self.job {
            job.terminate();
        }
    }
}

#[cfg(windows)]
mod job {
    use std::io;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject, TerminateJobObject,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };

    pub struct Job(HANDLE);

    // SAFETY: a job handle may be used from any thread.
    unsafe impl Send for Job {}
    // SAFETY: as above; the handle is only read.
    unsafe impl Sync for Job {}

    impl Job {
        /// A new job holding `pid` (and whatever it starts from now on), ended when the job is closed.
        pub fn with_process(pid: u32) -> io::Result<Self> {
            // SAFETY: an unnamed job with default security; closed on drop.
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = Self(handle);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            // SAFETY: the structure and its size match the information class.
            let ok = unsafe {
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    (&raw const limits).cast(),
                    u32::try_from(std::mem::size_of_val(&limits)).expect("small"),
                )
            };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: plain handle-returning call; closed below.
            let process = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid) };
            if process.is_null() {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: both handles are valid.
            let ok = unsafe { AssignProcessToJobObject(job.0, process) };
            let assigned = if ok == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            };
            // SAFETY: the handle came from OpenProcess.
            unsafe { CloseHandle(process) };
            assigned.map(|()| job)
        }

        pub fn contains(&self, pid: u32) -> bool {
            // SAFETY: plain handle-returning call; closed below.
            let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
            if process.is_null() {
                return false;
            }
            let mut inside = 0;
            // SAFETY: valid process and job handles and an out parameter.
            let ok = unsafe { IsProcessInJob(process, self.0, &mut inside) };
            // SAFETY: the handle came from OpenProcess.
            unsafe { CloseHandle(process) };
            ok != 0 && inside != 0
        }

        pub fn terminate(&self) {
            // SAFETY: a valid job handle.
            unsafe { TerminateJobObject(self.0, 1) };
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateJobObjectW.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Ordinary commands for tests on every platform (SDD §23.1: no fake `claude`, plain commands in its place).
pub mod testing {
    use std::process::Command;

    /// A command that exits at once with status 0 (`true`; `cmd /c exit 0` on Windows).
    pub fn quick_command() -> Command {
        if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/d", "/c", "exit 0"]);
            command
        } else {
            Command::new("true")
        }
    }

    /// A stand-in program named `name` in `dir` that runs the `sh` script `script` (stand-ins for `gh`, `az`,
    /// `git` in tests; never `claude`). Returns the path a PATH lookup finds: `dir/name` on macOS and Linux;
    /// on Windows `dir/name.cmd`, which runs `dir/name.sh` with Git for Windows' `sh` (see [`git_sh`]).
    pub fn stub_command(dir: &std::path::Path, name: &str, script: &str) -> std::path::PathBuf {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = dir.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).expect("stub written");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("stub executable");
            path
        }
        #[cfg(windows)]
        {
            let sh = git_sh().expect("Git for Windows' sh.exe (tests need Git for Windows)");
            std::fs::write(dir.join(format!("{name}.sh")), format!("{script}\n"))
                .expect("stub written");
            let path = dir.join(format!("{name}.cmd"));
            std::fs::write(
                &path,
                format!("@\"{}\" \"%~dp0{name}.sh\" %*\r\n", sh.display()),
            )
            .expect("stub written");
            path
        }
    }

    /// Git for Windows' `sh.exe` (the one in `bin`, which puts Git's Unix tools on its PATH), found from `git`
    /// on PATH. Tests on Windows use it where macOS and Linux use `/bin/sh`.
    #[cfg(windows)]
    pub fn git_sh() -> Option<std::path::PathBuf> {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let git = crate::tools::find_on_path("git", &path)?;
        let root = git.parent()?.parent()?;
        Some(root.join("bin").join("sh.exe")).filter(|p| p.is_file())
    }

    /// A command that runs for about `secs` seconds (`sleep`; `ping` on Windows, which has no plain sleep
    /// that works without a console).
    pub fn sleep_command(secs: u32) -> Command {
        if cfg!(windows) {
            let mut command = Command::new("ping");
            command
                .args(["-n", &(secs + 1).to_string(), "127.0.0.1"])
                .stdout(std::process::Stdio::null());
            command
        } else {
            let mut command = Command::new("sleep");
            command.arg(secs.to_string());
            command
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_has_its_parent() {
        let me = std::process::id();
        let chain = ancestry(me);
        assert_eq!(chain[0], me);
        assert!(chain.len() >= 2, "{chain:?}");
        assert_eq!(parent_of(me), Some(chain[1]));
    }

    #[test]
    fn a_child_lists_this_process_among_its_ancestors() {
        let mut child = testing::sleep_command(5).spawn().unwrap();
        let chain = ancestry(child.id());
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(chain[1], std::process::id());
    }

    #[test]
    fn a_missing_process_has_no_parent() {
        assert_eq!(parent_of(u32::MAX - 7), None);
        assert_eq!(ancestry(u32::MAX - 7), [u32::MAX - 7]);
    }
}
