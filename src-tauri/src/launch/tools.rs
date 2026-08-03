use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use crate::{
    config::{Instance, LauncherSettings},
    error::{Error, Result},
};

const HOOK_TIMEOUT: Duration = Duration::from_secs(120);
const REPORTED_LINES: usize = 12;

#[derive(Debug, Clone, Default)]
pub struct LaunchTools {
    pub wrapper: String,
    pub pre_launch: String,
    pub post_exit: String,
}

fn pick(instance: Option<&String>, global: &str) -> String {
    let own = instance.map(String::as_str).unwrap_or("").trim();
    if own.is_empty() {
        global.trim().to_string()
    } else {
        own.to_string()
    }
}

pub fn resolve(
    settings: &LauncherSettings,
    instance: &Instance,
    placeholders: &HashMap<&str, String>,
) -> LaunchTools {
    let render = |value: String| super::render_placeholders(&value, placeholders);

    LaunchTools {
        wrapper: render(pick(
            instance.wrapper_command.as_ref(),
            &settings.wrapper_command,
        )),
        pre_launch: render(pick(
            instance.pre_launch_command.as_ref(),
            &settings.pre_launch_command,
        )),
        post_exit: render(pick(
            instance.post_exit_command.as_ref(),
            &settings.post_exit_command,
        )),
    }
}

fn tail(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(REPORTED_LINES);
    lines[start..].join("\n")
}

fn shell(command: &str) -> tokio::process::Command {
    let mut spawned = if cfg!(target_os = "windows") {
        let mut spawned = tokio::process::Command::new("cmd");
        spawned.arg("/C");
        spawned
    } else {
        let mut spawned = tokio::process::Command::new("sh");
        spawned.arg("-c");
        spawned
    };
    spawned.arg(command);
    spawned
}

pub async fn run_hook(
    label: &str,
    command: &str,
    cwd: &Path,
    env: &[(String, String)],
) -> Result<()> {
    if command.trim().is_empty() {
        return Ok(());
    }

    tracing::info!(label, command, "running launch hook");
    let mut spawned = shell(command);
    spawned.current_dir(cwd).envs(env.iter().cloned());

    let finished = tokio::time::timeout(HOOK_TIMEOUT, spawned.output()).await;

    let output = match finished {
        Err(_) => {
            return Err(Error::other(format!(
                "The {label} command is still running after {} seconds.",
                HOOK_TIMEOUT.as_secs()
            )))
        }
        Ok(Err(error)) => {
            return Err(Error::other(format!(
                "The {label} command could not start: {error}"
            )))
        }
        Ok(Ok(output)) => output,
    };

    if output.status.success() {
        return Ok(());
    }

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let reported = tail(&text);
    tracing::error!(label, command, "launch hook failed:\n{reported}");

    Err(Error::other(if reported.trim().is_empty() {
        format!("The {label} command failed.")
    } else {
        format!("The {label} command failed:\n{reported}")
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> LauncherSettings {
        LauncherSettings {
            wrapper_command: "mangohud".to_string(),
            pre_launch_command: "echo global".to_string(),
            post_exit_command: String::new(),
            ..LauncherSettings::default()
        }
    }

    fn instance() -> Instance {
        Instance {
            id: "test".to_string(),
            name: "Test".to_string(),
            version_id: "1.21.1".to_string(),
            created_at: chrono::Utc::now(),
            min_memory_mb: None,
            max_memory_mb: None,
            java_path: None,
            last_played_at: None,
            playtime_secs: 0,
            dir: String::new(),
            logo: None,
            loader: None,
            loader_version: None,
            launch_version_id: None,
            pack_provider: None,
            pack_project_id: None,
            pack_version_id: None,
            jvm_args: None,
            jvm_args_mode: None,
            env_vars: None,
            env_vars_mode: None,
            import_source: None,
            import_source_id: None,
            banner_id: None,
            notes: None,
            wrapper_command: None,
            pre_launch_command: None,
            post_exit_command: None,
        }
    }

    fn placeholders() -> HashMap<&'static str, String> {
        let mut values = HashMap::new();
        values.insert("instance_name", "Test".to_string());
        values
    }

    #[test]
    fn an_instance_that_says_nothing_takes_the_launcher_setting() {
        let tools = resolve(&settings(), &instance(), &placeholders());
        assert_eq!(tools.wrapper, "mangohud");
        assert_eq!(tools.pre_launch, "echo global");
    }

    #[test]
    fn an_instance_value_stands_in_for_the_launcher_one() {
        let mut own = instance();
        own.wrapper_command = Some("strace -f".to_string());

        let tools = resolve(&settings(), &own, &placeholders());
        assert_eq!(tools.wrapper, "strace -f");
        assert_eq!(tools.pre_launch, "echo global");
    }

    #[test]
    fn placeholders_reach_inside_a_hook() {
        let mut own = instance();
        own.pre_launch_command = Some("echo {{instance_name}}".to_string());
        let tools = resolve(&settings(), &own, &placeholders());
        assert_eq!(tools.pre_launch, "echo Test");
    }

    #[tokio::test]
    async fn a_command_that_fails_reports_what_it_printed() {
        if cfg!(target_os = "windows") {
            return;
        }
        let error = run_hook(
            "pre-launch",
            "echo something went wrong; exit 3",
            Path::new("."),
            &[],
        )
        .await
        .unwrap_err();

        let message = error.to_string();
        assert!(message.contains("something went wrong"), "{message}");
    }

    #[tokio::test]
    async fn an_empty_command_is_not_a_failure() {
        assert!(run_hook("pre-launch", "   ", Path::new("."), &[])
            .await
            .is_ok());
    }
}
