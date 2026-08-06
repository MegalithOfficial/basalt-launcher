use std::{ffi::OsString, io::IsTerminal, sync::OnceLock, time::Duration};

use clap::Parser;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::{
    config::Instance,
    error::{Error, Result},
    launch,
    state::AppState,
    tasks::TaskKind,
};

const MIN_ID_PREFIX_LEN: usize = 8;
const VALUE_FLAGS: &[&str] = &["-l", "--launch"];
const BARE_FLAGS: &[&str] = &["-h", "--help", "-V", "--version", "-L", "--list"];
const GAME_POLL_INTERVAL: Duration = Duration::from_millis(500);

fn version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(crate::build_info::display_version)
}

#[derive(Parser, Debug)]
#[command(
    name = "basalt-launcher",
    version = version(),
    about = "A polished Minecraft launcher that puts form and function on equal footing."
)]
struct Cli {
    #[arg(
        short = 'l',
        long = "launch",
        value_name = "INSTANCE",
        help = "Launch an instance by name or by the start of its ID",
        action = clap::ArgAction::Append
    )]
    launch: Vec<String>,

    #[arg(
        short = 'L',
        long = "list",
        help = "Print every instance with the selector that launches it, then exit"
    )]
    list: bool,
}

pub enum Request {
    Nothing,
    Launch(String),
    List,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Startup,
    Activation,
}

impl Origin {
    fn owns_process(self) -> bool {
        self == Origin::Startup
    }
}

#[derive(Clone, Copy)]
struct Candidate<'a> {
    id: &'a str,
    name: &'a str,
}

pub fn startup_request() -> Result<Request> {
    parse(std::env::args_os(), Origin::Startup)
}

pub fn handle_activation(app: &AppHandle, argv: Vec<String>) {
    match parse(argv, Origin::Activation) {
        Ok(Request::Launch(selector)) => start_launch(app.clone(), selector, Origin::Activation),
        Ok(Request::List | Request::Nothing) => {}
        Err(error) => show_error(app, error, Origin::Activation),
    }
}

pub fn start_launch(app: AppHandle, selector: String, origin: Origin) {
    tauri::async_runtime::spawn(async move {
        let Some(state) = app.try_state::<AppState>() else {
            let waiting =
                Error::other("Basalt is still starting. Try launching the instance again.");
            show_error(&app, waiting, origin);
            return;
        };

        let instance = match ready_instance(&state, &selector) {
            Ok(instance) => instance,
            Err(error) => return show_error(&app, error, origin),
        };

        let running_id = match launch::launch_instance(&app, &state, &instance).await {
            Ok(running_id) => running_id,
            Err(error) => return show_error(&app, error, origin),
        };

        tracing::info!(
            instance_id = %instance.id,
            selector,
            "instance launched from the command line"
        );

        if origin.owns_process() {
            wait_for_game(&app, &state, &running_id).await;
            app.exit(0);
        }
    });
}

pub fn print_instances(state: &AppState) -> Result<()> {
    let instances = state.db.list_instances(&state.files)?;
    let candidates = candidates(&instances);
    for instance in &instances {
        println!(
            "{}\t{}",
            unique_prefix(&candidates, &instance.id),
            instance.name
        );
    }
    Ok(())
}

pub fn launch_command(state: &AppState, instance_id: &str) -> Result<String> {
    let instances = state.db.list_instances(&state.files)?;
    let target = instances
        .iter()
        .find(|instance| instance.id == instance_id)
        .ok_or_else(|| Error::other("Instance not found."))?;
    Ok(format!(
        "{} -l {}",
        shell_quoted(&program_path()),
        unique_prefix(&candidates(&instances), &target.id)
    ))
}

fn program_path() -> String {
    std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "basalt-launcher".to_string())
}

fn shell_quoted(path: &str) -> String {
    if path.contains(char::is_whitespace) {
        format!("\"{path}\"")
    } else {
        path.to_string()
    }
}

fn parse<I, S>(args: I, origin: Origin) -> Result<Request>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
{
    let (known, ignored) = split_known(args);
    warn_about(&ignored);

    let parsed = match Cli::try_parse_from(known) {
        Ok(parsed) => parsed,
        Err(error) => return from_clap_error(error, origin),
    };

    if parsed.list {
        return Ok(Request::List);
    }

    match parsed.launch.as_slice() {
        [] => Ok(Request::Nothing),
        [selector] if selector.trim().is_empty() => {
            Err(Error::other("The launch selector cannot be empty."))
        }
        [selector] => Ok(Request::Launch(selector.clone())),
        _ => Err(Error::other("Only one instance can be launched at a time.")),
    }
}

fn split_known<I, S>(args: I) -> (Vec<OsString>, Vec<String>)
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
{
    let mut args = args.into_iter().map(Into::into);
    let mut known: Vec<OsString> = args.next().into_iter().collect();
    let mut ignored = Vec::new();

    while let Some(argument) = args.next() {
        let text = argument.to_string_lossy().into_owned();
        if VALUE_FLAGS.contains(&text.as_str()) {
            known.push(argument);
            known.extend(args.next());
        } else if BARE_FLAGS.contains(&text.as_str()) || text.starts_with("--launch=") {
            known.push(argument);
        } else {
            ignored.push(text);
        }
    }

    (known, ignored)
}

fn warn_about(ignored: &[String]) {
    if ignored.is_empty() {
        return;
    }
    let listed = ignored.join(" ");
    tracing::warn!(ignored = %listed, "ignoring unrecognised command line arguments");
    eprintln!("Ignoring unrecognised arguments: {listed}");
    eprintln!("Run with --help to see what Basalt accepts.");
}

fn from_clap_error(error: clap::Error, origin: Origin) -> Result<Request> {
    let asked_for_text = matches!(
        error.kind(),
        clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
    );

    if asked_for_text {
        if !origin.owns_process() {
            return Ok(Request::Nothing);
        }
        let _ = error.print();
        std::process::exit(0);
    }

    Err(Error::other(first_line(&error.render().to_string())))
}

fn first_line(message: &str) -> String {
    message
        .lines()
        .next()
        .unwrap_or("Could not read the command line.")
        .trim_start_matches("error: ")
        .to_string()
}

fn ready_instance(state: &AppState, selector: &str) -> Result<Instance> {
    let instance = resolve_instance(state, selector)?;
    if state.tasks.has_active(&instance.id, TaskKind::WorldImport) {
        return Err(Error::other(
            "Wait for the world import to finish before launching this instance.",
        ));
    }
    Ok(instance)
}

fn resolve_instance(state: &AppState, selector: &str) -> Result<Instance> {
    let instances = state.db.list_instances(&state.files)?;
    let id = resolve_selector(&candidates(&instances), selector)?;
    instances
        .into_iter()
        .find(|instance| instance.id == id)
        .ok_or_else(|| Error::other("Instance not found."))
}

fn candidates(instances: &[Instance]) -> Vec<Candidate<'_>> {
    instances
        .iter()
        .map(|instance| Candidate {
            id: &instance.id,
            name: &instance.name,
        })
        .collect()
}

fn compact_id(id: &str) -> String {
    id.chars()
        .filter(|character| *character != '-')
        .flat_map(char::to_lowercase)
        .collect()
}

fn resolve_selector(candidates: &[Candidate<'_>], selector: &str) -> Result<String> {
    let selector = selector.trim();
    let compact_selector = compact_id(selector);

    let exact = candidates.iter().find(|candidate| {
        candidate.id.eq_ignore_ascii_case(selector) || compact_id(candidate.id) == compact_selector
    });
    if let Some(candidate) = exact {
        return Ok(candidate.id.to_string());
    }

    let named = candidates
        .iter()
        .filter(|candidate| candidate.name == selector)
        .collect::<Vec<_>>();
    match named.as_slice() {
        [candidate] => return Ok(candidate.id.to_string()),
        [_, _, ..] => {
            return Err(Error::other(format!(
                "More than one instance is named \"{selector}\". Use its ID prefix instead."
            )))
        }
        [] => {}
    }

    if compact_selector.len() < MIN_ID_PREFIX_LEN {
        return Err(Error::other(format!(
            "No instance is named \"{selector}\". ID prefixes must be at least {MIN_ID_PREFIX_LEN} characters."
        )));
    }

    let matched = candidates
        .iter()
        .filter(|candidate| compact_id(candidate.id).starts_with(&compact_selector))
        .collect::<Vec<_>>();
    match matched.as_slice() {
        [candidate] => Ok(candidate.id.to_string()),
        [_, _, ..] => Err(Error::other(format!(
            "The ID prefix \"{selector}\" matches more than one instance. Use a longer prefix."
        ))),
        [] => Err(Error::other(format!("No instance matches \"{selector}\"."))),
    }
}

fn unique_prefix(candidates: &[Candidate<'_>], instance_id: &str) -> String {
    let compact = compact_id(instance_id);
    let shortest = MIN_ID_PREFIX_LEN.min(compact.len());
    (shortest..=compact.len())
        .map(|length| &compact[..length])
        .find(|prefix| {
            candidates
                .iter()
                .filter(|candidate| compact_id(candidate.id).starts_with(prefix))
                .count()
                == 1
        })
        .map(str::to_string)
        .unwrap_or(compact)
}

async fn wait_for_game(app: &AppHandle, state: &AppState, running_id: &str) {
    loop {
        tokio::time::sleep(GAME_POLL_INTERVAL).await;

        let finished = state
            .running
            .lock()
            .unwrap()
            .get(running_id)
            .is_none_or(|handle| handle.status.lock().unwrap().state != "running");
        if finished {
            tracing::info!(running_id, "command-line game session ended");
            return;
        }

        if app.get_webview_window("main").is_none() {
            return;
        }
    }
}

pub(crate) fn show_error(app: &AppHandle, error: Error, origin: Origin) {
    tracing::error!(error = %error, "command-line launch failed");
    eprintln!("{error}");

    if origin.owns_process() && std::io::stderr().is_terminal() {
        std::process::exit(1);
    }

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }

    app.dialog()
        .message(error.to_string())
        .title("Could not launch instance")
        .kind(MessageDialogKind::Error)
        .show(|_| {});
}

#[cfg(test)]
mod tests {
    use super::{parse, resolve_selector, unique_prefix, Candidate, Origin, Request};

    const INSTANCES: &[Candidate<'static>] = &[
        Candidate {
            id: "4f9c2a81-1111-2222-3333-444444444444",
            name: "Vanilla",
        },
        Candidate {
            id: "abcd1234-1111-2222-3333-444444444444",
            name: "Shared",
        },
        Candidate {
            id: "abcd1234-aaaa-bbbb-cccc-dddddddddddd",
            name: "Shared",
        },
    ];

    fn selector<const N: usize>(args: [&str; N]) -> super::Result<Option<String>> {
        match parse(args, Origin::Activation)? {
            Request::Launch(selector) => Ok(Some(selector)),
            _ => Ok(None),
        }
    }

    #[test]
    fn parses_short_long_and_equals_launch_flags() {
        assert_eq!(
            selector(["basalt", "-l", "Vanilla"]).unwrap(),
            Some("Vanilla".to_string())
        );
        assert_eq!(
            selector(["basalt", "--launch", "4f9c2a81"]).unwrap(),
            Some("4f9c2a81".to_string())
        );
        assert_eq!(
            selector(["basalt", "--launch=Vanilla"]).unwrap(),
            Some("Vanilla".to_string())
        );
    }

    #[test]
    fn rejects_missing_and_repeated_selectors() {
        assert!(selector(["basalt", "-l"]).is_err());
        assert!(selector(["basalt", "-l", "one", "--launch=two"]).is_err());
        assert!(selector(["basalt", "-l", "   "]).is_err());
    }

    #[test]
    fn listing_wins_over_launching() {
        assert!(matches!(
            parse(["basalt", "-L"], Origin::Activation),
            Ok(Request::List)
        ));
        assert!(matches!(
            parse(["basalt", "--list"], Origin::Activation),
            Ok(Request::List)
        ));
        assert!(matches!(
            parse(["basalt", "--list", "-l", "Vanilla"], Origin::Activation),
            Ok(Request::List)
        ));
    }

    #[test]
    fn arguments_the_desktop_adds_are_ignored() {
        assert_eq!(selector(["basalt"]).unwrap(), None);
        assert_eq!(selector(["basalt", "--gtk-module=x"]).unwrap(), None);
        assert_eq!(selector(["basalt", "-psn_0_1234"]).unwrap(), None);
        assert_eq!(
            selector(["basalt", "--gtk-module=x", "-l", "Vanilla"]).unwrap(),
            Some("Vanilla".to_string())
        );
    }

    #[test]
    fn resolves_unique_names_and_id_prefixes() {
        assert_eq!(
            resolve_selector(INSTANCES, "Vanilla").unwrap(),
            INSTANCES[0].id
        );
        assert_eq!(
            resolve_selector(INSTANCES, "4f9c2a81").unwrap(),
            INSTANCES[0].id
        );
        assert_eq!(
            resolve_selector(INSTANCES, "4f9c2a81111122223333444444444444").unwrap(),
            INSTANCES[0].id
        );
    }

    #[test]
    fn rejects_duplicate_names_and_ambiguous_prefixes() {
        assert!(resolve_selector(INSTANCES, "Shared").is_err());
        assert!(resolve_selector(INSTANCES, "abcd1234").is_err());
    }

    #[test]
    fn extends_colliding_prefixes() {
        assert_eq!(unique_prefix(INSTANCES, INSTANCES[0].id), "4f9c2a81");
        assert_eq!(unique_prefix(INSTANCES, INSTANCES[1].id), "abcd12341");
    }
}
