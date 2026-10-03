//! The process in a terminal's foreground, as herdr finds it (`src/platform/`): the terminal's
//! foreground process group, read through its shell, that group's leader's name and
//! arguments, and a process's working directory.
//!
//! Ported from herdr (https://github.com/herdrdev/herdr), licensed under the Apache License,
//! Version 2.0 (see `LICENSE-APACHE`). Changed: only the group's leader is read.

/// A process, as agent identification reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ForegroundProcess {
    /// The kernel's name for it.
    pub name: String,
    /// The basename of `argv[0]`, without a login shell's `-`. Programs may change it, as
    /// Node's `process.title` does.
    pub argv0: Option<String>,
    pub argv: Option<Vec<String>>,
}

impl ForegroundProcess {
    pub(crate) fn new(name: String, argv: Vec<String>) -> Self {
        let argv0 = argv.first().and_then(|argv0| {
            let basename = std::path::Path::new(argv0).file_name()?.to_str()?;
            let name = basename.strip_prefix('-').unwrap_or(basename);
            (!name.is_empty()).then(|| name.to_string())
        });
        Self {
            name,
            argv0,
            argv: (!argv.is_empty()).then_some(argv),
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::ForegroundProcess;

    fn bsd_info(pid: u32) -> Option<libc::proc_bsdinfo> {
        // SAFETY: `info` is plain data the size `proc_pidinfo` is told to fill.
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
        // SAFETY: the buffer is `size` bytes.
        let filled = unsafe {
            libc::proc_pidinfo(
                pid as libc::c_int,
                libc::PROC_PIDTBSDINFO,
                0,
                &mut info as *mut _ as *mut libc::c_void,
                size,
            )
        };
        (filled == size).then_some(info)
    }

    /// The foreground process group of `pid`'s controlling terminal (`e_tpgid`).
    pub(crate) fn foreground_process_group_id(pid: u32) -> Option<u32> {
        let group = bsd_info(pid)?.e_tpgid;
        (group > 0).then_some(group)
    }

    pub(crate) fn group_leader(process_group_id: u32) -> Option<ForegroundProcess> {
        let info = bsd_info(process_group_id)?;
        if info.pbi_pgid != process_group_id {
            return None;
        }
        let end = info
            .pbi_comm
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(info.pbi_comm.len());
        if end == 0 {
            return None;
        }
        let name: Vec<u8> = info.pbi_comm[..end]
            .iter()
            .map(|&byte| byte as u8)
            .collect();
        let name = String::from_utf8(name).ok()?;
        let argv = kern_procargs2(process_group_id)
            .and_then(|buffer| procargs2_argv(&buffer))
            .unwrap_or_default();
        Some(ForegroundProcess::new(name, argv))
    }

    /// A process's working directory (herdr's `process_cwd`): `pvi_cdir` from
    /// `PROC_PIDVNODEPATHINFO`.
    pub(crate) fn process_cwd(pid: u32) -> Option<std::path::PathBuf> {
        use std::os::unix::ffi::OsStrExt as _;
        // SAFETY: `info` is plain data the size `proc_pidinfo` is told to fill.
        let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as libc::c_int;
        // SAFETY: the buffer is `size` bytes.
        let filled = unsafe {
            libc::proc_pidinfo(
                pid as libc::c_int,
                libc::PROC_PIDVNODEPATHINFO,
                0,
                &mut info as *mut _ as *mut libc::c_void,
                size,
            )
        };
        if filled != size {
            return None;
        }
        // SAFETY: `vip_path` is `MAXPATHLEN` bytes, declared as nested arrays.
        let path = unsafe {
            std::slice::from_raw_parts(
                info.pvi_cdir.vip_path.as_ptr() as *const u8,
                libc::MAXPATHLEN as usize,
            )
        };
        let end = path.iter().position(|&byte| byte == 0)?;
        (end > 0).then(|| std::ffi::OsStr::from_bytes(&path[..end]).into())
    }

    /// Every process in the session (herdr's `session_processes`).
    pub(crate) fn session_processes(session_id: u32) -> Vec<u32> {
        // SAFETY: a null buffer asks for the count.
        let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
        let mut capacity = if count > 0 {
            count as usize + 128
        } else {
            4096
        };
        for _ in 0..8 {
            let mut pids = vec![0 as libc::pid_t; capacity];
            // SAFETY: the buffer holds `capacity` pids, and is told its size in bytes.
            let count = unsafe {
                libc::proc_listallpids(
                    pids.as_mut_ptr() as *mut libc::c_void,
                    (pids.len() * std::mem::size_of::<libc::pid_t>()) as libc::c_int,
                )
            };
            if count <= 0 {
                return Vec::new();
            }
            let count = count as usize;
            if count < capacity {
                return pids
                    .into_iter()
                    .take(count)
                    .filter(|pid| *pid > 0)
                    // SAFETY: `getsid` only reads.
                    .filter(|pid| unsafe { libc::getsid(*pid) } == session_id as libc::pid_t)
                    .map(|pid| pid as u32)
                    .collect();
            }
            capacity = capacity.saturating_mul(2);
        }
        Vec::new()
    }

    fn kern_procargs2(pid: u32) -> Option<Vec<u8>> {
        let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as libc::c_int];
        let mut size: libc::size_t = 0;
        // SAFETY: a null buffer asks for the size.
        let status = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                std::ptr::null_mut(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if status != 0 || size == 0 {
            return None;
        }
        let mut buffer = vec![0u8; size];
        // SAFETY: the buffer is `size` bytes, and `size` says so.
        let status = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                buffer.as_mut_ptr() as *mut libc::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if status != 0 {
            return None;
        }
        buffer.truncate(size);
        Some(buffer)
    }

    /// `argv` from `KERN_PROCARGS2`'s layout: `argc`, the executable's path, NUL padding,
    /// then the arguments.
    fn procargs2_argv(buffer: &[u8]) -> Option<Vec<String>> {
        let argc = i32::from_ne_bytes(buffer.get(..4)?.try_into().ok()?);
        if argc < 1 {
            return None;
        }
        let rest = &buffer[4..];
        let exec_end = rest.iter().position(|&byte| byte == 0)?;
        let mut current = exec_end;
        while current < rest.len() && rest[current] == 0 {
            current += 1;
        }
        let mut argv = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            if current >= rest.len() {
                return None;
            }
            let end = rest[current..]
                .iter()
                .position(|&byte| byte == 0)
                .map_or(rest.len(), |offset| current + offset);
            if end == current {
                return None;
            }
            argv.push(String::from_utf8_lossy(&rest[current..end]).into_owned());
            current = end + 1;
        }
        Some(argv)
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::ForegroundProcess;

    /// Fields of `/proc/<pid>/stat` after `(comm)`, which may itself hold spaces and parens.
    fn stat_fields(pid: u32) -> Option<(String, Vec<String>)> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let close = stat.rfind(')')?;
        let comm = stat.get(stat.find('(')? + 1..close)?.to_string();
        let fields = stat
            .get(close + 2..)?
            .split_whitespace()
            .map(str::to_string)
            .collect();
        Some((comm, fields))
    }

    /// The foreground process group of `pid`'s controlling terminal (`tpgid`).
    pub(crate) fn foreground_process_group_id(pid: u32) -> Option<u32> {
        let (_, fields) = stat_fields(pid)?;
        let group: i32 = fields.get(5)?.parse().ok()?;
        (group > 0).then_some(group as u32)
    }

    pub(crate) fn group_leader(process_group_id: u32) -> Option<ForegroundProcess> {
        let (name, fields) = stat_fields(process_group_id)?;
        let group: u32 = fields.get(2)?.parse().ok()?;
        if group != process_group_id {
            return None;
        }
        let argv = std::fs::read(format!("/proc/{process_group_id}/cmdline"))
            .map(|bytes| {
                bytes
                    .split(|&byte| byte == 0)
                    .filter(|part| !part.is_empty())
                    .map(|part| String::from_utf8_lossy(part).into_owned())
                    .collect()
            })
            .unwrap_or_default();
        Some(ForegroundProcess::new(name, argv))
    }

    /// A process's working directory (herdr's `process_cwd`).
    pub(crate) fn process_cwd(pid: u32) -> Option<std::path::PathBuf> {
        std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
    }

    /// Every process in the session (herdr's `session_processes`).
    pub(crate) fn session_processes(session_id: u32) -> Vec<u32> {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
            .filter(|pid| {
                stat_fields(*pid).and_then(|(_, fields)| fields.get(3)?.parse::<u32>().ok())
                    == Some(session_id)
            })
            .collect()
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod platform {
    use super::ForegroundProcess;

    pub(crate) fn foreground_process_group_id(_pid: u32) -> Option<u32> {
        None
    }

    pub(crate) fn group_leader(_process_group_id: u32) -> Option<ForegroundProcess> {
        None
    }

    pub(crate) fn process_cwd(_pid: u32) -> Option<std::path::PathBuf> {
        None
    }

    pub(crate) fn session_processes(_session_id: u32) -> Vec<u32> {
        Vec::new()
    }
}

pub(crate) use platform::{
    foreground_process_group_id, group_leader, process_cwd, session_processes,
};

/// Ends processes as herdr ends a closed pane's: hang up, then terminate, then kill, giving
/// each a moment, until none is left. Blocks for up to three quarters of a second.
#[cfg(unix)]
pub(crate) fn end_processes(pids: Vec<u32>) {
    const GRACE: std::time::Duration = std::time::Duration::from_millis(250);
    let is_alive = |pid: u32| {
        // SAFETY: signal 0 only checks that the process exists and may be signalled.
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    };
    for signal in [libc::SIGHUP, libc::SIGTERM, libc::SIGKILL] {
        let alive: Vec<u32> = pids.iter().copied().filter(|pid| is_alive(*pid)).collect();
        if alive.is_empty() {
            return;
        }
        for pid in &alive {
            // SAFETY: sends a signal; a process that's gone or not ours just refuses it.
            unsafe { libc::kill(*pid as libc::pid_t, signal) };
        }
        let deadline = std::time::Instant::now() + GRACE;
        while std::time::Instant::now() < deadline && alive.iter().any(|pid| is_alive(*pid)) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    let left: Vec<u32> = pids.into_iter().filter(|pid| is_alive(*pid)).collect();
    if !left.is_empty() {
        log::warn!("processes still running after their terminal closed: {left:?}");
    }
}
