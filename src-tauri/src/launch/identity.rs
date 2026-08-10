use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

const RUN_MARKER_PREFIX: &str = "-Dbasalt.running_id=";

pub fn run_marker(running_id: &str) -> String {
    format!("{RUN_MARKER_PREFIX}{running_id}")
}

fn refresh_process(system: &mut System, pid: Pid) {
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        false,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );
}

fn command_has_marker(command: &[std::ffi::OsString], running_id: &str) -> bool {
    let expected = run_marker(running_id);
    command.iter().any(|argument| argument == expected.as_str())
}

fn identity_matches(
    actual_started_at: u64,
    command: &[std::ffi::OsString],
    expected_started_at: u64,
    running_id: &str,
) -> bool {
    actual_started_at == expected_started_at && command_has_marker(command, running_id)
}

pub fn process_matches(pid: u32, process_started_at: u64, running_id: &str) -> bool {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    refresh_process(&mut system, pid);
    system.process(pid).is_some_and(|process| {
        identity_matches(
            process.start_time(),
            process.cmd(),
            process_started_at,
            running_id,
        )
    })
}

pub fn spawned_process_start(pid: u32, running_id: &str) -> Option<u64> {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    refresh_process(&mut system, pid);
    let process = system.process(pid)?;
    command_has_marker(process.cmd(), running_id).then(|| process.start_time())
}

pub fn kill_recovered_process(pid: u32, process_started_at: u64, running_id: &str) -> bool {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    refresh_process(&mut system, pid);
    system.process(pid).is_some_and(|process| {
        process.start_time() == process_started_at
            && command_has_marker(process.cmd(), running_id)
            && process.kill()
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{command_has_marker, identity_matches, run_marker};

    #[test]
    fn marker_matches_only_the_expected_run() {
        let args = vec![
            OsString::from("-Xmx4G"),
            OsString::from(run_marker("run-1")),
            OsString::from("net.minecraft.client.main.Main"),
        ];
        assert!(command_has_marker(&args, "run-1"));
        assert!(!command_has_marker(&args, "run-2"));
        assert!(identity_matches(100, &args, 100, "run-1"));
        assert!(!identity_matches(101, &args, 100, "run-1"));
    }
}
