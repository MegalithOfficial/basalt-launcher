pub const PLACEHOLDER: &str = "[redacted]";

const HOME_MARKERS: &[&str] = &["/home/", "/Users/", "\\Users\\", "/var/home/"];

const FLAGS: &[&str] = &[
    "--accessToken",
    "--session",
    "--uuid",
    "--clientId",
    "--xuid",
    "--password",
];

const KEYS: &[&str] = &[
    "access_token",
    "refresh_token",
    "accesstoken",
    "id_token",
    "session_id",
    "api_key",
    "apikey",
    "x-api-key",
    "authorization",
    "password",
    "passwd",
    "secret",
];

fn earliest<'a>(text: &str, markers: &[&'a str]) -> Option<(usize, &'a str)> {
    markers
        .iter()
        .filter_map(|marker| text.find(marker).map(|at| (at, *marker)))
        .min_by_key(|(at, _)| *at)
}

fn is_path_break(character: char) -> bool {
    matches!(character, '/' | '\\' | '"' | '\'' | ':' | ',' | ')' | ']')
        || character.is_whitespace()
}

fn is_value_break(character: char) -> bool {
    matches!(character, '"' | '\'' | ',' | '}' | ']' | '&' | ';' | ')') || character.is_whitespace()
}

fn take_while_not(text: &str, stop: fn(char) -> bool) -> (&str, &str) {
    let end = text.find(stop).unwrap_or(text.len());
    text.split_at(end)
}

fn home_paths(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some((at, marker)) = earliest(rest, HOME_MARKERS) {
        out.push_str(&rest[..at + marker.len()]);
        let (name, tail) = take_while_not(&rest[at + marker.len()..], is_path_break);
        if name.is_empty() {
            out.push_str(name);
        } else {
            out.push_str("user");
        }
        rest = tail;
    }
    out.push_str(rest);
    out
}

fn flag_values(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some((at, flag)) = earliest(rest, FLAGS) {
        out.push_str(&rest[..at + flag.len()]);
        let after = &rest[at + flag.len()..];
        let gap = after.len() - after.trim_start_matches([' ', '=', ':']).len();
        out.push_str(&after[..gap]);
        let (value, tail) = take_while_not(&after[gap..], |c| c.is_whitespace());
        if value.is_empty() {
            rest = tail;
            continue;
        }
        out.push_str(PLACEHOLDER);
        rest = tail;
    }
    out.push_str(rest);
    out
}

fn key_values(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let mut out = String::with_capacity(line.len());
    let mut cursor = 0usize;
    while let Some((at, key)) = earliest(&lower[cursor..], KEYS) {
        let start = cursor + at + key.len();
        out.push_str(&line[cursor..start]);
        let after = &line[start..];
        let gap = after.len() - after.trim_start_matches(['"', '\'', ':', '=', ' ']).len();
        if gap == 0 {
            cursor = start;
            continue;
        }
        out.push_str(&after[..gap]);
        let (value, _) = take_while_not(&after[gap..], is_value_break);
        if value.is_empty() {
            cursor = start + gap;
            continue;
        }
        out.push_str(PLACEHOLDER);
        cursor = start + gap + value.len();
    }
    out.push_str(&line[cursor..]);
    out
}

fn bearer_tokens(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find("Bearer ") {
        out.push_str(&rest[..at + "Bearer ".len()]);
        let (value, tail) = take_while_not(&rest[at + "Bearer ".len()..], is_value_break);
        if !value.is_empty() {
            out.push_str(PLACEHOLDER);
        }
        rest = tail;
    }
    out.push_str(rest);
    out
}

fn looks_like_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    match bytes.len() {
        36 => bytes.iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        }),
        32 => bytes.iter().all(u8::is_ascii_hexdigit),
        _ => false,
    }
}

fn uuids(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while !rest.is_empty() {
        let start = rest
            .find(|c: char| c.is_ascii_hexdigit())
            .unwrap_or(rest.len());
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        if rest.is_empty() {
            break;
        }
        let (run, tail) = take_while_not(rest, |c| !(c.is_ascii_hexdigit() || c == '-'));
        if looks_like_uuid(run) {
            out.push_str(PLACEHOLDER);
        } else {
            out.push_str(run);
        }
        rest = tail;
    }
    out
}

fn url_credentials(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find("://") {
        out.push_str(&rest[..at + 3]);
        let after = &rest[at + 3..];
        let (authority, tail) = take_while_not(after, |c| c == '/' || c.is_whitespace());
        match authority
            .find('@')
            .and_then(|end| authority[..end].find(':').map(|colon| (colon, end)))
        {
            Some((colon, end)) => {
                out.push_str(&authority[..colon + 1]);
                out.push_str(PLACEHOLDER);
                out.push_str(&authority[end..]);
            }
            None => out.push_str(authority),
        }
        rest = tail;
    }
    out.push_str(rest);
    out
}

pub fn redact(text: &str, secrets: &[String]) -> String {
    let mut carried = text.to_string();
    for secret in secrets {
        let secret = secret.trim();
        if secret.len() < 6 {
            continue;
        }
        carried = carried.replace(secret, PLACEHOLDER);
    }
    carried
        .lines()
        .map(|line| {
            let line = home_paths(line);
            let line = url_credentials(&line);
            let line = flag_values(&line);
            let line = key_values(&line);
            let line = bearer_tokens(&line);
            uuids(&line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(text: &str) -> String {
        redact(text, &[])
    }

    #[test]
    fn the_home_folder_keeps_its_shape_but_loses_the_name() {
        assert_eq!(
            clean("Loading /home/megalith/.basalt/instances/pack/mods"),
            "Loading /home/user/.basalt/instances/pack/mods"
        );
        assert_eq!(
            clean("at C:\\Users\\Ela\\AppData\\Roaming\\basalt"),
            "at C:\\Users\\user\\AppData\\Roaming\\basalt"
        );
    }

    #[test]
    fn the_session_arguments_never_reach_the_paste() {
        let line = "--username Steve --accessToken eyJhbGciOiJIUzI1NiJ9.payload --uuid 069a79f4-44e9-4726-a5be-fca90e38aaf5";
        let cleaned = clean(line);
        assert!(cleaned.contains("--username Steve"));
        assert!(!cleaned.contains("eyJhbGciOiJIUzI1NiJ9.payload"));
        assert!(!cleaned.contains("069a79f4"));
    }

    #[test]
    fn a_token_in_a_json_body_is_removed_with_its_key_left_readable() {
        let cleaned = clean(r#"{"access_token":"ya29.a0Af","expires_in":86400}"#);
        assert!(cleaned.starts_with(r#"{"access_token":"[redacted]"#));
        assert!(cleaned.contains("expires_in"));
        assert!(!cleaned.contains("ya29.a0Af"));
    }

    #[test]
    fn a_proxy_password_in_a_url_is_removed() {
        assert_eq!(
            clean("proxy http://megalith:hunter2@10.0.0.1:8080 ready"),
            "proxy http://megalith:[redacted]@10.0.0.1:8080 ready"
        );
    }

    #[test]
    fn a_bare_player_uuid_is_removed() {
        let cleaned = clean("Setting user: Steve (069a79f444e94726a5befca90e38aaf5)");
        assert_eq!(cleaned, "Setting user: Steve ([redacted])");
    }

    #[test]
    fn known_secrets_are_removed_wherever_they_appear() {
        let cleaned = redact(
            "GET /v1/mods failed for key $2a$10$abcdefgh",
            &["$2a$10$abcdefgh".to_string()],
        );
        assert_eq!(cleaned, "GET /v1/mods failed for key [redacted]");
    }

    #[test]
    fn an_ordinary_stack_trace_survives_untouched() {
        let trace = "java.lang.NullPointerException\n\tat net.minecraft.client.Main.run(Main.java:212)\n\tat java.base/java.lang.Thread.run(Thread.java:1583)";
        assert_eq!(clean(trace), trace);
    }
}
