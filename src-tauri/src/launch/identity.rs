use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

const RUN_MARKER_PREFIX: &str = "-Dbasalt.running_id=";

pub fn run_marker(running_id: &str) -> String {
    format!("{RUN_MARKER_PREFIX}{running_id}")
}

#[derive(Debug, Clone)]
pub enum Identity {
    Marker(String),
    Executable(std::path::PathBuf),
}

impl Identity {
    pub fn marker(running_id: &str) -> Self {
        Identity::Marker(running_id.to_string())
    }

    pub fn executable(path: impl Into<std::path::PathBuf>) -> Self {
        Identity::Executable(path.into())
    }
}

fn refresh_process(system: &mut System, pid: Pid) {
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        false,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_exe(UpdateKind::Always),
    );
}

fn command_has_marker(command: &[std::ffi::OsString], running_id: &str) -> bool {
    let expected = run_marker(running_id);
    command.iter().any(|argument| argument == expected.as_str())
}

fn is_the_same_process(process: &sysinfo::Process, identity: &Identity) -> bool {
    match identity {
        Identity::Marker(running_id) => command_has_marker(process.cmd(), running_id),
        Identity::Executable(path) => process.exe().is_some_and(|actual| actual == path),
    }
}

fn identity_matches(
    actual_started_at: u64,
    process: &sysinfo::Process,
    expected_started_at: u64,
    identity: &Identity,
) -> bool {
    actual_started_at == expected_started_at && is_the_same_process(process, identity)
}

pub fn process_matches(pid: u32, process_started_at: u64, identity: &Identity) -> bool {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    refresh_process(&mut system, pid);
    system.process(pid).is_some_and(|process| {
        identity_matches(process.start_time(), process, process_started_at, identity)
    })
}

pub fn process_start(pid: u32) -> Option<u64> {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    refresh_process(&mut system, pid);
    system.process(pid).map(sysinfo::Process::start_time)
}

pub fn descendant_java(root: u32) -> Option<u32> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );

    let mut best: Option<(u32, u64)> = None;
    for (pid, process) in system.processes() {
        let name = process
            .exe()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if !name.starts_with("java") {
            continue;
        }
        if !descends_from(&system, *pid, Pid::from_u32(root)) {
            continue;
        }
        let started = process.start_time();
        if best.is_none_or(|(_, seen)| started < seen) {
            best = Some((pid.as_u32(), started));
        }
    }
    best.map(|(pid, _)| pid)
}

fn descends_from(system: &System, mut pid: Pid, root: Pid) -> bool {
    for _ in 0..16 {
        if pid == root {
            return true;
        }
        let Some(parent) = system.process(pid).and_then(sysinfo::Process::parent) else {
            return false;
        };
        pid = parent;
    }
    false
}

pub fn kill_tree(root: u32) -> bool {
    let root = Pid::from_u32(root);
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let descendants = system
        .processes()
        .keys()
        .copied()
        .filter(|pid| *pid != root && descends_from(&system, *pid, root))
        .collect::<Vec<_>>();

    let mut killed = system.process(root).is_some_and(sysinfo::Process::kill);
    for pid in descendants {
        killed |= system.process(pid).is_some_and(sysinfo::Process::kill);
    }
    killed
}

pub fn spawned_process_start(pid: u32, identity: &Identity) -> Option<u64> {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    refresh_process(&mut system, pid);
    let process = system.process(pid)?;
    is_the_same_process(process, identity).then(|| process.start_time())
}

pub fn kill_recovered_process(pid: u32, process_started_at: u64, identity: &Identity) -> bool {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    refresh_process(&mut system, pid);
    system.process(pid).is_some_and(|process| {
        process.start_time() == process_started_at
            && is_the_same_process(process, identity)
            && process.kill()
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{command_has_marker, run_marker};

    #[test]
    fn marker_matches_only_the_expected_run() {
        let args = vec![
            OsString::from("-Xmx4G"),
            OsString::from(run_marker("run-1")),
            OsString::from("net.minecraft.client.main.Main"),
        ];
        assert!(command_has_marker(&args, "run-1"));
        assert!(!command_has_marker(&args, "run-2"));
    }
}
