use std::{
    sync::{
        mpsc::{self, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    time::Duration,
};

use serde_json::{json, Map, Value};

use crate::{build_info, config::LauncherSettings};

mod ipc;

use ipc::Connection;

const IDLE_LARGE_IMAGE: &str = "basalt";
const IDLE_ACTIVITY_NAME: &str = "Basalt";
const ACTIVITY_NAME: &str = "Minecraft";
const HEARTBEAT: Duration = Duration::from_secs(15);
const WATCHDOG_INTERVAL: Duration = Duration::from_secs(30);
const WATCHDOG_REPLY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceActivity {
    pub app_id: String,
    pub instance_name: String,
    pub version_line: Option<String>,
    pub detail_line: Option<String>,
    pub logo_url: Option<String>,
    pub started_at: i64,
}

enum Message {
    Set {
        running_id: String,
        activity: Box<PresenceActivity>,
    },
    Clear {
        running_id: String,
    },
    ClearAll,
    Idle {
        app_id: String,
        line: String,
        started_at: i64,
    },
    Reconnect {
        app_id: String,
        reply: Sender<std::result::Result<(), String>>,
    },
    Ping(Sender<()>),
}

pub struct Presence {
    tx: Arc<Mutex<Sender<Message>>>,
}

#[derive(Default)]
struct SharedState {
    last: Option<Value>,
}

struct Worker {
    client: Option<(String, Connection)>,
    games: Vec<(String, PresenceActivity)>,
    idle: Option<(String, String, i64)>,
}

pub fn app_id(settings: &LauncherSettings) -> Option<String> {
    let configured = settings.discord_app_id.trim();
    if !configured.is_empty() {
        return Some(configured.to_string());
    }
    build_info::bundled_discord_app_id().map(|id| id.to_string())
}

pub fn activity_for(
    settings: &LauncherSettings,
    instance_name: &str,
    version_id: &str,
    loader: Option<&str>,
    logo_url: Option<&str>,
    detail_line: Option<String>,
    started_at: i64,
) -> Option<PresenceActivity> {
    if !settings.discord_rpc {
        return None;
    }
    let app_id = app_id(settings)?;

    let version_line = settings.discord_rpc_show_version.then(|| match loader {
        Some(loader) if !loader.is_empty() => format!("{version_id} · {loader}"),
        _ => version_id.to_string(),
    });
    let detail_line = settings
        .discord_rpc_show_streak
        .then_some(detail_line)
        .flatten();
    let logo_url = settings
        .discord_rpc_show_logo
        .then_some(logo_url)
        .flatten()
        .filter(|url| url.starts_with("http"))
        .map(|url| url.to_string());

    Some(PresenceActivity {
        app_id,
        instance_name: instance_name.to_string(),
        version_line,
        detail_line,
        logo_url,
        started_at,
    })
}

fn build_activity(state: &PresenceActivity) -> Value {
    let mut activity = Map::new();
    activity.insert("type".into(), json!(0));
    activity.insert("name".into(), json!(ACTIVITY_NAME));
    activity.insert("details".into(), json!(state.instance_name));
    if let Some(line) = state.version_line.as_deref() {
        activity.insert("state".into(), json!(line));
    }
    activity.insert(
        "timestamps".into(),
        json!({ "start": state.started_at * 1000 }),
    );

    let mut assets = Map::new();
    assets.insert(
        "large_image".into(),
        json!(state.logo_url.as_deref().unwrap_or(IDLE_LARGE_IMAGE)),
    );
    assets.insert("large_text".into(), json!(state.instance_name));
    if let Some(line) = state.detail_line.as_deref() {
        assets.insert("small_text".into(), json!(line));
    }
    activity.insert("assets".into(), Value::Object(assets));

    Value::Object(activity)
}

fn build_idle_activity(line: &str, started_at: i64) -> Value {
    let mut activity = Map::new();
    activity.insert("type".into(), json!(0));
    activity.insert("name".into(), json!(IDLE_ACTIVITY_NAME));
    activity.insert("details".into(), json!(line));
    activity.insert("timestamps".into(), json!({ "start": started_at * 1000 }));

    let mut assets = Map::new();
    assets.insert("large_image".into(), json!(IDLE_LARGE_IMAGE));
    assets.insert("large_text".into(), json!(IDLE_ACTIVITY_NAME));
    activity.insert("assets".into(), Value::Object(assets));

    Value::Object(activity)
}

fn connect(app_id: &str) -> std::result::Result<Connection, String> {
    Connection::open(app_id).map_err(|error| {
        let message = error.to_string();
        tracing::debug!(%message, "could not open a discord connection");
        message
    })
}

fn current_activity(worker: &Worker) -> Option<Value> {
    match worker.games.last() {
        Some((_, activity)) => Some(build_activity(activity)),
        None => worker
            .idle
            .as_ref()
            .map(|(_, line, started_at)| build_idle_activity(line, *started_at)),
    }
}

fn target_app_id(worker: &Worker) -> Option<String> {
    worker
        .games
        .last()
        .map(|(_, activity)| activity.app_id.clone())
        .or_else(|| worker.idle.as_ref().map(|(app_id, _, _)| app_id.clone()))
}

fn publish_current(worker: &mut Worker, shared: &Mutex<SharedState>) {
    let Some(activity) = current_activity(worker) else {
        return;
    };
    let mut shared = shared.lock().unwrap();
    if shared.last.as_ref() == Some(&activity) {
        return;
    }
    let Some((_, client)) = worker.client.as_mut() else {
        return;
    };
    match client.set_activity(activity.clone()) {
        Ok(()) => shared.last = Some(activity),
        Err(error) => {
            tracing::debug!(%error, "the discord connection went away, dropping it");
            worker.client = None;
        }
    }
}

fn heartbeat(worker: &mut Worker, shared: &Mutex<SharedState>) {
    if worker.client.is_none() {
        if let Some(app_id) = target_app_id(worker) {
            if let Ok(fresh) = connect(&app_id) {
                worker.client = Some((app_id, fresh));
            }
        }
    }
    publish_current(worker, shared);
}

fn clear_everything(worker: &mut Worker, shared: &Mutex<SharedState>) {
    worker.games.clear();
    worker.idle = None;
    shared.lock().unwrap().last = None;
    if let Some((_, client)) = worker.client.as_mut() {
        if client.clear_activity().is_err() {
            worker.client = None;
        }
    }
}

fn scoped_clear(worker: &mut Worker, shared: &Mutex<SharedState>, running_id: &str) {
    worker.games.retain(|(id, _)| id != running_id);
    let target = current_activity(worker);
    if target.is_none() {
        shared.lock().unwrap().last = None;
    }
    let Some((_, client)) = worker.client.as_mut() else {
        return;
    };
    let outcome = match target.clone() {
        Some(activity) => client.set_activity(activity),
        None => client.clear_activity(),
    };
    match outcome {
        Ok(()) => shared.lock().unwrap().last = target,
        Err(error) => {
            tracing::debug!(%error, "the discord connection went away, dropping it");
            worker.client = None;
        }
    }
}

fn republish(worker: &mut Worker, shared: &Mutex<SharedState>) -> std::result::Result<(), String> {
    let target = current_activity(worker);
    let Some((_, client)) = worker.client.as_mut() else {
        return Err("no discord connection".to_string());
    };
    match target {
        Some(activity) => client
            .set_activity(activity.clone())
            .map(|_| shared.lock().unwrap().last = Some(activity))
            .map_err(|error| error.to_string()),
        None => client
            .clear_activity()
            .map(|_| shared.lock().unwrap().last = None)
            .map_err(|error| error.to_string()),
    }
}

fn worker(rx: mpsc::Receiver<Message>, shared: Arc<Mutex<SharedState>>) {
    let mut worker = Worker {
        client: None,
        games: Vec::new(),
        idle: None,
    };

    loop {
        let message = match rx.recv_timeout(HEARTBEAT) {
            Ok(message) => message,
            Err(RecvTimeoutError::Timeout) => {
                heartbeat(&mut worker, &shared);
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };

        match message {
            Message::Ping(reply) => {
                let _ = reply.send(());
            }
            Message::Set {
                running_id,
                activity,
            } => {
                match worker.games.iter_mut().find(|(id, _)| id == &running_id) {
                    Some(slot) => slot.1 = *activity,
                    None => worker.games.push((running_id, *activity)),
                }
                publish_current(&mut worker, &shared);
            }
            Message::Clear { running_id } => {
                scoped_clear(&mut worker, &shared, &running_id);
            }
            Message::ClearAll => {
                clear_everything(&mut worker, &shared);
            }
            Message::Idle {
                app_id,
                line,
                started_at,
            } => {
                let changed = worker
                    .idle
                    .as_ref()
                    .is_none_or(|(_, current, _)| current != &line);
                if changed {
                    worker.idle = Some((app_id, line, started_at));
                    publish_current(&mut worker, &shared);
                }
            }
            Message::Reconnect { app_id, reply } => {
                if let Some((_, mut stale)) = worker.client.take() {
                    stale.close();
                }
                let outcome = match connect(&app_id) {
                    Ok(fresh) => {
                        worker.client = Some((app_id.clone(), fresh));
                        if target_app_id(&worker).as_deref() == Some(app_id.as_str()) {
                            republish(&mut worker, &shared)
                        } else {
                            Ok(())
                        }
                    }
                    Err(message) => Err(message),
                };
                if outcome.is_err() {
                    worker.client = None;
                }
                let _ = reply.send(outcome);
            }
        }
    }

    if let Some((_, mut active)) = worker.client {
        let _ = active.clear_activity();
        active.close();
    }
}

fn supervisor(tx: Arc<Mutex<Sender<Message>>>, shared: Arc<Mutex<SharedState>>) {
    loop {
        std::thread::sleep(WATCHDOG_INTERVAL);
        let (reply, ack) = mpsc::channel();
        let sent = tx.lock().unwrap().send(Message::Ping(reply)).is_ok();
        if sent && ack.recv_timeout(WATCHDOG_REPLY).is_ok() {
            continue;
        }
        tracing::warn!("the discord presence worker is unresponsive, respawning it");
        let (fresh_tx, fresh_rx) = mpsc::channel();
        *tx.lock().unwrap() = fresh_tx;
        let worker_shared = shared.clone();
        std::thread::Builder::new()
            .name("discord-presence".to_string())
            .spawn(move || worker(fresh_rx, worker_shared))
            .expect("could not restart the discord presence thread");
    }
}

impl Presence {
    pub fn spawn() -> Self {
        let shared = Arc::new(Mutex::new(SharedState { last: None }));
        let (tx, rx) = mpsc::channel();
        let sender = Arc::new(Mutex::new(tx));
        let worker_shared = shared.clone();
        std::thread::Builder::new()
            .name("discord-presence".to_string())
            .spawn(move || worker(rx, worker_shared))
            .expect("could not start the discord presence thread");
        let supervisor_tx = sender.clone();
        std::thread::Builder::new()
            .name("discord-presence-supervisor".to_string())
            .spawn(move || supervisor(supervisor_tx, shared))
            .expect("could not start the discord presence watchdog");
        Self { tx: sender }
    }

    pub fn set(&self, running_id: String, activity: PresenceActivity) {
        let _ = self.tx.lock().unwrap().send(Message::Set {
            running_id,
            activity: Box::new(activity),
        });
    }

    pub fn clear_run(&self, running_id: &str) {
        let _ = self.tx.lock().unwrap().send(Message::Clear {
            running_id: running_id.to_string(),
        });
    }

    pub fn clear(&self) {
        let _ = self.tx.lock().unwrap().send(Message::ClearAll);
    }

    pub fn idle(&self, app_id: String, line: String, started_at: i64) {
        let _ = self.tx.lock().unwrap().send(Message::Idle {
            app_id,
            line,
            started_at,
        });
    }

    pub fn reconnect(&self, app_id: String) -> std::result::Result<(), String> {
        let (reply, answer) = mpsc::channel();
        if self
            .tx
            .lock()
            .unwrap()
            .send(Message::Reconnect { app_id, reply })
            .is_err()
        {
            return Err("The presence worker is not running.".to_string());
        }
        answer
            .recv_timeout(Duration::from_secs(15))
            .unwrap_or_else(|_| Err("Discord did not answer in time.".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> LauncherSettings {
        LauncherSettings {
            discord_app_id: "1234567890".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn the_master_toggle_suppresses_everything() {
        let mut settings = settings();
        settings.discord_rpc = false;
        assert!(activity_for(
            &settings,
            "ATM10",
            "1.21.1",
            Some("neoforge"),
            None,
            None,
            0
        )
        .is_none());
    }

    #[test]
    fn missing_app_id_suppresses_everything() {
        let mut settings = settings();
        settings.discord_app_id = String::new();
        assert_eq!(
            activity_for(&settings, "ATM10", "1.21.1", None, None, None, 0).is_none(),
            build_info::bundled_discord_app_id().is_none()
        );
    }

    #[test]
    fn version_and_loader_share_one_line() {
        let activity = activity_for(
            &settings(),
            "ATM10",
            "1.21.1",
            Some("neoforge"),
            None,
            None,
            0,
        )
        .unwrap();
        assert_eq!(activity.version_line.as_deref(), Some("1.21.1 · neoforge"));
    }

    #[test]
    fn each_field_toggle_drops_only_its_own_line() {
        let mut settings = settings();
        settings.discord_rpc_show_version = false;
        settings.discord_rpc_show_logo = false;
        let activity = activity_for(
            &settings,
            "ATM10",
            "1.21.1",
            Some("neoforge"),
            Some("https://example.invalid/logo.png"),
            Some("4 day streak".to_string()),
            42,
        )
        .unwrap();
        assert!(activity.version_line.is_none());
        assert!(activity.logo_url.is_none());
        assert_eq!(activity.detail_line.as_deref(), Some("4 day streak"));
        assert_eq!(activity.instance_name, "ATM10");
        assert_eq!(activity.started_at, 42);
    }

    #[test]
    fn only_remote_logos_are_forwarded() {
        let activity = activity_for(
            &settings(),
            "ATM10",
            "1.21.1",
            None,
            Some("/home/user/.local/share/basalt/logo.png"),
            None,
            0,
        )
        .unwrap();
        assert!(activity.logo_url.is_none());
    }

    fn activity(instance_name: &str) -> PresenceActivity {
        PresenceActivity {
            app_id: "1234567890".to_string(),
            instance_name: instance_name.to_string(),
            version_line: None,
            detail_line: None,
            logo_url: None,
            started_at: 1,
        }
    }

    fn worker_with_idle() -> Worker {
        Worker {
            client: None,
            games: Vec::new(),
            idle: Some(("1234567890".to_string(), "Browsing modpacks".to_string(), 2)),
        }
    }

    #[test]
    fn the_newest_game_wins_over_idle() {
        let mut worker = worker_with_idle();
        worker.games.push(("run-1".to_string(), activity("ATM10")));
        worker
            .games
            .push(("run-2".to_string(), activity("Fabulously Optimized")));
        let published = current_activity(&worker).unwrap();
        assert_eq!(published["name"], "Minecraft");
        assert_eq!(published["details"], "Fabulously Optimized");
    }

    #[test]
    fn idle_is_published_when_no_game_runs() {
        let worker = worker_with_idle();
        let published = current_activity(&worker).unwrap();
        assert_eq!(published["name"], "Basalt");
        assert_eq!(published["details"], "Browsing modpacks");
        assert_eq!(published["timestamps"]["start"], 2 * 1000);
    }

    #[test]
    fn the_oldest_game_resurfaces_after_the_newest_exits() {
        let mut worker = worker_with_idle();
        worker.games.push(("run-1".to_string(), activity("ATM10")));
        worker
            .games
            .push(("run-2".to_string(), activity("Fabulously Optimized")));
        worker.games.retain(|(id, _)| id != "run-2");
        let published = current_activity(&worker).unwrap();
        assert_eq!(published["details"], "ATM10");
    }
}
