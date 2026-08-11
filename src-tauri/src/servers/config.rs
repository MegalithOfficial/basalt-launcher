use std::path::Path;

use toml_edit::{DocumentMut, Item, Table, Value};

use crate::{
    error::{Error, Result},
    files::FileManager,
};

use super::{properties::Properties, Server};

pub struct Entry {
    pub key: String,
    pub value: String,
}

pub struct Config {
    pub entries: Vec<Entry>,
    pub port: Option<u16>,
    pub motd: Option<String>,
    pub max_players: Option<u32>,
}

pub fn read(files: &FileManager, server: &Server) -> Result<Config> {
    let file = server.flavor.config_file();
    let path = Path::new(&server.dir).join(file);
    let bytes = files.read(&path).unwrap_or_default();

    if file.ends_with(".toml") {
        return read_toml(file, &String::from_utf8_lossy(&bytes));
    }
    Ok(read_properties(&Properties::parse(&bytes)))
}

pub fn write(
    files: &FileManager,
    server: &Server,
    changes: &[Entry],
    removed: &[String],
) -> Result<Config> {
    let file = server.flavor.config_file();
    let path = Path::new(&server.dir).join(file);
    let bytes = files.read(&path).unwrap_or_default();

    let rendered = if file.ends_with(".toml") {
        write_toml(&String::from_utf8_lossy(&bytes), changes, removed)?
    } else {
        let mut properties = Properties::parse(&bytes);
        for change in changes {
            properties.set(change.key.trim(), &change.value);
        }
        for key in removed {
            properties.remove(key.trim());
        }
        properties.render()
    };

    files.write_atomic(&path, &rendered)?;
    read(files, server)
}

fn read_properties(properties: &Properties) -> Config {
    Config {
        entries: properties
            .entries()
            .into_iter()
            .map(|(key, value)| Entry {
                key: key.to_string(),
                value: value.to_string(),
            })
            .collect(),
        port: properties
            .get("server-port")
            .and_then(|value| value.trim().parse().ok()),
        motd: properties.get("motd").map(str::to_string),
        max_players: properties
            .get("max-players")
            .and_then(|value| value.trim().parse().ok()),
    }
}

fn read_toml(file: &'static str, text: &str) -> Result<Config> {
    let document: DocumentMut = text
        .parse()
        .map_err(|error| Error::other(format!("{file} does not parse: {error}")))?;
    let mut entries = Vec::new();
    flatten(document.as_table(), "", &mut entries);

    let port = entries
        .iter()
        .find(|entry| entry.key.ends_with("address"))
        .and_then(|entry| entry.value.rsplit_once(':'))
        .and_then(|(_, port)| port.trim().parse().ok());
    let motd = entries
        .iter()
        .find(|entry| entry.key == "motd" || entry.key.ends_with(".motd"))
        .map(|entry| entry.value.clone());
    let max_players = entries
        .iter()
        .find(|entry| entry.key.ends_with("max_players"))
        .and_then(|entry| entry.value.trim().parse().ok());

    Ok(Config {
        entries,
        port,
        motd,
        max_players,
    })
}

fn flatten(table: &Table, prefix: &str, entries: &mut Vec<Entry>) {
    for (key, item) in table.iter() {
        let path = if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{prefix}.{key}")
        };
        match item {
            Item::Table(inner) => flatten(inner, &path, entries),
            Item::Value(Value::InlineTable(inner)) => {
                let mut nested = Table::new();
                for (key, value) in inner.iter() {
                    nested.insert(key, Item::Value(value.clone()));
                }
                flatten(&nested, &path, entries);
            }
            Item::Value(value) => entries.push(Entry {
                key: path,
                value: shown(value),
            }),
            _ => {}
        }
    }
}

fn shown(value: &Value) -> String {
    match value {
        Value::String(text) => text.value().clone(),
        other => other.to_string().trim().to_string(),
    }
}

fn write_toml(text: &str, changes: &[Entry], removed: &[String]) -> Result<Vec<u8>> {
    let mut document: DocumentMut = text
        .parse()
        .map_err(|error| Error::other(format!("The file does not parse: {error}")))?;

    for change in changes {
        set_path(&mut document, change.key.trim(), &change.value)?;
    }
    for key in removed {
        remove_path(&mut document, key.trim());
    }
    Ok(document.to_string().into_bytes())
}

fn walk<'a>(document: &'a mut DocumentMut, path: &[&str]) -> Option<(&'a mut Table, String)> {
    let (last, parents) = path.split_last()?;
    let mut table = document.as_table_mut();
    for step in parents {
        table = table.get_mut(step)?.as_table_mut()?;
    }
    Some((table, (*last).to_string()))
}

fn set_path(document: &mut DocumentMut, key: &str, text: &str) -> Result<()> {
    let path = key.split('.').collect::<Vec<_>>();
    let (table, leaf) = walk(document, &path)
        .ok_or_else(|| Error::other(format!("There is no {key} in this file.")))?;

    let existing = table
        .get(&leaf)
        .and_then(Item::as_value)
        .ok_or_else(|| Error::other(format!("There is no {key} in this file.")))?;

    let replacement = match existing {
        Value::String(_) => Value::from(text),
        Value::Boolean(_) => Value::from(
            text.trim()
                .parse::<bool>()
                .map_err(|_| Error::other(format!("{key} takes true or false.")))?,
        ),
        Value::Integer(_) => Value::from(
            text.trim()
                .parse::<i64>()
                .map_err(|_| Error::other(format!("{key} takes a whole number.")))?,
        ),
        Value::Float(_) => Value::from(
            text.trim()
                .parse::<f64>()
                .map_err(|_| Error::other(format!("{key} takes a number.")))?,
        ),
        _ => text
            .trim()
            .parse::<Value>()
            .map_err(|error| Error::other(format!("{key} does not parse: {error}")))?,
    };

    let decor = existing.decor().clone();
    table.insert(
        &leaf,
        Item::Value(
            replacement.decorated(
                decor
                    .prefix()
                    .and_then(|prefix| prefix.as_str())
                    .unwrap_or(" "),
                decor
                    .suffix()
                    .and_then(|suffix| suffix.as_str())
                    .unwrap_or(""),
            ),
        ),
    );
    Ok(())
}

fn remove_path(document: &mut DocumentMut, key: &str) {
    let path = key.split('.').collect::<Vec<_>>();
    if let Some((table, leaf)) = walk(document, &path) {
        table.remove(&leaf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUMPKIN: &str = r#"
# The server this file belongs to
[networking.java]
address = "0.0.0.0:25565"
enabled = true

[server]
max_players = 20
motd = "A Pumpkin server"
"#;

    #[test]
    fn a_toml_file_is_flattened_into_dotted_keys() {
        let config = read_toml("pumpkin.toml", PUMPKIN).unwrap();
        let keys = config
            .entries
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            [
                "networking.java.address",
                "networking.java.enabled",
                "server.max_players",
                "server.motd"
            ]
        );
        assert_eq!(config.port, Some(25565));
        assert_eq!(config.motd.as_deref(), Some("A Pumpkin server"));
        assert_eq!(config.max_players, Some(20));
    }

    #[test]
    fn a_string_is_shown_without_its_quotes() {
        let config = read_toml("pumpkin.toml", PUMPKIN).unwrap();
        let address = config
            .entries
            .iter()
            .find(|entry| entry.key == "networking.java.address")
            .unwrap();
        assert_eq!(address.value, "0.0.0.0:25565");
    }

    #[test]
    fn editing_one_key_leaves_the_rest_of_the_file_alone() {
        let rendered = write_toml(
            PUMPKIN,
            &[Entry {
                key: "networking.java.address".to_string(),
                value: "0.0.0.0:25580".to_string(),
            }],
            &[],
        )
        .unwrap();
        let text = String::from_utf8(rendered).unwrap();
        assert!(text.contains("# The server this file belongs to"));
        assert!(text.contains(r#"address = "0.0.0.0:25580""#));
        assert_eq!(
            text.replace("25580", "25565"),
            PUMPKIN,
            "nothing but the edited value should move"
        );
    }

    #[test]
    fn a_value_keeps_the_type_the_file_gave_it() {
        assert!(write_toml(
            PUMPKIN,
            &[Entry {
                key: "server.max_players".to_string(),
                value: "many".to_string(),
            }],
            &[],
        )
        .is_err());

        let rendered = write_toml(
            PUMPKIN,
            &[Entry {
                key: "server.max_players".to_string(),
                value: "40".to_string(),
            }],
            &[],
        )
        .unwrap();
        assert!(String::from_utf8(rendered)
            .unwrap()
            .contains("max_players = 40"));
    }

    #[test]
    fn a_key_the_file_does_not_have_is_refused() {
        let failed = write_toml(
            PUMPKIN,
            &[Entry {
                key: "server.hardcore".to_string(),
                value: "true".to_string(),
            }],
            &[],
        );
        assert!(failed.is_err());
    }
}
