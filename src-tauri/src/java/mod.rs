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

pub async fn detect(explicit: Option<&str>) -> Option<JavaInfo> {
    if let Some(path) = explicit {
        if let Some(info) = probe(path).await {
            return Some(info);
        }
    }
    if let Ok(home) = std::env::var("JAVA_HOME") {
        if let Some(info) = probe(&format!("{home}/bin/java")).await {
            return Some(info);
        }
    }
    probe("java").await
}

#[cfg(test)]
mod tests {
    use super::parse_major;

    #[test]
    fn parses_legacy_and_modern() {
        assert_eq!(parse_major("openjdk version \"1.8.0_292\""), Some(8));
        assert_eq!(parse_major("openjdk version \"17.0.1\" 2021"), Some(17));
        assert_eq!(parse_major("java version \"21\""), Some(21));
        assert_eq!(parse_major("openjdk version \"11.0.2\""), Some(11));
    }
}
