use std::{
    fs, io,
    path::Path,
    process::{Child, Command},
    thread,
    time::Duration,
};

pub(crate) const PARENT_ENV: &str = "PIXELCODE_PARENT";
const OWNER_FILE: &str = "process-owner";
const TERM_GRACE: Duration = Duration::from_secs(2);

#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub(crate) fn configure(command: &mut Command) {
    command.env(PARENT_ENV, std::process::id().to_string());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
}

pub(crate) fn spawn(command: &mut Command) -> io::Result<Child> {
    configure(command);
    command.spawn()
}

pub(crate) fn terminate(pids: &[u32]) {
    if pids.is_empty() {
        return;
    }
    platform::terminate(pids);
}

#[cfg(test)]
pub(crate) fn process_exists(pid: u32) -> bool {
    platform::process_exists(pid)
}

pub(crate) fn reap_orphans(data_dir: &Path) {
    let previous = read_owner(data_dir);
    platform::reap(previous);
    write_owner(data_dir, std::process::id());
}

fn read_owner(data_dir: &Path) -> Option<u32> {
    fs::create_dir_all(data_dir).ok()?;
    fs::read_to_string(data_dir.join(OWNER_FILE))
        .ok()
        .and_then(|value| value.trim().parse().ok())
}

fn write_owner(data_dir: &Path, pid: u32) {
    let _ = fs::create_dir_all(data_dir);
    let _ = fs::write(data_dir.join(OWNER_FILE), pid.to_string());
}

#[cfg(unix)]
mod platform {
    use super::*;
    use std::collections::HashSet;

    pub fn terminate(pids: &[u32]) {
        for &pid in pids {
            unsafe {
                libc::killpg(pid as libc::pid_t, libc::SIGTERM);
            }
        }
        thread::sleep(TERM_GRACE);
        for &pid in pids {
            unsafe {
                libc::killpg(pid as libc::pid_t, libc::SIGKILL);
            }
        }
    }

    pub fn process_exists(pid: u32) -> bool {
        if unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
            return true;
        }
        io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }

    pub fn reap(previous: Option<u32>) {
        #[cfg(target_os = "linux")]
        reap_linux(previous);
        #[cfg(not(target_os = "linux"))]
        let _ = previous;
    }

    #[cfg(target_os = "linux")]
    fn reap_linux(previous: Option<u32>) {
        let _ = previous;
        let current = std::process::id();
        let current_group = unsafe { libc::getpgrp() } as u32;
        let mut groups = HashSet::new();
        let Ok(entries) = fs::read_dir("/proc") else {
            return;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else {
                continue;
            };
            let Ok(pid) = name.parse::<u32>() else {
                continue;
            };
            if pid == current {
                continue;
            }
            let Ok(bytes) = fs::read(entry.path().join("environ")) else {
                continue;
            };
            let Some(marker) = parent_marker(&bytes) else {
                continue;
            };
            if marker == current || process_exists(marker) {
                continue;
            }
            let pgid = unsafe { libc::getpgid(pid as libc::pid_t) };
            if pgid <= 1 {
                continue;
            }
            let pgid = pgid as u32;
            if pgid == current || pgid == current_group {
                continue;
            }
            groups.insert(pgid);
        }
        let groups: Vec<u32> = groups.into_iter().collect();
        if groups.is_empty() {
            return;
        }
        for &pgid in &groups {
            unsafe {
                libc::killpg(pgid as libc::pid_t, libc::SIGTERM);
            }
        }
        let deadline = std::time::Instant::now() + TERM_GRACE;
        while std::time::Instant::now() < deadline && groups.iter().any(|&pgid| group_exists(pgid)) {
            thread::sleep(Duration::from_millis(50));
        }
        for pgid in groups {
            if group_exists(pgid) {
                unsafe {
                    libc::killpg(pgid as libc::pid_t, libc::SIGKILL);
                }
            }
        }
    }

    fn parent_marker(bytes: &[u8]) -> Option<u32> {
        const KEY: &[u8] = b"PIXELCODE_PARENT=";
        bytes.split(|byte| *byte == 0).find_map(|part| {
            part.strip_prefix(KEY).and_then(|value| {
                std::str::from_utf8(value)
                    .ok()
                    .and_then(|value| value.parse().ok())
            })
        })
    }

    #[cfg(target_os = "linux")]
    fn group_exists(pgid: u32) -> bool {
        if unsafe { libc::killpg(pgid as libc::pid_t, 0) } == 0 {
            return true;
        }
        io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::process::Stdio;

    pub fn terminate(pids: &[u32]) {
        for &pid in pids {
            taskkill(pid, false);
        }
        thread::sleep(TERM_GRACE);
        for &pid in pids {
            taskkill(pid, true);
        }
    }

    pub fn process_exists(pid: u32) -> bool {
        let Ok(output) = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .stdin(Stdio::null())
            .output()
        else {
            return true;
        };
        let needle = pid.to_string();
        String::from_utf8_lossy(&output.stdout).lines().any(|line| {
            line.split(',')
                .nth(1)
                .is_some_and(|field| field.trim_matches('"') == needle)
        })
    }

    pub fn reap(previous: Option<u32>) {
        let Some(previous) = previous else {
            return;
        };
        if process_exists(previous) {
            return;
        }
        let descendants = descendant_pids(previous);
        if descendants.is_empty() {
            return;
        }
        terminate(&descendants);
    }

    fn taskkill(pid: u32, force: bool) {
        let mut command = Command::new("taskkill");
        command
            .arg("/PID")
            .arg(pid.to_string())
            .arg("/T")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if force {
            command.arg("/F");
        }
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
        let _ = command.status();
    }

    fn descendant_pids(root: u32) -> Vec<u32> {
        let script = "Get-CimInstance Win32_Process | ForEach-Object { \"$($_.ProcessId),$($_.ParentProcessId)\" }";
        let Ok(output) = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .stdin(Stdio::null())
            .output()
        else {
            return Vec::new();
        };
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Some((pid, parent)) = line.split_once(',') else {
                continue;
            };
            let (Ok(pid), Ok(parent)) = (pid.trim().parse::<u32>(), parent.trim().parse::<u32>())
            else {
                continue;
            };
            children.entry(parent).or_default().push(pid);
        }
        let current = std::process::id();
        let mut visited = HashSet::from([root, current]);
        let mut queue = VecDeque::from([root]);
        let mut result = Vec::new();
        while let Some(pid) = queue.pop_front() {
            for &child in children.get(&pid).into_iter().flatten() {
                if visited.insert(child) {
                    result.push(child);
                    queue.push_back(child);
                }
            }
        }
        result
    }
}
