use std::collections::HashMap;

use super::TextProblem;

#[derive(Debug, Clone)]
enum Line {
    Kept(String),
    Entry {
        indent: String,
        key_raw: String,
        key: String,
        separator: String,
        value_raw: String,
        value: String,
        edited: bool,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Properties {
    lines: Vec<Line>,
}

impl Properties {
    pub fn parse(bytes: &[u8]) -> Self {
        let mut lines = Vec::new();
        let mut pending: Option<String> = None;
        for raw in split_lines(&decode_latin1(bytes)) {
            let carried = match pending.take() {
                Some(previous) => format!("{previous}\n{raw}"),
                None => raw.clone(),
            };
            if continues(&raw) {
                pending = Some(carried);
                continue;
            }
            if carried.contains('\n') {
                lines.push(Line::Kept(carried));
                continue;
            }
            lines.push(parse_line(carried));
        }
        if let Some(dangling) = pending {
            lines.push(Line::Kept(dangling));
        }
        Self { lines }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().rev().find_map(|line| match line {
            Line::Entry {
                key: name, value, ..
            } if name == key => Some(value.as_str()),
            _ => None,
        })
    }

    pub fn entries(&self) -> Vec<(&str, &str)> {
        self.lines
            .iter()
            .filter_map(|line| match line {
                Line::Entry {
                    key: name, value, ..
                } => Some((name.as_str(), value.as_str())),
                _ => None,
            })
            .collect()
    }

    pub fn set(&mut self, key: &str, value: &str) {
        for line in self.lines.iter_mut() {
            if let Line::Entry {
                key: name,
                value: current,
                edited,
                ..
            } = line
            {
                if name == key {
                    if current != value {
                        *current = value.to_string();
                        *edited = true;
                    }
                    return;
                }
            }
        }
        self.lines.push(Line::Entry {
            indent: String::new(),
            key_raw: encode_key(key),
            key: key.to_string(),
            separator: "=".to_string(),
            value_raw: String::new(),
            value: value.to_string(),
            edited: true,
        });
    }

    pub fn remove(&mut self, key: &str) -> bool {
        let before = self.lines.len();
        self.lines
            .retain(|line| !matches!(line, Line::Entry { key: name, .. } if name == key));
        self.lines.len() != before
    }

    pub fn render(&self) -> Vec<u8> {
        let mut text = String::new();
        for line in &self.lines {
            match line {
                Line::Kept(raw) => text.push_str(raw),
                Line::Entry {
                    indent,
                    key_raw,
                    separator,
                    value_raw,
                    value,
                    edited,
                    ..
                } => {
                    text.push_str(indent);
                    text.push_str(key_raw);
                    text.push_str(separator);
                    if *edited {
                        text.push_str(&encode_value(value));
                    } else {
                        text.push_str(value_raw);
                    }
                }
            }
            text.push('\n');
        }
        encode_latin1(&text)
    }
}

pub fn validate(text: &str) -> Option<TextProblem> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let trimmed = raw.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }
        if let Some(column) = malformed_escape(raw) {
            return Some(TextProblem {
                line: number,
                column,
                message: "This backslash is not a valid escape. Use \\\\ for a backslash and \\uXXXX for a unicode character.".to_string(),
            });
        }
        if continues(raw) {
            continue;
        }
        let Line::Entry { key, .. } = parse_line(raw.to_string()) else {
            continue;
        };
        if let Some(first) = seen.insert(key.clone(), number) {
            return Some(TextProblem {
                line: number,
                column: raw.len() - trimmed.len() + 1,
                message: format!("{key} is already set on line {first}, so this one wins and that one is ignored."),
            });
        }
    }
    None
}

pub fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| *byte as char).collect()
}

pub fn encode_latin1(text: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(text.len());
    for character in text.chars() {
        if (character as u32) < 0x100 {
            bytes.push(character as u8);
        } else {
            bytes.extend_from_slice(escape_unicode(character).as_bytes());
        }
    }
    bytes
}

fn escape_unicode(character: char) -> String {
    let mut buffer = [0u16; 2];
    character
        .encode_utf16(&mut buffer)
        .iter()
        .map(|unit| format!("\\u{unit:04X}"))
        .collect()
}

fn split_lines(text: &str) -> Vec<String> {
    let mut lines = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect::<Vec<_>>();
    if lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

fn continues(line: &str) -> bool {
    line.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1
}

fn parse_line(raw: String) -> Line {
    let indent_len = raw.len() - raw.trim_start().len();
    let rest = &raw[indent_len..];
    if rest.is_empty() || rest.starts_with('#') || rest.starts_with('!') {
        return Line::Kept(raw);
    }

    let mut key_end = rest.len();
    let mut escaped = false;
    for (offset, character) in rest.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '=' | ':' | ' ' | '\t' | '\u{c}' => {
                key_end = offset;
                break;
            }
            _ => {}
        }
    }

    let after_key = &rest[key_end..];
    let mut separator_end = after_key
        .find(|c: char| !matches!(c, ' ' | '\t' | '\u{c}'))
        .unwrap_or(after_key.len());
    if after_key[separator_end..].starts_with(['=', ':']) {
        separator_end += 1;
        separator_end += after_key[separator_end..]
            .find(|c: char| !matches!(c, ' ' | '\t' | '\u{c}'))
            .unwrap_or(after_key.len() - separator_end);
    }

    let key_raw = &rest[..key_end];
    let separator = &after_key[..separator_end];
    let value_raw = &after_key[separator_end..];
    Line::Entry {
        indent: raw[..indent_len].to_string(),
        key: unescape(key_raw),
        key_raw: key_raw.to_string(),
        separator: separator.to_string(),
        value: unescape(value_raw),
        value_raw: value_raw.to_string(),
        edited: false,
    }
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('f') => out.push('\u{c}'),
            Some('u') => {
                let digits = chars.by_ref().take(4).collect::<String>();
                match u32::from_str_radix(&digits, 16)
                    .ok()
                    .and_then(char::from_u32)
                {
                    Some(decoded) => out.push(decoded),
                    None => {
                        out.push_str("\\u");
                        out.push_str(&digits);
                    }
                }
            }
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn malformed_escape(line: &str) -> Option<usize> {
    let characters = line.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < characters.len() {
        if characters[index] != '\\' {
            index += 1;
            continue;
        }
        match characters.get(index + 1) {
            Some('u') => {
                let digits = characters
                    .get(index + 2..index + 6)
                    .map(|slice| slice.iter().collect::<String>());
                let valid = digits
                    .as_deref()
                    .map(|value| value.chars().all(|c| c.is_ascii_hexdigit()))
                    .unwrap_or(false);
                if !valid {
                    return Some(index + 1);
                }
                index += 6;
            }
            Some(_) => index += 2,
            None => index += 1,
        }
    }
    None
}

fn encode_key(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    for character in key.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            ' ' => out.push_str("\\ "),
            '=' => out.push_str("\\="),
            ':' => out.push_str("\\:"),
            '#' => out.push_str("\\#"),
            '!' => out.push_str("\\!"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

fn encode_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for (index, character) in value.chars().enumerate() {
        match character {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{c}' => out.push_str("\\f"),
            ' ' if index == 0 => out.push_str("\\ "),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "#Minecraft server properties\n\
                          #Mon Aug 11 20:21:43 CEST 2026\n\
                          enable-jmx-monitoring=false\n\
                          rcon.port=25575\n\
                          level-seed=\n\
                          motd=A Minecraft Server\n\
                          \n\
                          server-port=25565\n\
                          # a plugin wrote this\n\
                          custom.plugin-key: yes\n";

    #[test]
    fn an_untouched_file_renders_back_byte_for_byte() {
        let properties = Properties::parse(SAMPLE.as_bytes());
        assert_eq!(properties.render(), SAMPLE.as_bytes());
    }

    #[test]
    fn editing_one_key_leaves_every_other_line_alone() {
        let mut properties = Properties::parse(SAMPLE.as_bytes());
        properties.set("motd", "Basalt survival");

        let rendered = String::from_utf8(properties.render()).unwrap();

        assert!(rendered.contains("motd=Basalt survival\n"));
        assert_eq!(
            rendered
                .lines()
                .filter(|line| *line != "motd=Basalt survival")
                .count(),
            SAMPLE
                .lines()
                .filter(|line| !line.starts_with("motd="))
                .count()
        );
        assert!(rendered.contains("#Minecraft server properties"));
        assert!(rendered.contains("# a plugin wrote this"));
    }

    #[test]
    fn keys_nobody_knows_about_survive_and_stay_readable() {
        let mut properties = Properties::parse(SAMPLE.as_bytes());
        assert_eq!(properties.get("custom.plugin-key"), Some("yes"));
        assert_eq!(properties.get("rcon.port"), Some("25575"));
        assert_eq!(properties.get("level-seed"), Some(""));

        properties.set("server-port", "25570");
        let reparsed = Properties::parse(&properties.render());

        assert_eq!(reparsed.get("custom.plugin-key"), Some("yes"));
        assert_eq!(reparsed.get("server-port"), Some("25570"));
        assert_eq!(reparsed.entries().len(), 6);
    }

    #[test]
    fn a_new_key_lands_at_the_end() {
        let mut properties = Properties::parse(b"motd=hi\n");
        properties.set("white-list", "true");

        assert_eq!(
            String::from_utf8(properties.render()).unwrap(),
            "motd=hi\nwhite-list=true\n"
        );
    }

    #[test]
    fn unicode_survives_the_round_trip_as_escapes() {
        let properties = Properties::parse(b"motd=Welcome \\u00A7cback caf\\u00e9\n");
        assert_eq!(properties.get("motd"), Some("Welcome §cback café"));

        let mut edited = properties.clone();
        edited.set("motd", "Welcome §aback café ✦");

        assert_eq!(
            edited.render(),
            b"motd=Welcome \xa7aback caf\xe9 \\u2726\n".to_vec(),
            "latin1 goes out as raw bytes and anything above it as an escape"
        );
        assert_eq!(
            Properties::parse(&edited.render()).get("motd"),
            Some("Welcome §aback café ✦")
        );
    }

    #[test]
    fn spacing_around_the_separator_is_preserved() {
        let mut properties = Properties::parse(b"  spaced   :   value  \nplain\n");
        assert_eq!(properties.get("spaced"), Some("value  "));
        assert_eq!(properties.get("plain"), Some(""));

        properties.set("spaced", "other");
        assert_eq!(
            String::from_utf8(properties.render()).unwrap(),
            "  spaced   :   other\nplain\n"
        );
    }

    #[test]
    fn a_continued_line_is_kept_exactly_as_written() {
        let source = "motd=one \\\n  two\nport=1\n";
        let properties = Properties::parse(source.as_bytes());

        assert_eq!(properties.render(), source.as_bytes());
        assert_eq!(properties.get("port"), Some("1"));
    }

    #[test]
    fn removing_a_key_takes_its_line_with_it() {
        let mut properties = Properties::parse(b"a=1\nb=2\n");
        assert!(properties.remove("a"));
        assert!(!properties.remove("a"));
        assert_eq!(String::from_utf8(properties.render()).unwrap(), "b=2\n");
    }

    #[test]
    fn validation_catches_broken_escapes_and_duplicates() {
        assert!(validate(SAMPLE).is_none());
        assert!(validate("motd=a \\u00A7c\n").is_none());

        let broken = validate("motd=a \\u00ZZ\n").unwrap();
        assert_eq!(broken.line, 1);
        assert_eq!(broken.column, 8);

        let duplicate = validate("port=1\nmotd=hi\nport=2\n").unwrap();
        assert_eq!(duplicate.line, 3);
        assert!(duplicate.message.contains("line 1"));
    }
}
