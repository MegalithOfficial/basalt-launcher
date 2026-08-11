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
