pub mod process;

use std::collections::HashMap;

use tauri::AppHandle;

use crate::auth::account::{self, Account};
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

async fn ensure_account(state: &AppState) -> Result<Account> {
    let mut store = account::load(&state.paths)?;
    let account = store
        .active()
        .cloned()
        .ok_or_else(|| Error::other("No account signed in. Add a Microsoft account first."))?;

    if account.expires_at > now() + 60 {
        return Ok(account);
    }

    let refreshed = microsoft::refresh(&state.http, &account.refresh_token).await?;
    let mc = microsoft::authenticate_minecraft(&state.http, &refreshed.access_token).await?;
    let updated = Account {
        id: account.id,
        name: mc.name,
        mc_access_token: mc.access_token,
        refresh_token: refreshed.refresh_token,
        expires_at: now() + mc.expires_in,
    };
    store.upsert_active(updated.clone());
    account::save(&state.paths, &store)?;
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

fn classpath_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

pub async fn launch_instance(
    app: &AppHandle,
    state: &AppState,
    instance: &Instance,
) -> Result<String> {
    let version: VersionJson = install::load_version_json(state, &instance.version_id).await?;
    let account = ensure_account(state).await?;

    let (java_path, settings_java) = {
        let settings = state.settings.lock().unwrap();
        (instance.java_path.clone(), settings.java_path.clone())
    };
    let explicit = java_path.or(settings_java);
    let required = version.required_java_major();
    let java = java::find_for_major(required, explicit.as_deref())
        .await
        .ok_or_else(|| {
            Error::other(format!(
                "No Java found. Minecraft {} needs Java {required}.",
                version.id
            ))
        })?;
    if java.major < required {
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
    classpath.push(state.paths.version_jar(&version.id).display().to_string());
    let classpath = classpath.join(classpath_separator());

    let natives_dir = state.paths.natives_dir(&version.id);
    let game_dir = state.paths.instance_dir(&instance.id);
    std::fs::create_dir_all(&game_dir)?;

    let (min_mb, max_mb) = {
        let settings = state.settings.lock().unwrap();
        (
            instance.min_memory_mb.unwrap_or(settings.min_memory_mb),
            instance.max_memory_mb.unwrap_or(settings.max_memory_mb),
        )
    };

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
    subs.insert("assets_index_name", version.assets.clone());
    subs.insert("auth_uuid", account.id.clone());
    subs.insert("auth_access_token", account.mc_access_token.clone());
    subs.insert("clientid", microsoft::CLIENT_ID.to_string());
    subs.insert("auth_xuid", String::new());
    subs.insert("user_type", "msa".to_string());
    subs.insert("version_type", version.kind.clone());

    let mut args: Vec<String> = vec![format!("-Xms{min_mb}M"), format!("-Xmx{max_mb}M")];

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

    let running_id = uuid::Uuid::new_v4().to_string();
    process::spawn_process(
        app,
        &state.running,
        &instance.id,
        &running_id,
        now(),
        &java.path,
        args,
        &game_dir,
    )?;

    Ok(running_id)
}
