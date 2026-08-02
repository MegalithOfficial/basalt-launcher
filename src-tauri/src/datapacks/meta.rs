use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{config::Instance, files::FileManager, paths::Paths};

const MAX_MCMETA_BYTES: u64 = 512 * 1024;

#[derive(Deserialize)]
struct VersionFile {
    #[serde(default)]
    pack_version: Option<serde_json::Value>,
}

fn data_format(value: &serde_json::Value) -> Option<u32> {
    match value {
        serde_json::Value::Number(flat) => flat.as_u64().map(|value| value as u32),
        serde_json::Value::Object(fields) => fields
            .get("data")
            .or_else(|| fields.get("data_major"))
            .and_then(serde_json::Value::as_u64)
            .map(|value| value as u32),
        _ => None,
    }
}

fn format_in_jar(files: &FileManager, jar: &Path) -> Option<u32> {
    let handle = files.open(jar).ok()?;
    let mut archive = zip::ZipArchive::new(handle).ok()?;
    let entry = archive.by_name("version.json").ok()?;
    if entry.size() > MAX_MCMETA_BYTES {
        return None;
    }
    let mut body = String::new();
    entry
        .take(MAX_MCMETA_BYTES)
        .read_to_string(&mut body)
        .ok()?;
    let parsed: VersionFile = serde_json::from_str(&body).ok()?;
    data_format(&parsed.pack_version?)
}

/// The datapack format a version expects, taken from the release itself rather than a table
/// that would need editing every time Mojang ships one.
pub fn expected_format(files: &FileManager, paths: &Paths, instance: &Instance) -> Option<u32> {
    instance
        .launch_version_id
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(instance.version_id.as_str()))
        .find_map(|id| format_in_jar(files, &paths.version_jar(id)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum Compatibility {
    Fits,
    Unknown,
    Mismatch { needs: u32, has: u32 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PackMeta {
    pub description: Option<String>,
    pub min_format: Option<u32>,
    pub max_format: Option<u32>,
}

impl PackMeta {
    pub fn against(&self, expected: Option<u32>) -> Compatibility {
        let (Some(min), Some(max)) = (self.min_format, self.max_format) else {
            return Compatibility::Unknown;
        };
        let Some(wanted) = expected else {
            return Compatibility::Unknown;
        };
        if wanted >= min && wanted <= max {
            Compatibility::Fits
        } else {
            Compatibility::Mismatch {
                needs: wanted,
                has: if wanted < min { min } else { max },
            }
        }
    }
}

#[derive(Deserialize)]
struct RawFile {
    pack: RawPack,
}

#[derive(Deserialize)]
struct RawPack {
    #[serde(default)]
    pack_format: Option<u32>,
    #[serde(default)]
    supported_formats: Option<serde_json::Value>,
    #[serde(default)]
    description: Option<serde_json::Value>,
}

fn flatten_description(value: &serde_json::Value, into: &mut String) {
    match value {
        serde_json::Value::String(text) => into.push_str(text),
        serde_json::Value::Array(parts) => {
            for part in parts {
                flatten_description(part, into);
            }
        }
        serde_json::Value::Object(fields) => {
            if let Some(text) = fields.get("text") {
                flatten_description(text, into);
            }
            if let Some(extra) = fields.get("extra") {
                flatten_description(extra, into);
            }
        }
        _ => {}
    }
}

fn range_from(value: &serde_json::Value) -> Option<(u32, u32)> {
    match value {
        serde_json::Value::Number(single) => {
            let only = single.as_u64()? as u32;
            Some((only, only))
        }
        serde_json::Value::Array(pair) => {
            let min = pair.first()?.as_u64()? as u32;
            let max = pair.get(1)?.as_u64().unwrap_or(min as u64) as u32;
            Some((min, max))
        }
        serde_json::Value::Object(fields) => {
            let min = fields.get("min_inclusive")?.as_u64()? as u32;
            let max = fields.get("max_inclusive")?.as_u64()? as u32;
            Some((min, max))
        }
        _ => None,
    }
}

pub fn parse(body: &str) -> PackMeta {
    let Ok(raw) = serde_json::from_str::<RawFile>(body) else {
        return PackMeta::default();
    };

    let declared = raw.pack.pack_format;
    let range = raw.pack.supported_formats.as_ref().and_then(range_from);
    let (min_format, max_format) = match (range, declared) {
        (Some((min, max)), _) => (Some(min), Some(max)),
        (None, Some(only)) => (Some(only), Some(only)),
        (None, None) => (None, None),
    };

    let mut description = String::new();
    if let Some(value) = &raw.pack.description {
        flatten_description(value, &mut description);
    }
    let description = description.trim().to_string();

    PackMeta {
        description: (!description.is_empty()).then_some(description),
        min_format,
        max_format,
    }
}

fn read_from_zip(files: &FileManager, path: &Path) -> Option<PackMeta> {
    let handle = files.open(path).ok()?;
    let mut archive = zip::ZipArchive::new(handle).ok()?;
    let entry = archive.by_name("pack.mcmeta").ok()?;
    if entry.size() > MAX_MCMETA_BYTES {
        return None;
    }
    let mut body = String::new();
    entry
        .take(MAX_MCMETA_BYTES)
        .read_to_string(&mut body)
        .ok()?;
    Some(parse(&body))
}

fn read_from_directory(files: &FileManager, path: &Path) -> Option<PackMeta> {
    let file = path.join("pack.mcmeta");
    let metadata = files.metadata(&file).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_MCMETA_BYTES {
        return None;
    }
    let bytes = files.read(&file).ok()?;
    Some(parse(&String::from_utf8_lossy(&bytes)))
}

pub fn read(files: &FileManager, path: &Path, directory: bool) -> PackMeta {
    let found = if directory {
        read_from_directory(files, path)
    } else {
        read_from_zip(files, path)
    };
    found.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_declared_range_beats_the_single_format_beside_it() {
        let meta = parse(
            r#"{"pack":{"pack_format":48,"supported_formats":{"min_inclusive":48,"max_inclusive":57}}}"#,
        );
        assert_eq!(meta.min_format, Some(48));
        assert_eq!(meta.max_format, Some(57));
    }

    #[test]
    fn a_range_written_as_a_pair_is_read_the_same_way() {
        let meta = parse(r#"{"pack":{"supported_formats":[71,71]}}"#);
        assert_eq!(meta.min_format, Some(71));
        assert_eq!(meta.max_format, Some(71));
    }

    #[test]
    fn a_lone_format_covers_only_itself() {
        let meta = parse(r#"{"pack":{"pack_format":18}}"#);
        assert_eq!(meta.min_format, Some(18));
        assert_eq!(meta.max_format, Some(18));
    }

    #[test]
    fn a_description_written_as_components_reads_as_one_line() {
        let meta = parse(
            r#"{"pack":{"pack_format":18,"description":[{"text":"Invisible Item Frames","color":"green"},{"text":" by "},{"text":"The8BitMonkey"}]}}"#,
        );
        assert_eq!(
            meta.description.as_deref(),
            Some("Invisible Item Frames by The8BitMonkey")
        );
    }

    #[test]
    fn a_pack_that_says_nothing_is_not_called_broken() {
        let meta = parse("not json at all");
        assert_eq!(meta, PackMeta::default());
        assert_eq!(meta.against(Some(48)), Compatibility::Unknown);
    }

    #[test]
    fn a_release_whose_jar_is_missing_is_never_called_incompatible() {
        let meta = parse(r#"{"pack":{"pack_format":48}}"#);
        assert_eq!(meta.against(None), Compatibility::Unknown);
    }

    #[test]
    fn a_pack_built_for_a_later_release_is_flagged() {
        let meta = parse(r#"{"pack":{"supported_formats":[71,71]}}"#);
        assert_eq!(
            meta.against(Some(48)),
            Compatibility::Mismatch { needs: 48, has: 71 }
        );
        assert_eq!(meta.against(Some(71)), Compatibility::Fits);
    }

    #[test]
    fn both_shapes_of_pack_version_yield_the_data_format() {
        let old: serde_json::Value = serde_json::from_str(r#"{"resource":34,"data":48}"#).unwrap();
        let new: serde_json::Value = serde_json::from_str(
            r#"{"resource_major":75,"resource_minor":0,"data_major":94,"data_minor":1}"#,
        )
        .unwrap();
        let ancient: serde_json::Value = serde_json::from_str("4").unwrap();

        assert_eq!(data_format(&old), Some(48));
        assert_eq!(data_format(&new), Some(94));
        assert_eq!(data_format(&ancient), Some(4));
    }
}
