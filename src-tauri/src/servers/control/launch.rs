use std::{path::Path, process::Command, time::Duration};

use crate::error::{Error, Result};

use super::{client::Session, control_path, forget_control, read_control, FLAG};

const READY_TIMEOUT: Duration = Duration::from_secs(70);
const POLL: Duration = Duration::from_millis(50);

pub enum Outcome {
    Supervised(Session),
    Unavailable(String),
}

pub fn command(
    exe: &Path,
    server_id: &str,
    root: &Path,
    dir: &Path,
    program: &Path,
    arguments: &[String],
    through_shell: bool,
) -> Command {
    let mut command = Command::new(exe);
    command.arg(FLAG);
    if through_shell {
        command.arg("--shell");
    }
    command
        .arg("--id")
        .arg(server_id)
        .arg("--root")
        .arg(root)
        .arg("--dir")
        .arg(dir)
        .arg("--")
        .arg(program)
        .args(arguments)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .current_dir(dir);
    super::detach(&mut command);
    command
}

pub async fn start(
    server_id: &str,
    root: &Path,
    dir: &Path,
    program: &Path,
    arguments: &[String],
    through_shell: bool,
) -> Result<Outcome> {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => return Ok(Outcome::Unavailable(error.to_string())),
    };

    let path = control_path(root, server_id);
    if let Some(control) = read_control(&path) {
        if super::client::still_running(&control) {
            return Err(Error::other(
                "This server already has a live supervisor. Reconnect to it instead of starting another copy.",
            ));
        }
    }
    forget_control(&path);

    let mut child = match command(
        &exe,
        server_id,
        root,
        dir,
        program,
        arguments,
        through_shell,
    )
    .spawn()
    {
        Ok(child) => child,
        Err(error) => return Ok(Outcome::Unavailable(error.to_string())),
    };

    let deadline = tokio::time::Instant::now() + READY_TIMEOUT;
    loop {
        if let Some(control) = read_control(&path) {
            reap(child);
            let session = Session::connect(&control).await?;
            return Ok(Outcome::Supervised(session));
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                return Ok(Outcome::Unavailable(format!(
                    "the supervisor exited with {status} before starting anything"
                )));
            }
            Ok(None) => {}
            Err(error) => return Ok(Outcome::Unavailable(error.to_string())),
        }

        if tokio::time::Instant::now() >= deadline {
            crate::launch::identity::kill_tree(child.id());
            reap(child);
            return Err(Error::other(
                "Basalt could not reach the server it just started. Force stop it and try again.",
            ));
        }
        tokio::time::sleep(POLL).await;
    }
}

fn reap(mut child: std::process::Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}
