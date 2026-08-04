use std::{
    io::{Read, Write},
    path::PathBuf,
    time::Duration,
};

use serde_json::{json, Value};

use crate::error::{Error, Result};

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;
const OP_PING: u32 = 3;
const OP_PONG: u32 = 4;

const IO_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_FRAME_BYTES: usize = 64 * 1024;
const SOCKET_SLOTS: u8 = 10;

#[cfg(unix)]
type Transport = std::os::unix::net::UnixStream;
#[cfg(windows)]
type Transport = std::fs::File;

#[cfg(unix)]
const NESTED_DIRS: [&str; 5] = [
    "",
    "app/com.discordapp.Discord",
    "app/com.discordapp.DiscordCanary",
    "snap.discord",
    "snap.discord-canary",
];

#[cfg(unix)]
fn socket_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for key in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() {
                roots.push(PathBuf::from(value));
            }
        }
    }
    roots.push(PathBuf::from("/tmp"));
    roots
}

#[cfg(unix)]
fn open_transport() -> Result<Transport> {
    for root in socket_roots() {
        for nested in NESTED_DIRS {
            let dir = if nested.is_empty() {
                root.clone()
            } else {
                root.join(nested)
            };
            for slot in 0..SOCKET_SLOTS {
                let path = dir.join(format!("discord-ipc-{slot}"));
                if let Ok(stream) = Transport::connect(&path) {
                    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
                    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
                    tracing::debug!(path = %path.display(), "opened the discord socket");
                    return Ok(stream);
                }
            }
        }
    }
    Err(Error::other("Discord is not running."))
}

#[cfg(windows)]
fn open_transport() -> Result<Transport> {
    for prefix in [r"\\?\pipe", r"\\.\pipe"] {
        for slot in 0..SOCKET_SLOTS {
            let path = format!(r"{prefix}\discord-ipc-{slot}");
            if let Ok(pipe) = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
            {
                tracing::debug!(path, "opened the discord pipe");
                return Ok(pipe);
            }
        }
    }
    Err(Error::other("Discord is not running."))
}

fn reason(payload: &Value, fallback: &str) -> String {
    let data = payload.get("data").unwrap_or(payload);
    let message = data
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or(fallback);
    match data.get("code").and_then(Value::as_i64) {
        Some(code) => format!("{message} (code {code})"),
        None => message.to_string(),
    }
}

pub struct Connection {
    transport: Transport,
    nonce: u64,
}

impl Connection {
    pub fn open(app_id: &str) -> Result<Self> {
        let mut connection = Self {
            transport: open_transport()?,
            nonce: 0,
        };
        connection.send(OP_HANDSHAKE, json!({ "v": 1, "client_id": app_id }))?;

        let (op, payload) = connection.receive()?;
        if op == OP_CLOSE {
            return Err(Error::other(reason(
                &payload,
                "Discord refused the application id.",
            )));
        }
        if payload.get("evt").and_then(Value::as_str) != Some("READY") {
            return Err(Error::other(reason(
                &payload,
                "Discord did not accept the handshake.",
            )));
        }
        Ok(connection)
    }

    pub fn set_activity(&mut self, activity: Value) -> Result<()> {
        self.command(json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": std::process::id(), "activity": activity },
        }))
    }

    pub fn clear_activity(&mut self) -> Result<()> {
        self.command(json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": std::process::id(), "activity": Value::Null },
        }))
    }

    pub fn close(&mut self) {
        let _ = self.send(OP_CLOSE, json!({}));
    }

    fn command(&mut self, mut body: Value) -> Result<()> {
        self.nonce += 1;
        body["nonce"] = json!(self.nonce.to_string());
        self.send(OP_FRAME, body)?;

        loop {
            let (op, payload) = self.receive()?;
            match op {
                OP_PING => self.send(OP_PONG, payload)?,
                OP_CLOSE => {
                    return Err(Error::other(reason(
                        &payload,
                        "Discord closed the connection.",
                    )))
                }
                _ => {
                    if payload.get("evt").and_then(Value::as_str) == Some("ERROR") {
                        return Err(Error::other(reason(
                            &payload,
                            "Discord rejected the update.",
                        )));
                    }
                    return Ok(());
                }
            }
        }
    }

    fn send(&mut self, op: u32, payload: Value) -> Result<()> {
        let body = payload.to_string();
        let mut frame = Vec::with_capacity(8 + body.len());
        frame.extend_from_slice(&op.to_le_bytes());
        frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
        frame.extend_from_slice(body.as_bytes());
        self.transport.write_all(&frame)?;
        self.transport.flush()?;
        Ok(())
    }

    fn receive(&mut self) -> Result<(u32, Value)> {
        let mut header = [0u8; 8];
        self.transport.read_exact(&mut header)?;
        let op = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        if length > MAX_FRAME_BYTES {
            return Err(Error::other("Discord sent an oversized frame."));
        }
        let mut body = vec![0u8; length];
        self.transport.read_exact(&mut body)?;
        let payload = serde_json::from_slice(&body).unwrap_or(Value::Null);
        Ok((op, payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_carries_a_little_endian_opcode_and_length() {
        let body = json!({ "v": 1 }).to_string();
        let mut frame = Vec::new();
        frame.extend_from_slice(&OP_HANDSHAKE.to_le_bytes());
        frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
        frame.extend_from_slice(body.as_bytes());

        assert_eq!(&frame[0..4], &[0, 0, 0, 0]);
        assert_eq!(
            u32::from_le_bytes(frame[4..8].try_into().unwrap()) as usize,
            body.len()
        );
    }

    #[test]
    fn an_error_reply_reports_the_code_and_message() {
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "evt": "ERROR",
            "data": { "code": 4006, "message": "Not authenticated" },
        });
        assert_eq!(
            reason(&payload, "fallback"),
            "Not authenticated (code 4006)"
        );
    }

    #[test]
    #[ignore = "talks to a running discord, run with BASALT_PROBE_APP_ID set"]
    fn probe_a_live_discord() {
        let app_id = std::env::var("BASALT_PROBE_APP_ID").expect("BASALT_PROBE_APP_ID");
        let mut connection = match Connection::open(&app_id) {
            Ok(connection) => connection,
            Err(error) => panic!("handshake rejected: {error}"),
        };

        let started_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let large_image =
            std::env::var("BASALT_PROBE_IMAGE").unwrap_or_else(|_| "basalt".to_string());
        println!("large_image: {large_image}");
        let activity = json!({
            "type": 0,
            "name": "Minecraft",
            "details": "probe",
            "state": "icon test",
            "timestamps": { "start": started_at * 1000 },
            "assets": { "large_image": large_image, "large_text": "Fabulously Optimized" },
        });
        if let Err(error) = connection.set_activity(activity) {
            panic!("activity rejected: {error}");
        }

        let hold = std::env::var("BASALT_PROBE_HOLD_SECS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(20);
        println!("activity accepted, holding {hold}s");
        std::thread::sleep(Duration::from_secs(hold));
        let _ = connection.clear_activity();
        connection.close();
    }

    #[test]
    fn a_reply_without_details_falls_back() {
        assert_eq!(
            reason(&json!({}), "Discord said nothing."),
            "Discord said nothing."
        );
    }
}
