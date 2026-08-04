use std::sync::mpsc::{self, Sender};

use discord_rich_presence::{
    activity::{Activity, Assets, Timestamps},
    DiscordIpc, DiscordIpcClient,
};

use crate::{build_info, config::LauncherSettings};

const IDLE_LARGE_IMAGE: &str = "basalt";

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
    Set(Box<PresenceActivity>),
    Clear,
    Reconnect { app_id: String, reply: Sender<bool> },
}

pub struct Presence {
    tx: Sender<Message>,
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

fn build_activity(state: &PresenceActivity) -> Activity<'_> {
    let mut activity = Activity::new()
        .details(state.instance_name.as_str())
        .timestamps(Timestamps::new().start(state.started_at));

    if let Some(line) = state.version_line.as_deref() {
        activity = activity.state(line);
    }

    let large_image = state.logo_url.as_deref().unwrap_or(IDLE_LARGE_IMAGE);
    let mut assets = Assets::new()
        .large_image(large_image)
        .large_text(state.instance_name.as_str());
    if let Some(line) = state.detail_line.as_deref() {
        assets = assets.small_text(line);
    }
    activity.assets(assets)
}

fn connect(app_id: &str) -> Option<DiscordIpcClient> {
    let mut fresh = DiscordIpcClient::new(app_id);
    match fresh.connect() {
        Ok(()) => Some(fresh),
        Err(error) => {
            tracing::debug!(%error, "discord is not accepting connections");
            None
        }
    }
}

fn worker(rx: mpsc::Receiver<Message>) {
    let mut client: Option<(String, DiscordIpcClient)> = None;
    let mut last: Option<PresenceActivity> = None;

    while let Ok(message) = rx.recv() {
        match message {
            Message::Clear => {
                last = None;
                if let Some((_, active)) = client.as_mut() {
                    if active.clear_activity().is_err() {
                        client = None;
                    }
                }
            }
            Message::Reconnect { app_id, reply } => {
                if let Some((_, mut stale)) = client.take() {
                    let _ = stale.close();
                }
                client = connect(&app_id).map(|fresh| (app_id.clone(), fresh));
                if let (Some((_, active)), Some(state)) = (client.as_mut(), last.as_ref()) {
                    if state.app_id == app_id {
                        let _ = active.set_activity(build_activity(state));
                    }
                }
                let _ = reply.send(client.is_some());
            }
            Message::Set(state) => {
                if client.as_ref().is_some_and(|(id, _)| id != &state.app_id) {
                    if let Some((_, mut stale)) = client.take() {
                        let _ = stale.close();
                    }
                }

                if client.is_none() {
                    client = connect(&state.app_id).map(|fresh| (state.app_id.clone(), fresh));
                }
                last = Some((*state).clone());

                if let Some((_, active)) = client.as_mut() {
                    if let Err(error) = active.set_activity(build_activity(&state)) {
                        tracing::debug!(%error, "could not publish the discord activity");
                        client = None;
                    }
                }
            }
        }
    }

    if let Some((_, mut active)) = client {
        let _ = active.clear_activity();
        let _ = active.close();
    }
}

impl Presence {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("discord-presence".to_string())
            .spawn(move || worker(rx))
            .expect("could not start the discord presence thread");
        Self { tx }
    }

    pub fn set(&self, activity: PresenceActivity) {
        let _ = self.tx.send(Message::Set(Box::new(activity)));
    }

    pub fn clear(&self) {
        let _ = self.tx.send(Message::Clear);
    }

    pub fn reconnect(&self, app_id: String) -> bool {
        let (reply, answer) = mpsc::channel();
        if self.tx.send(Message::Reconnect { app_id, reply }).is_err() {
            return false;
        }
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> LauncherSettings {
        let mut settings = LauncherSettings::default();
        settings.discord_app_id = "1234567890".to_string();
        settings
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
}
