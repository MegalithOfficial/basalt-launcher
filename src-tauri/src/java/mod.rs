use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::files::FileManager;

pub mod managed;

#[derive(Debug, Clone, Serialize)]
pub struct JavaInfo {
    pub path: String,
    pub major: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct JavaStatus {
    pub required_major: u32,
    pub found: Option<JavaInfo>,
    pub ok: bool,
}

fn parse_major(text: &str) -> Option<u32> {
    let start = text.find("version \"")? + "version \"".len();
    let rest = &text[start..];
    let end = rest.find('"')?;
    let version = rest[..end].trim_start_matches("1.");
    let digits: String = version.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

pub async fn probe(path: &str) -> Option<JavaInfo> {
    let output = tokio::process::Command::new(path)
        .arg("-version")
        .output()
        .await
        .ok()?;
    let mut text = String::from_utf8_lossy(&output.stderr).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    let major = parse_major(&text)?;
    Some(JavaInfo {
        path: path.to_string(),
        major,
    })
}

fn java_binary() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub fn runtime_binaries_in(files: &FileManager, root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = files.read_external_dir(root) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for base in entries {
        for relative in ["bin", "Contents/Home/bin", "jre/bin"] {
            let bin = base.join(relative).join(java_binary());
            if files.is_external_file(&bin) {
                found.push(bin);
            }
        }
    }
    found
}

fn install_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = [
        "/usr/lib/jvm",
        "/usr/lib64/jvm",
        "/usr/java",
        "/opt/java",
        "/Library/Java/JavaVirtualMachines",
        "/System/Library/Java/JavaVirtualMachines",
        "C:\\Program Files\\Java",
        "C:\\Program Files\\Eclipse Adoptium",
        "C:\\Program Files\\Microsoft",
        "C:\\Program Files\\Zulu",
        "C:\\Program Files (x86)\\Java",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();

    if let Some(home) = home_dir() {
        for relative in [
            ".sdkman/candidates/java",
            ".asdf/installs/java",
            ".jdks",
            ".gradle/jdks",
            ".jenv/versions",
            ".local/share/mise/installs/java",
            "Library/Java/JavaVirtualMachines",
            "scoop/apps",
        ] {
            roots.push(home.join(relative));
        }
    }
    roots
}

async fn candidates(files: &FileManager, explicit: Option<&str>) -> Vec<JavaInfo> {
    let mut paths: Vec<String> = Vec::new();
    let managed_root = files.paths().runtimes();
    if let Some(path) = explicit {
        paths.push(path.to_string());
    }
    if let Ok(home) = std::env::var("JAVA_HOME") {
        paths.push(
            PathBuf::from(home)
                .join("bin")
                .join(java_binary())
                .display()
                .to_string(),
        );
    }
    paths.push(java_binary().to_string());
    if let Some(home) = home_dir() {
        paths.push(
            home.join(".nix-profile/bin")
                .join(java_binary())
                .display()
                .to_string(),
        );
    }
    paths.extend(
        runtime_binaries_in(files, &managed_root)
            .into_iter()
            .map(|path| path.display().to_string()),
    );
    for root in install_roots() {
        paths.extend(
            runtime_binaries_in(files, &root)
                .into_iter()
                .map(|p| p.display().to_string()),
        );
    }

    let mut seen = std::collections::HashSet::new();
    paths.retain(|path| {
        let key = files
            .canonicalize_external(path)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| path.clone());
        seen.insert(key)
    });

    let mut found: Vec<JavaInfo> = Vec::new();
    for path in paths {
        match probe(&path).await {
            Some(info) => found.push(info),
            None => tracing::trace!(path, "not a usable java runtime"),
        }
    }
    tracing::debug!(count = found.len(), "probed java runtimes");
    found
}

pub async fn list_all(files: &FileManager) -> Vec<JavaInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for info in candidates(files, None).await {
        let canonical = files
            .canonicalize_external(&info.path)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| info.path.clone());
        if seen.insert(canonical.clone()) {
            result.push(JavaInfo {
                path: canonical,
                major: info.major,
            });
        }
    }
    result.sort_by_key(|runtime| std::cmp::Reverse(runtime.major));
    result
}

pub fn pick(found: &[JavaInfo], required: u32) -> Option<JavaInfo> {
    found
        .iter()
        .filter(|j| j.major >= required)
        .min_by_key(|j| j.major)
        .or_else(|| found.iter().max_by_key(|j| j.major))
        .cloned()
}

pub async fn find_for_major(
    files: &FileManager,
    required: u32,
    explicit: Option<&str>,
) -> Option<JavaInfo> {
    if let Some(path) = explicit.map(str::trim).filter(|p| !p.is_empty()) {
        match probe(path).await {
            Some(info) => {
                tracing::info!(
                    path,
                    major = info.major,
                    required,
                    "using the pinned java runtime"
                );
                return Some(info);
            }
            None => tracing::warn!(
                path,
                "the pinned java runtime could not be run, falling back to detection"
            ),
        }
    }

    let found = candidates(files, None).await;
    let picked = pick(&found, required);
    match &picked {
        Some(java) => {
            tracing::debug!(required, major = java.major, path = %java.path, "java selected")
        }
        None => tracing::warn!(required, "no java runtime found on this system"),
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::{install_roots, java_binary, parse_major, pick, runtime_binaries_in, JavaInfo};
    use crate::{files::FileManager, paths::Paths};

    fn java(major: u32) -> JavaInfo {
        JavaInfo {
            path: format!("/usr/lib/jvm/java-{major}/bin/java"),
            major,
        }
    }

    #[test]
    fn finds_runtimes_in_every_supported_layout() {
        let root = std::env::temp_dir().join(format!("basalt-jvm-{}", uuid::Uuid::new_v4()));
        for relative in [
            "jdk-21/bin",
            "zulu-17/Contents/Home/bin",
            "legacy-8/jre/bin",
        ] {
            let dir = root.join(relative);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(java_binary()), b"").unwrap();
        }
        std::fs::create_dir_all(root.join("not-a-jdk/share")).unwrap();

        let files = FileManager::new(Paths::plain(root.clone())).unwrap();
        let found = runtime_binaries_in(&files, &root);
        assert_eq!(found.len(), 3, "found: {found:?}");
        assert!(found.iter().all(|p| p.ends_with(java_binary())));
        assert!(runtime_binaries_in(&files, &root.join("missing")).is_empty());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn looks_in_version_manager_directories() {
        let roots = install_roots();
        let rendered: Vec<String> = roots.iter().map(|r| r.display().to_string()).collect();
        for expected in [".sdkman", ".asdf", ".jdks", "mise", "/usr/lib/jvm"] {
            assert!(
                rendered.iter().any(|r| r.contains(expected)),
                "no root covering {expected} in {rendered:?}"
            );
        }
    }

    #[test]
    fn prefers_the_closest_runtime_that_meets_the_requirement() {
        let found = vec![java(8), java(17), java(21), java(25)];
        assert_eq!(pick(&found, 17).unwrap().major, 17);
        assert_eq!(
            pick(&found, 18).unwrap().major,
            21,
            "should not jump past 21 to 25"
        );
        assert_eq!(pick(&found, 8).unwrap().major, 8);
        assert!(pick(&[], 21).is_none());
    }

    #[test]
    fn falls_back_to_the_newest_when_nothing_is_new_enough() {
        let found = vec![java(8), java(17)];
        assert_eq!(pick(&found, 21).unwrap().major, 17);
    }

    #[test]
    fn does_not_depend_on_the_order_runtimes_were_found() {
        let ordered = vec![java(8), java(17), java(21), java(25)];
        let shuffled = vec![java(25), java(8), java(21), java(17)];
        for required in [8, 17, 18, 21, 26] {
            assert_eq!(
                pick(&ordered, required).unwrap().major,
                pick(&shuffled, required).unwrap().major,
                "required {required} depended on discovery order"
            );
        }
    }

    #[test]
    fn parses_legacy_and_modern() {
        assert_eq!(parse_major("openjdk version \"1.8.0_292\""), Some(8));
        assert_eq!(parse_major("openjdk version \"17.0.1\" 2021"), Some(17));
        assert_eq!(parse_major("java version \"21\""), Some(21));
        assert_eq!(parse_major("openjdk version \"11.0.2\""), Some(11));
    }
}
