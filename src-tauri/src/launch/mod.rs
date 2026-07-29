pub mod process;

use std::collections::HashMap;

use tauri::AppHandle;

use crate::auth::account::Account;
use crate::auth::microsoft;
use crate::config::Instance;
use crate::error::{Error, Result};
use crate::install;
use crate::java;
use crate::meta::version::{rules_allow, Arg, ArgValue, VersionJson};
use crate::state::AppState;

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[tracing::instrument(skip_all, err)]
pub(crate) async fn ensure_account(state: &AppState) -> Result<Account> {
    let mut store = state.db.load_accounts()?;
    let account = store
        .active()
        .cloned()
        .ok_or_else(|| Error::other("No account signed in. Add a Microsoft account first."))?;

    if account.expires_at > now() + 60 {
        tracing::debug!(account = %account.name, "using cached session token");
        return Ok(account);
    }

    tracing::info!(account = %account.name, "session expired, refreshing with microsoft");

    let refreshed = microsoft::refresh(&state.network, &account.refresh_token).await?;
    let mc = microsoft::authenticate_minecraft(&state.network, &refreshed.access_token).await?;
    let updated = Account {
        id: account.id,
        name: mc.name,
        mc_access_token: mc.access_token,
        refresh_token: refreshed.refresh_token,
        expires_at: now() + mc.expires_in,
    };
    store.upsert_active(updated.clone());
    state.db.save_accounts(&store)?;
    tracing::info!(account = %updated.name, "session refreshed");
    Ok(updated)
}

fn substitute(token: &str, subs: &HashMap<&str, String>) -> String {
    let mut result = token.to_string();
    for (key, value) in subs {
        result = result.replace(&format!("${{{key}}}"), value);
    }
    result
}

fn collect_arg(arg: &Arg, subs: &HashMap<&str, String>, out: &mut Vec<String>) {
    match arg {
        Arg::Plain(value) => out.push(substitute(value, subs)),
        Arg::Conditional { rules, value } => {
            if !rules_allow(rules) {
                return;
            }
            match value {
                ArgValue::Single(v) => out.push(substitute(v, subs)),
                ArgValue::Many(values) => {
                    for v in values {
                        out.push(substitute(v, subs));
                    }
                }
            }
        }
    }
}

pub const PLACEHOLDERS: [&str; 8] = [
    "min_ram",
    "max_ram",
    "natives_directory",
    "game_dir",
    "instance_name",
    "version",
    "launcher_name",
    "launcher_version",
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct LaunchPreview {
    pub java: String,
    pub pinned: bool,
    pub jvm: Vec<String>,
    pub game: Vec<String>,
}

pub fn sample_placeholders(paths: &crate::paths::Paths, settings: &crate::config::LauncherSettings) -> HashMap<&'static str, String> {
    HashMap::from([
        ("min_ram", settings.min_memory_mb.to_string()),
        ("max_ram", settings.max_memory_mb.to_string()),
        ("natives_directory", paths.natives_dir("1.21.4").display().to_string()),
        ("game_dir", paths.instances().join("example").display().to_string()),
        ("instance_name", "Example instance".to_string()),
        ("version", "1.21.4".to_string()),
        ("launcher_name", "basalt".to_string()),
        ("launcher_version", env!("CARGO_PKG_VERSION").to_string()),
    ])
}

pub fn preview(paths: &crate::paths::Paths, settings: &crate::config::LauncherSettings) -> LaunchPreview {
    let values = sample_placeholders(paths, settings);
    let template = if settings.jvm_args.trim().is_empty() {
        crate::config::DEFAULT_JVM_ARGS
    } else {
        settings.jvm_args.as_str()
    };

    let mut game = Vec::new();
    if settings.fullscreen {
        game.push("--fullscreen".to_string());
    } else if settings.window_width > 0 && settings.window_height > 0 {
        game.push("--width".to_string());
        game.push(settings.window_width.to_string());
        game.push("--height".to_string());
        game.push(settings.window_height.to_string());
    }
    game.extend(split_args(&render_placeholders(&settings.game_args, &values)));

    let pinned = settings
        .java_path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty());

    LaunchPreview {
        java: pinned.unwrap_or("java").to_string(),
        pinned: pinned.is_some(),
        jvm: split_args(&render_placeholders(template, &values)),
        game,
    }
}

pub fn render_placeholders(raw: &str, values: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;

    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        match after.find("}}") {
            Some(close) => {
                let name = after[..close].trim();
                match values.get(name) {
                    Some(value) => out.push_str(value),
                    None => {
                        tracing::warn!(placeholder = name, "unknown placeholder left as written");
                        out.push_str(&rest[open..open + 2 + close + 2]);
                    }
                }
                rest = &after[close + 2..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn split_args(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;

    for ch in raw.chars() {
        match quote {
            Some(active) if ch == active => quote = None,
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                started = true;
            }
            None if ch.is_whitespace() => {
                if started || !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            None => current.push(ch),
        }
    }
    if started || !current.is_empty() {
        out.push(current);
    }
    out
}

fn classpath_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

#[tracing::instrument(
    skip_all,
    fields(instance_id = %instance.id, name = %instance.name, version_id = %instance.version_id),
    err
)]
pub async fn launch_instance(
    app: &AppHandle,
    state: &AppState,
    instance: &Instance,
) -> Result<String> {
    let launch_version_id = instance
        .launch_version_id
        .clone()
        .unwrap_or_else(|| instance.version_id.clone());
    let version: VersionJson = install::load_merged_version(state, &launch_version_id).await?;
    let account = ensure_account(state).await?;

    let settings = state.db.load_settings()?;
    let explicit = instance.java_path.clone().or(settings.java_path.clone());
    let required = version.required_java_major();
    let java = java::find_for_major(required, explicit.as_deref())
        .await
        .ok_or_else(|| {
            Error::other(format!(
                "No Java found. Minecraft {} needs Java {required}.",
                version.id
            ))
        })?;
    tracing::info!(
        launch_version_id = %launch_version_id,
        required_java = required,
        java_major = java.major,
        java_path = %java.path,
        "resolved runtime for launch"
    );
    if java.major < required && settings.ignore_java_checks {
        tracing::warn!(
            required,
            found = java.major,
            "java version check skipped because ignore_java_checks is on"
        );
    } else if java.major < required {
        return Err(Error::other(format!(
            "Minecraft {} needs Java {required}, but the newest Java found is {} ({}).",
            version.id, java.major, java.path
        )));
    }

    let resolved = version.resolve_libraries(&state.paths);
    let mut classpath: Vec<String> = resolved
        .classpath
        .iter()
        .map(|spec| spec.dest.display().to_string())
        .collect();
    classpath.extend(
        resolved
            .natives
            .iter()
            .map(|native| native.spec.dest.display().to_string()),
    );
    classpath.push(state.paths.version_jar(version.jar_id()).display().to_string());
    let classpath = classpath.join(classpath_separator());

    let natives_dir = state.paths.natives_dir(&version.id);
    state.files.ensure_dir(&natives_dir)?;
    let game_dir = state.paths.instance_dir(&instance.id);
    state.files.ensure_dir(&game_dir)?;

    let min_mb = instance.min_memory_mb.unwrap_or(settings.min_memory_mb);
    let max_mb = instance.max_memory_mb.unwrap_or(settings.max_memory_mb);

    let mut subs: HashMap<&str, String> = HashMap::new();
    subs.insert("natives_directory", natives_dir.display().to_string());
    subs.insert("launcher_name", "basalt".to_string());
    subs.insert("launcher_version", env!("CARGO_PKG_VERSION").to_string());
    subs.insert("classpath", classpath.clone());
    subs.insert("classpath_separator", classpath_separator().to_string());
    subs.insert("library_directory", state.paths.libraries().display().to_string());
    subs.insert("auth_player_name", account.name.clone());
    subs.insert("version_name", version.id.clone());
    subs.insert("game_directory", game_dir.display().to_string());
    subs.insert("assets_root", state.paths.assets().display().to_string());
    subs.insert("assets_index_name", version.assets_name());
    subs.insert("auth_uuid", account.id.clone());
    subs.insert("auth_access_token", account.mc_access_token.clone());
    subs.insert("clientid", microsoft::CLIENT_ID.to_string());
    subs.insert("auth_xuid", String::new());
    subs.insert("user_type", "msa".to_string());
    subs.insert("version_type", version.kind.clone());

    let mut placeholders: HashMap<&str, String> = HashMap::new();
    placeholders.insert("min_ram", min_mb.to_string());
    placeholders.insert("max_ram", max_mb.to_string());
    placeholders.insert("natives_directory", natives_dir.display().to_string());
    placeholders.insert("game_dir", game_dir.display().to_string());
    placeholders.insert("instance_name", instance.name.clone());
    placeholders.insert("version", version.id.clone());
    placeholders.insert("launcher_name", "basalt".to_string());
    placeholders.insert("launcher_version", env!("CARGO_PKG_VERSION").to_string());

    let template = if settings.jvm_args.trim().is_empty() {
        crate::config::DEFAULT_JVM_ARGS
    } else {
        settings.jvm_args.as_str()
    };
    let mut args: Vec<String> = split_args(&render_placeholders(template, &placeholders));

    if let Some(arguments) = &version.arguments {
        for arg in &arguments.jvm {
            collect_arg(arg, &subs, &mut args);
        }
    } else {
        args.push(format!("-Djava.library.path={}", natives_dir.display()));
        args.push("-cp".to_string());
        args.push(classpath.clone());
    }

    args.push(version.main_class.clone());

    if let Some(arguments) = &version.arguments {
        for arg in &arguments.game {
            collect_arg(arg, &subs, &mut args);
        }
    } else if let Some(legacy) = &version.minecraft_arguments {
        for token in legacy.split(' ').filter(|t| !t.is_empty()) {
            args.push(substitute(token, &subs));
        }
    }

    if settings.fullscreen {
        args.push("--fullscreen".to_string());
    } else if settings.window_width > 0 && settings.window_height > 0 {
        args.push("--width".to_string());
        args.push(settings.window_width.to_string());
        args.push("--height".to_string());
        args.push(settings.window_height.to_string());
    }

    let extra_game = split_args(&render_placeholders(&settings.game_args, &placeholders));
    args.extend(extra_game.iter().cloned());

    let env: Vec<(String, String)> = settings
        .env_vars
        .iter()
        .filter(|v| !v.key.trim().is_empty())
        .map(|v| (v.key.trim().to_string(), v.value.clone()))
        .collect();

    tracing::info!(
        main_class = %version.main_class,
        classpath_entries = resolved.classpath.len() + resolved.natives.len() + 1,
        args = args.len(),
        min_memory_mb = min_mb,
        max_memory_mb = max_mb,
        game_dir = %game_dir.display(),
        jvm_args = args.len(),
        game_args = extra_game.len(),
        env_vars = env.len(),
        fullscreen = settings.fullscreen,
        "launching minecraft"
    );

    let running_id = uuid::Uuid::new_v4().to_string();
    process::spawn_process(
        app,
        &state.running,
        state.db.clone(),
        &instance.id,
        &running_id,
        now(),
        &java.path,
        args,
        &game_dir,
        env,
    )?;

    Ok(running_id)
}

#[cfg(test)]
mod tests {
    use super::{render_placeholders, split_args};
    use std::collections::HashMap;

    fn values() -> HashMap<&'static str, String> {
        HashMap::from([("min_ram", "512".to_string()), ("max_ram", "4096".to_string())])
    }

    #[test]
    fn fills_the_default_template() {
        let rendered = render_placeholders(crate::config::DEFAULT_JVM_ARGS, &values());
        assert_eq!(rendered, "-Xms512M -Xmx4096M");
        assert_eq!(split_args(&rendered), vec!["-Xms512M", "-Xmx4096M"]);
    }

    #[test]
    fn tolerates_spacing_and_unknown_placeholders() {
        assert_eq!(render_placeholders("-Xmx{{ max_ram }}M", &values()), "-Xmx4096M");
        assert_eq!(
            render_placeholders("-Xmx{{max_ram}}M {{nope}}", &values()),
            "-Xmx4096M {{nope}}"
        );
        assert_eq!(render_placeholders("-Xmx{{max_ram", &values()), "-Xmx{{max_ram");
        assert_eq!(render_placeholders("no placeholders", &values()), "no placeholders");
    }

    #[test]
    fn splits_on_whitespace() {
        assert_eq!(
            split_args("-XX:+UseG1GC -XX:G1NewSizePercent=20"),
            vec!["-XX:+UseG1GC", "-XX:G1NewSizePercent=20"]
        );
        assert!(split_args("   ").is_empty());
        assert!(split_args("").is_empty());
    }

    #[test]
    fn keeps_quoted_values_together() {
        assert_eq!(
            split_args("-Dname=\"Basalt Launcher\" -Xmx4G"),
            vec!["-Dname=Basalt Launcher", "-Xmx4G"]
        );
        assert_eq!(split_args("'single quoted arg'"), vec!["single quoted arg"]);
        assert_eq!(split_args("--flag \"\""), vec!["--flag", ""]);
    }

    #[test]
    fn collapses_newlines_and_repeated_spaces() {
        assert_eq!(
            split_args("-XX:+UnlockExperimentalVMOptions\n  -XX:+UseG1GC\t-Xss2M"),
            vec!["-XX:+UnlockExperimentalVMOptions", "-XX:+UseG1GC", "-Xss2M"]
        );
    }
}
