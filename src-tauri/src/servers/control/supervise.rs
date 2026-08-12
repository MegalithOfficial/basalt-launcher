use std::{
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Sender},
        Arc, Mutex,
    },
};

use super::{control_path, encode, forget_control, write_control, Control, Event, Request};

pub struct Args {
    pub server_id: String,
    pub root: PathBuf,
    pub dir: PathBuf,
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub through_shell: bool,
    pub java_path: Option<PathBuf>,
}

pub fn parse_args(raw: &[String]) -> Option<Args> {
    let start = raw.iter().position(|entry| entry == super::FLAG)? + 1;
    let split = raw.iter().position(|entry| entry == "--")?;
    let flags = &raw[start..split];
    let command = &raw[split + 1..];

    let value = |name: &str| {
        flags
            .iter()
            .position(|entry| entry == name)
            .and_then(|at| flags.get(at + 1))
            .cloned()
    };

    let program = command.first()?;
    Some(Args {
        server_id: value("--id")?,
        root: PathBuf::from(value("--root")?),
        dir: PathBuf::from(value("--dir")?),
        program: PathBuf::from(program),
        arguments: command[1..].to_vec(),
        through_shell: flags.iter().any(|entry| entry == "--shell"),
        java_path: value("--java").map(PathBuf::from),
    })
}

fn wait_for_java(shell: u32) -> Option<u32> {
    while crate::launch::identity::process_start(shell).is_some() {
        if let Some(found) = crate::launch::identity::descendant_server_java(shell) {
            return Some(found);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    None
}

fn started_at(pid: u32) -> u64 {
    crate::launch::identity::process_start(pid).unwrap_or(0)
}

pub fn run(args: Args) -> ! {
    let listener = match TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("basalt supervisor could not listen: {error}");
            std::process::exit(1);
        }
    };
    let port = listener
        .local_addr()
        .map(|address| address.port())
        .unwrap_or(0);
    let token = uuid::Uuid::new_v4().to_string();

    let mut command = Command::new(&args.program);
    command.args(&args.arguments).current_dir(&args.dir);
    if let Some(java_path) = &args.java_path {
        if let Some(bin) = java_path.parent() {
            let mut paths = vec![bin.to_path_buf()];
            if let Some(existing) = std::env::var_os("PATH") {
                paths.extend(std::env::split_paths(&existing));
            }
            if let Ok(path) = std::env::join_paths(paths) {
                command.env("PATH", path);
            }
            if let Some(home) = bin.parent() {
                command.env("JAVA_HOME", home);
            }
        }
    }
    let child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match child {
        Ok(child) => child,
        Err(error) => {
            eprintln!("basalt supervisor could not start the server: {error}");
            std::process::exit(1);
        }
    };

    let shell_pid = child.id();
    let path = control_path(&args.root, &args.server_id);
    let control = Control {
        port,
        token: token.clone(),
        supervisor_pid: std::process::id(),
        child_pid: shell_pid,
        child_started_at: started_at(shell_pid),
    };
    if let Err(error) = write_control(&path, &control) {
        eprintln!("basalt supervisor could not record its control file: {error}");
        crate::launch::identity::kill_tree(shell_pid);
        let _ = child.wait();
        std::process::exit(1);
    }

    let stdin = Arc::new(Mutex::new(child.stdin.take()));
    let clients: Arc<Mutex<Vec<Sender<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let done = Arc::new(AtomicBool::new(false));
    let process = Arc::new(Mutex::new((control.child_pid, control.child_started_at)));

    pump(child.stdout.take(), "stdout", clients.clone());
    pump(child.stderr.take(), "stderr", clients.clone());

    {
        let clients = clients.clone();
        let done = done.clone();
        let stdin = stdin.clone();
        let process = process.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if done.load(Ordering::SeqCst) {
                    return;
                }
                serve(
                    stream,
                    token.clone(),
                    process.clone(),
                    stdin.clone(),
                    clients.clone(),
                    shell_pid,
                );
            }
        });
    }

    if args.through_shell {
        let path = path.clone();
        let clients = clients.clone();
        let process = process.clone();
        std::thread::spawn(move || {
            let Some(java_pid) = wait_for_java(shell_pid) else {
                return;
            };
            let java_started_at = started_at(java_pid);
            *process.lock().unwrap() = (java_pid, java_started_at);
            let mut updated = control;
            updated.child_pid = java_pid;
            updated.child_started_at = java_started_at;
            if let Err(error) = write_control(&path, &updated) {
                eprintln!("basalt supervisor could not update its control file: {error}");
            }
            broadcast(
                &clients,
                &encode(&Event::Process {
                    pid: java_pid,
                    started_at: java_started_at,
                })
                .unwrap_or_default(),
            );
            while crate::launch::identity::process_start(java_pid).is_some() {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            crate::launch::identity::kill_tree(shell_pid);
        });
    }

    let status = child.wait();
    done.store(true, Ordering::SeqCst);
    let code = status.ok().and_then(|status| status.code());
    broadcast(&clients, &encode(&Event::Exit { code }).unwrap_or_default());
    forget_control(&path);
    std::process::exit(code.unwrap_or(0));
}

fn pump<R: std::io::Read + Send + 'static>(
    source: Option<R>,
    stream: &'static str,
    clients: Arc<Mutex<Vec<Sender<String>>>>,
) {
    let Some(source) = source else {
        return;
    };
    std::thread::spawn(move || {
        for line in BufReader::new(source)
            .lines()
            .map_while(std::result::Result::ok)
        {
            let frame = encode(&Event::Log {
                stream: stream.to_string(),
                line,
            })
            .unwrap_or_default();
            broadcast(&clients, &frame);
        }
    });
}

fn broadcast(clients: &Arc<Mutex<Vec<Sender<String>>>>, frame: &str) {
    let mut held = clients.lock().unwrap();
    held.retain(|client| client.send(frame.to_string()).is_ok());
}

fn serve(
    stream: TcpStream,
    token: String,
    process: Arc<Mutex<(u32, u64)>>,
    stdin: Arc<Mutex<Option<std::process::ChildStdin>>>,
    clients: Arc<Mutex<Vec<Sender<String>>>>,
    shell_pid: u32,
) {
    let Ok(reading) = stream.try_clone() else {
        return;
    };
    let (sender, receiver) = channel::<String>();

    std::thread::spawn(move || {
        let mut writing = stream;
        while let Ok(frame) = receiver.recv() {
            if writing.write_all(frame.as_bytes()).is_err() || writing.flush().is_err() {
                return;
            }
        }
    });

    std::thread::spawn(move || {
        let mut lines = BufReader::new(reading).lines();
        let greeting = lines.next().and_then(std::result::Result::ok);
        let allowed = greeting
            .as_deref()
            .and_then(|line| super::decode::<Request>(line).ok())
            .is_some_and(
                |request| matches!(request, Request::Hello { token: given } if given == token),
            );
        if !allowed {
            let _ = sender.send(
                encode(&Event::Refused {
                    reason: "wrong token".to_string(),
                })
                .unwrap_or_default(),
            );
            return;
        }

        let (pid, started_at) = *process.lock().unwrap();
        let _ = sender.send(encode(&Event::Ready { pid, started_at }).unwrap_or_default());
        clients.lock().unwrap().push(sender);

        for line in lines.map_while(std::result::Result::ok) {
            let Ok(request) = super::decode::<Request>(&line) else {
                continue;
            };
            match request {
                Request::Hello { .. } => {}
                Request::Send { line } => write_line(&stdin, &line),
                Request::Stop => write_line(&stdin, "stop"),
                Request::Kill => {
                    crate::launch::identity::kill_tree(shell_pid);
                }
            }
        }
    });
}

fn write_line(stdin: &Arc<Mutex<Option<std::process::ChildStdin>>>, line: &str) {
    let mut held = stdin.lock().unwrap();
    let Some(pipe) = held.as_mut() else {
        return;
    };
    let _ = pipe.write_all(line.as_bytes());
    let _ = pipe.write_all(b"\n");
    let _ = pipe.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|entry| (*entry).to_string()).collect()
    }

    #[test]
    fn the_command_after_the_separator_is_taken_whole() {
        let parsed = parse_args(&args(&[
            "basalt",
            "--supervise",
            "--id",
            "s1",
            "--root",
            "/data/servers",
            "--dir",
            "/srv/smp",
            "--java",
            "/opt/java/bin/java",
            "--",
            "/usr/bin/java",
            "-Xmx4G",
            "-jar",
            "server.jar",
            "nogui",
        ]))
        .unwrap();

        assert_eq!(parsed.server_id, "s1");
        assert_eq!(parsed.dir, PathBuf::from("/srv/smp"));
        assert_eq!(parsed.program, PathBuf::from("/usr/bin/java"));
        assert_eq!(parsed.arguments, ["-Xmx4G", "-jar", "server.jar", "nogui"]);
        assert_eq!(parsed.java_path, Some(PathBuf::from("/opt/java/bin/java")));
    }

    #[test]
    fn a_normal_launch_is_not_mistaken_for_a_supervisor_one() {
        assert!(parse_args(&args(&["basalt"])).is_none());
        assert!(parse_args(&args(&["basalt", "--launch", "abc"])).is_none());
        assert!(parse_args(&args(&["basalt", "--supervise", "--id", "s1"])).is_none());
    }
}
