//! Process ancestry: who started a process, and who started that one (CLI-01, d0031).
//!
//! The service tells an agent's `crewboard` commands from the user's by walking the caller's parents up to one of
//! its agent sessions. macOS reads a process's parent with `proc_pidinfo` (`PROC_PIDTBSDINFO`); Linux reads
//! `/proc/<pid>/stat`. Windows comes in T-0156.

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

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod os {
    pub fn parent_of(_pid: u32) -> Option<u32> {
        None
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
        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .unwrap();
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
