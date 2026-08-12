use std::path::{Path, PathBuf};

pub const CANDIDATES: [&str; 5] = ["startserver", "LaunchServer", "start", "run", "ServerStart"];

pub fn extensions() -> &'static [&'static str] {
    if cfg!(windows) {
        &["bat", "cmd", "ps1"]
    } else {
        &["sh", "command"]
    }
}

pub fn find(dir: &Path) -> Option<String> {
    let wanted = extensions();
    let entries = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    for stem in CANDIDATES {
        let found = entries.iter().find(|name| {
            let Some((base, ext)) = name.rsplit_once('.') else {
                return false;
            };
            base.eq_ignore_ascii_case(stem)
                && wanted
                    .iter()
                    .any(|allowed| ext.eq_ignore_ascii_case(allowed))
        });
        if let Some(name) = found {
            return Some(name.clone());
        }
    }
    None
}

pub fn command(dir: &Path, script: &str) -> (PathBuf, Vec<String>) {
    let path = dir.join(script);
    if cfg!(windows) {
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ps1"))
        {
            return (
                PathBuf::from("powershell.exe"),
                vec![
                    "-NoProfile".to_string(),
                    "-ExecutionPolicy".to_string(),
                    "Bypass".to_string(),
                    "-File".to_string(),
                    path.display().to_string(),
                ],
            );
        }
        (
            PathBuf::from("cmd"),
            vec!["/c".to_string(), path.display().to_string()],
        )
    } else {
        (path, Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("basalt-startup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in files {
            std::fs::write(dir.join(name), b"#!/bin/sh\n").unwrap();
        }
        dir
    }

    #[test]
    fn the_first_name_the_list_knows_wins() {
        let dir = sandbox(&["run.sh", "run.bat", "startserver.sh", "startserver.bat"]);
        let found = find(&dir).unwrap();
        if cfg!(windows) {
            assert_eq!(found, "startserver.bat");
        } else {
            assert_eq!(found, "startserver.sh");
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn the_other_spelling_windows_uses_counts_too() {
        let dir = sandbox(&["startserver.cmd", "startserver.command"]);
        let found = find(&dir);
        if cfg!(windows) {
            assert_eq!(found.as_deref(), Some("startserver.cmd"));
        } else {
            assert_eq!(found.as_deref(), Some("startserver.command"));
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_script_for_the_other_platform_is_ignored() {
        let only_windows = sandbox(&["startserver.bat"]);
        let only_unix = sandbox(&["startserver.sh"]);
        if cfg!(windows) {
            assert_eq!(find(&only_windows).as_deref(), Some("startserver.bat"));
            assert!(find(&only_unix).is_none());
        } else {
            assert_eq!(find(&only_unix).as_deref(), Some("startserver.sh"));
            assert!(find(&only_windows).is_none());
        }
        std::fs::remove_dir_all(only_windows).ok();
        std::fs::remove_dir_all(only_unix).ok();
    }

    #[test]
    fn a_folder_with_nothing_to_run_says_so() {
        let dir = sandbox(&["server.jar", "notes.txt"]);
        assert!(find(&dir).is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn the_script_is_handed_to_the_shell_the_platform_uses() {
        let dir = sandbox(&["startserver.sh"]);
        let (program, args) = command(&dir, "startserver.sh");
        if cfg!(windows) {
            assert_eq!(program, PathBuf::from("cmd"));
            assert_eq!(args[0], "/c");
        } else {
            assert_eq!(program, dir.join("startserver.sh"));
            assert!(args.is_empty());
        }
        assert!(
            program.ends_with("startserver.sh") || args.last().unwrap().ends_with("startserver.sh")
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
