use serde::Serialize;

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

async fn candidates(explicit: Option<&str>) -> Vec<JavaInfo> {
    let mut paths: Vec<String> = Vec::new();
    if let Some(path) = explicit {
        paths.push(path.to_string());
    }
    if let Ok(home) = std::env::var("JAVA_HOME") {
        paths.push(format!("{home}/bin/java"));
    }
    paths.push("java".to_string());
    for base in ["/usr/lib/jvm", "/usr/lib64/jvm", "/opt/java"] {
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let bin = entry.path().join("bin/java");
                if bin.is_file() {
                    paths.push(bin.display().to_string());
                }
            }
        }
    }

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

pub async fn detect(explicit: Option<&str>) -> Option<JavaInfo> {
    candidates(explicit).await.into_iter().next()
}

pub async fn list_all() -> Vec<JavaInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for info in candidates(None).await {
        let canonical = std::fs::canonicalize(&info.path)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| info.path.clone());
        if seen.insert(canonical.clone()) {
            result.push(JavaInfo {
                path: canonical,
                major: info.major,
            });
        }
    }
    result.sort_by(|a, b| b.major.cmp(&a.major));
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

pub async fn find_for_major(required: u32, explicit: Option<&str>) -> Option<JavaInfo> {
    if let Some(path) = explicit.map(str::trim).filter(|p| !p.is_empty()) {
        match probe(path).await {
            Some(info) => {
                tracing::info!(path, major = info.major, required, "using the pinned java runtime");
                return Some(info);
            }
            None => tracing::warn!(
                path,
                "the pinned java runtime could not be run, falling back to detection"
            ),
        }
    }

    let found = candidates(None).await;
    let picked = pick(&found, required);
    match &picked {
        Some(java) => tracing::debug!(required, major = java.major, path = %java.path, "java selected"),
        None => tracing::warn!(required, "no java runtime found on this system"),
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::{parse_major, pick, JavaInfo};

    fn java(major: u32) -> JavaInfo {
        JavaInfo {
            path: format!("/usr/lib/jvm/java-{major}/bin/java"),
            major,
        }
    }

    #[test]
    fn prefers_the_closest_runtime_that_meets_the_requirement() {
        let found = vec![java(8), java(17), java(21), java(25)];
        assert_eq!(pick(&found, 17).unwrap().major, 17);
        assert_eq!(pick(&found, 18).unwrap().major, 21, "should not jump past 21 to 25");
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
