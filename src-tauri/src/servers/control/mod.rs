use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub mod client;
pub mod launch;
pub mod supervise;

pub const FLAG: &str = "--supervise";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Hello { token: String },
    Send { line: String },
    Stop,
    Kill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Ready { pid: u32, started_at: u64 },
    Process { pid: u32, started_at: u64 },
    Log { stream: String, line: String },
    Exit { code: Option<i32> },
    Refused { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Control {
    pub port: u16,
    pub token: String,
    pub supervisor_pid: u32,
    pub child_pid: u32,
    pub child_started_at: u64,
}

pub fn control_path(root: &Path, server_id: &str) -> PathBuf {
    root.join(".control").join(format!("{server_id}.json"))
}

pub fn read_control(path: &Path) -> Option<Control> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

pub fn write_control(path: &Path, control: &Control) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec(control)?)?;
    restrict(path);
    Ok(())
}

#[cfg(unix)]
fn restrict(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict(_path: &Path) {}

pub fn detach(command: &mut std::process::Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
}

pub fn forget_control(path: &Path) {
    let _ = std::fs::remove_file(path);
}

pub fn encode<T: Serialize>(value: &T) -> Result<String> {
    let mut line = serde_json::to_string(value)?;
    line.push('\n');
    Ok(line)
}

pub fn decode<T: serde::de::DeserializeOwned>(line: &str) -> Result<T> {
    serde_json::from_str(line.trim_end()).map_err(|error| Error::other(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_survives_the_round_trip() {
        let line = encode(&Request::Send {
            line: "say hello".to_string(),
        })
        .unwrap();
        assert!(line.ends_with('\n'));
        assert!(!line.trim_end().contains('\n'));

        let back: Request = decode(&line).unwrap();
        assert!(matches!(back, Request::Send { line } if line == "say hello"));
    }

    #[test]
    fn a_log_line_with_newlines_in_it_stays_one_frame() {
        let line = encode(&Event::Log {
            stream: "stdout".to_string(),
            line: "first\nsecond".to_string(),
        })
        .unwrap();
        assert_eq!(line.matches('\n').count(), 1);

        let back: Event = decode(&line).unwrap();
        match back {
            Event::Log { line, .. } => assert_eq!(line, "first\nsecond"),
            other => panic!("expected a log line, got {other:?}"),
        }
    }

    #[test]
    fn a_supervisor_can_promote_the_shell_to_the_server_process() {
        let line = encode(&Event::Process {
            pid: 42,
            started_at: 1234,
        })
        .unwrap();
        let back: Event = decode(&line).unwrap();
        assert!(matches!(
            back,
            Event::Process {
                pid: 42,
                started_at: 1234
            }
        ));
    }

    #[test]
    fn nonsense_is_refused_rather_than_guessed_at() {
        assert!(decode::<Request>("not json").is_err());
        assert!(decode::<Request>(r#"{"cmd":"detonate"}"#).is_err());
    }

    #[test]
    fn the_control_file_round_trips() {
        let dir = std::env::temp_dir().join(format!("basalt-control-{}", uuid::Uuid::new_v4()));
        let path = control_path(&dir, "s1");
        let control = Control {
            port: 40404,
            token: "abc".to_string(),
            supervisor_pid: 10,
            child_pid: 11,
            child_started_at: 12,
        };

        write_control(&path, &control).unwrap();
        let back = read_control(&path).unwrap();
        assert_eq!(back.port, 40404);
        assert_eq!(back.child_pid, 11);

        forget_control(&path);
        assert!(read_control(&path).is_none());
        std::fs::remove_dir_all(dir).ok();
    }
}
