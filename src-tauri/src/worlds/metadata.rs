use std::{
    borrow::Cow,
    io::Read,
    path::{Path, PathBuf},
};

use base64::Engine;
use flate2::read::GzDecoder;
use serde::Serialize;

use crate::{
    error::{Error, Result},
    files::FileManager,
};

const MAX_COMPRESSED_LEVEL_BYTES: u64 = 8 * 1024 * 1024;
const MAX_DECOMPRESSED_LEVEL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_NBT_DEPTH: usize = 32;
const MAX_NBT_TAGS: usize = 100_000;
const MAX_NBT_COLLECTION_ITEMS: usize = 1_000_000;
const MAX_WORLD_ICON_BYTES: u64 = 512 * 1024;

const TAG_END: u8 = 0;
const TAG_BYTE: u8 = 1;
const TAG_SHORT: u8 = 2;
const TAG_INT: u8 = 3;
const TAG_LONG: u8 = 4;
const TAG_FLOAT: u8 = 5;
const TAG_DOUBLE: u8 = 6;
const TAG_BYTE_ARRAY: u8 = 7;
const TAG_STRING: u8 = 8;
const TAG_LIST: u8 = 9;
const TAG_COMPOUND: u8 = 10;
const TAG_INT_ARRAY: u8 = 11;
const TAG_LONG_ARRAY: u8 = 12;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorldStatus {
    Ok,
    Recovered,
    Damaged,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldSummary {
    pub folder_name: String,
    pub name: String,
    pub last_played_ms: Option<i64>,
    pub version_name: Option<String>,
    pub data_version: Option<i32>,
    pub game_mode: String,
    pub hardcore: bool,
    pub difficulty: Option<i8>,
    pub icon_data_url: Option<String>,
    pub status: WorldStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct WorldMetadata {
    pub name: Option<String>,
    pub last_played_ms: Option<i64>,
    pub version_name: Option<String>,
    pub data_version: Option<i32>,
    pub game_type: Option<i32>,
    pub hardcore: bool,
    pub difficulty: Option<i8>,
    pub enabled_packs: Vec<String>,
    pub disabled_packs: Vec<String>,
}

struct NbtReader<'a> {
    bytes: &'a [u8],
    position: usize,
    tags: usize,
}

impl<'a> NbtReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            position: 0,
            tags: 0,
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| Error::other("truncated NBT data"))?;
        let bytes = &self.bytes[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn i8(&mut self) -> Result<i8> {
        Ok(self.byte()? as i8)
    }

    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn string(&mut self) -> Result<String> {
        let length = u16::from_be_bytes(self.take(2)?.try_into().unwrap()) as usize;
        let bytes = self.take(length)?;
        let decoded: Cow<'_, str> = cesu8::from_java_cesu8(bytes)
            .map_err(|_| Error::other("invalid Java string in NBT data"))?;
        Ok(decoded.into_owned())
    }

    fn collection_length(&mut self) -> Result<usize> {
        let length = self.i32()?;
        if length < 0 {
            return Err(Error::other("negative NBT collection length"));
        }
        let length = length as usize;
        if length > MAX_NBT_COLLECTION_ITEMS {
            return Err(Error::other("NBT collection exceeds the safety limit"));
        }
        Ok(length)
    }

    fn count_tag(&mut self) -> Result<()> {
        self.tags += 1;
        if self.tags > MAX_NBT_TAGS {
            return Err(Error::other("NBT tag count exceeds the safety limit"));
        }
        Ok(())
    }

    fn skip_payload(&mut self, tag: u8, depth: usize) -> Result<()> {
        if depth > MAX_NBT_DEPTH {
            return Err(Error::other("NBT nesting exceeds the safety limit"));
        }
        match tag {
            TAG_END => Ok(()),
            TAG_BYTE => {
                self.take(1)?;
                Ok(())
            }
            TAG_SHORT => {
                self.take(2)?;
                Ok(())
            }
            TAG_INT | TAG_FLOAT => {
                self.take(4)?;
                Ok(())
            }
            TAG_LONG | TAG_DOUBLE => {
                self.take(8)?;
                Ok(())
            }
            TAG_BYTE_ARRAY => {
                let length = self.collection_length()?;
                self.take(length)?;
                Ok(())
            }
            TAG_STRING => {
                let length = u16::from_be_bytes(self.take(2)?.try_into().unwrap()) as usize;
                self.take(length)?;
                Ok(())
            }
            TAG_LIST => {
                let item_tag = self.byte()?;
                let length = self.collection_length()?;
                for _ in 0..length {
                    self.count_tag()?;
                    self.skip_payload(item_tag, depth + 1)?;
                }
                Ok(())
            }
            TAG_COMPOUND => self.skip_compound(depth + 1),
            TAG_INT_ARRAY => {
                let length = self.collection_length()?;
                self.take(
                    length
                        .checked_mul(4)
                        .ok_or_else(|| Error::other("NBT array length overflow"))?,
                )?;
                Ok(())
            }
            TAG_LONG_ARRAY => {
                let length = self.collection_length()?;
                self.take(
                    length
                        .checked_mul(8)
                        .ok_or_else(|| Error::other("NBT array length overflow"))?,
                )?;
                Ok(())
            }
            _ => Err(Error::other(format!("unknown NBT tag {tag}"))),
        }
    }

    fn skip_compound(&mut self, depth: usize) -> Result<()> {
        loop {
            let tag = self.byte()?;
            if tag == TAG_END {
                return Ok(());
            }
            self.count_tag()?;
            self.string()?;
            self.skip_payload(tag, depth)?;
        }
    }

    fn read_version(&mut self, depth: usize, metadata: &mut WorldMetadata) -> Result<()> {
        if depth > MAX_NBT_DEPTH {
            return Err(Error::other("NBT nesting exceeds the safety limit"));
        }
        loop {
            let tag = self.byte()?;
            if tag == TAG_END {
                return Ok(());
            }
            self.count_tag()?;
            let name = self.string()?;
            match (name.as_str(), tag) {
                ("Name", TAG_STRING) => metadata.version_name = Some(self.string()?),
                ("Id", TAG_INT) if metadata.data_version.is_none() => {
                    metadata.data_version = Some(self.i32()?)
                }
                _ => self.skip_payload(tag, depth)?,
            }
        }
    }

    fn read_string_list(&mut self) -> Result<Vec<String>> {
        let element = self.byte()?;
        let length = self.collection_length()?;
        if element != TAG_STRING {
            for _ in 0..length {
                self.skip_payload(element, MAX_NBT_DEPTH)?;
            }
            return Ok(Vec::new());
        }
        let mut names = Vec::with_capacity(length.min(256));
        for _ in 0..length {
            self.count_tag()?;
            names.push(self.string()?);
        }
        Ok(names)
    }

    fn read_data_packs(&mut self, depth: usize, metadata: &mut WorldMetadata) -> Result<()> {
        if depth > MAX_NBT_DEPTH {
            return Err(Error::other("NBT nesting exceeds the safety limit"));
        }
        loop {
            let tag = self.byte()?;
            if tag == TAG_END {
                return Ok(());
            }
            self.count_tag()?;
            let name = self.string()?;
            match (name.as_str(), tag) {
                ("Enabled", TAG_LIST) => metadata.enabled_packs = self.read_string_list()?,
                ("Disabled", TAG_LIST) => metadata.disabled_packs = self.read_string_list()?,
                _ => self.skip_payload(tag, depth)?,
            }
        }
    }

    fn read_data(&mut self, depth: usize, metadata: &mut WorldMetadata) -> Result<()> {
        if depth > MAX_NBT_DEPTH {
            return Err(Error::other("NBT nesting exceeds the safety limit"));
        }
        loop {
            let tag = self.byte()?;
            if tag == TAG_END {
                return Ok(());
            }
            self.count_tag()?;
            let name = self.string()?;
            match (name.as_str(), tag) {
                ("LevelName", TAG_STRING) => metadata.name = Some(self.string()?),
                ("LastPlayed", TAG_LONG) => metadata.last_played_ms = Some(self.i64()?),
                ("DataVersion", TAG_INT) => metadata.data_version = Some(self.i32()?),
                ("GameType", TAG_INT) => metadata.game_type = Some(self.i32()?),
                ("hardcore", TAG_BYTE) => metadata.hardcore = self.i8()? != 0,
                ("Difficulty", TAG_BYTE) => metadata.difficulty = Some(self.i8()?),
                ("Version", TAG_COMPOUND) => self.read_version(depth + 1, metadata)?,
                ("DataPacks", TAG_COMPOUND) => self.read_data_packs(depth + 1, metadata)?,
                _ => self.skip_payload(tag, depth)?,
            }
        }
    }

    fn read_level(mut self) -> Result<WorldMetadata> {
        if self.byte()? != TAG_COMPOUND {
            return Err(Error::other("NBT root is not a compound"));
        }
        self.string()?;
        let mut metadata = WorldMetadata::default();
        let mut found_data = false;
        loop {
            let tag = self.byte()?;
            if tag == TAG_END {
                break;
            }
            self.count_tag()?;
            let name = self.string()?;
            if name == "Data" && tag == TAG_COMPOUND {
                self.read_data(1, &mut metadata)?;
                found_data = true;
            } else {
                self.skip_payload(tag, 1)?;
            }
        }
        if !found_data {
            return Err(Error::other("level metadata has no Data compound"));
        }
        Ok(metadata)
    }
}

pub(super) fn read_compressed_level(reader: impl Read) -> Result<WorldMetadata> {
    let mut decoder = GzDecoder::new(reader).take(MAX_DECOMPRESSED_LEVEL_BYTES + 1);
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_DECOMPRESSED_LEVEL_BYTES {
        return Err(Error::other(
            "decompressed level metadata exceeds the safety limit",
        ));
    }
    NbtReader::new(&bytes).read_level()
}

fn read_level_metadata(files: &FileManager, path: &Path) -> Result<WorldMetadata> {
    let metadata = files.metadata(path)?;
    if !metadata.is_file() {
        return Err(Error::other("level metadata is not a regular file"));
    }
    if metadata.len() > MAX_COMPRESSED_LEVEL_BYTES {
        return Err(Error::other(
            "compressed level metadata exceeds the safety limit",
        ));
    }

    read_compressed_level(files.open(path)?)
}

pub(super) fn game_mode(value: Option<i32>) -> String {
    match value {
        Some(0) => "survival",
        Some(1) => "creative",
        Some(2) => "adventure",
        Some(3) => "spectator",
        _ => "unknown",
    }
    .to_string()
}

fn world_icon(files: &FileManager, world_dir: &Path) -> Option<String> {
    let path = world_dir.join("icon.png");
    let metadata = files.metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_WORLD_ICON_BYTES {
        return None;
    }
    let bytes = files.read(&path).ok()?;
    let image = image::load_from_memory(&bytes).ok()?;
    if image.width() != 64 || image.height() != 64 {
        return None;
    }
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

pub fn pack_state(files: &FileManager, world_dir: &Path) -> (Vec<String>, Vec<String>) {
    let metadata = read_level_metadata(files, &world_dir.join("level.dat"))
        .or_else(|_| read_level_metadata(files, &world_dir.join("level.dat_old")));
    match metadata {
        Ok(metadata) => (metadata.enabled_packs, metadata.disabled_packs),
        Err(_) => (Vec::new(), Vec::new()),
    }
}

pub(super) fn world_from_dir(files: &FileManager, world_dir: PathBuf) -> Option<WorldSummary> {
    let metadata = files.symlink_metadata(&world_dir).ok()?;
    if !metadata.is_dir() || metadata.is_symlink() {
        return None;
    }
    let folder_name = world_dir.file_name()?.to_str()?.to_string();
    let primary = world_dir.join("level.dat");
    let backup = world_dir.join("level.dat_old");
    let primary_exists = files.is_file(&primary).unwrap_or(false);
    let backup_exists = files.is_file(&backup).unwrap_or(false);
    if !primary_exists && !backup_exists {
        return None;
    }

    let (metadata, status, error) = match read_level_metadata(files, &primary) {
        Ok(metadata) => (Some(metadata), WorldStatus::Ok, None),
        Err(primary_error) => match read_level_metadata(files, &backup) {
            Ok(metadata) => (
                Some(metadata),
                WorldStatus::Recovered,
                Some(format!("level.dat could not be read: {primary_error}")),
            ),
            Err(backup_error) => (
                None,
                WorldStatus::Damaged,
                Some(format!(
                    "level.dat could not be read: {primary_error}; level.dat_old could not be read: {backup_error}"
                )),
            ),
        },
    };
    let metadata = metadata.unwrap_or_default();
    Some(WorldSummary {
        name: metadata
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| folder_name.clone()),
        folder_name,
        last_played_ms: metadata.last_played_ms,
        version_name: metadata.version_name,
        data_version: metadata.data_version,
        game_mode: game_mode(metadata.game_type),
        hardcore: metadata.hardcore,
        difficulty: metadata.difficulty,
        icon_data_url: world_icon(files, &world_dir),
        status,
        error,
    })
}

pub fn list(files: &FileManager, instance_id: &str) -> Result<Vec<WorldSummary>> {
    let saves = files
        .paths()
        .instance_saves_dir_checked(instance_id)
        .ok_or_else(|| Error::other("invalid instance id"))?;
    let entries = match files.read_dir(saves) {
        Ok(entries) => entries,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new())
        }
        Err(error) => return Err(error),
    };
    let mut worlds = entries
        .into_iter()
        .filter_map(|entry| world_from_dir(files, entry))
        .collect::<Vec<_>>();
    worlds.sort_by(|a, b| {
        b.last_played_ms
            .unwrap_or_default()
            .cmp(&a.last_played_ms.unwrap_or_default())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(worlds)
}

#[cfg(test)]
mod tests {
    use std::{io::Write, path::Path};

    use flate2::{write::GzEncoder, Compression};

    use super::{list, NbtReader, WorldStatus};
    use crate::{files::FileManager, paths::Paths};

    fn files() -> FileManager {
        let root = std::env::temp_dir().join(format!("basalt-worlds-{}", uuid::Uuid::new_v4()));
        FileManager::new(Paths::plain(root)).unwrap()
    }

    fn named_tag(tag: u8, name: &str, payload: &[u8], out: &mut Vec<u8>) {
        out.push(tag);
        out.extend_from_slice(&(name.len() as u16).to_be_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(payload);
    }

    fn string_payload(value: &str) -> Vec<u8> {
        let mut payload = Vec::with_capacity(value.len() + 2);
        payload.extend_from_slice(&(value.len() as u16).to_be_bytes());
        payload.extend_from_slice(value.as_bytes());
        payload
    }

    fn level_bytes(name: &str, last_played: i64) -> Vec<u8> {
        let mut data = Vec::new();
        named_tag(8, "LevelName", &string_payload(name), &mut data);
        named_tag(4, "LastPlayed", &last_played.to_be_bytes(), &mut data);
        named_tag(3, "DataVersion", &4440_i32.to_be_bytes(), &mut data);
        named_tag(3, "GameType", &1_i32.to_be_bytes(), &mut data);
        named_tag(1, "hardcore", &[1], &mut data);
        let mut version = Vec::new();
        named_tag(8, "Name", &string_payload("26.1.2"), &mut version);
        version.push(0);
        named_tag(10, "Version", &version, &mut data);
        data.push(0);

        let mut root = vec![10, 0, 0];
        named_tag(10, "Data", &data, &mut root);
        root.push(0);
        root
    }

    fn legacy_level_bytes(name: &str, last_played: i64) -> Vec<u8> {
        let mut data = Vec::new();
        named_tag(8, "LevelName", &string_payload(name), &mut data);
        named_tag(4, "LastPlayed", &last_played.to_be_bytes(), &mut data);
        named_tag(3, "GameType", &0_i32.to_be_bytes(), &mut data);
        named_tag(1, "hardcore", &[0], &mut data);
        data.push(0);

        let mut root = vec![10, 0, 0];
        named_tag(10, "Data", &data, &mut root);
        root.push(0);
        root
    }

    fn write_level(files: &FileManager, path: &Path, bytes: &[u8]) {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(bytes).unwrap();
        files
            .write_atomic(path, &encoder.finish().unwrap())
            .unwrap();
    }

    #[test]
    fn bounded_reader_extracts_world_metadata() {
        let metadata = NbtReader::new(&level_bytes("A World", 123))
            .read_level()
            .unwrap();
        assert_eq!(metadata.name.as_deref(), Some("A World"));
        assert_eq!(metadata.last_played_ms, Some(123));
        assert_eq!(metadata.version_name.as_deref(), Some("26.1.2"));
        assert_eq!(metadata.data_version, Some(4440));
        assert_eq!(metadata.game_type, Some(1));
        assert!(metadata.hardcore);
    }

    #[test]
    fn pre_data_version_worlds_remain_readable() {
        let metadata = NbtReader::new(&legacy_level_bytes("Legacy 1.8", 456))
            .read_level()
            .unwrap();
        assert_eq!(metadata.name.as_deref(), Some("Legacy 1.8"));
        assert_eq!(metadata.last_played_ms, Some(456));
        assert_eq!(metadata.game_type, Some(0));
        assert!(!metadata.hardcore);
        assert_eq!(metadata.data_version, None);
        assert_eq!(metadata.version_name, None);
    }

    #[test]
    fn malformed_lengths_are_rejected_without_allocating() {
        let mut bytes = vec![10, 0, 0, 10, 0, 4];
        bytes.extend_from_slice(b"Data");
        named_tag(9, "bad", &[1, 0xff, 0xff, 0xff, 0xff], &mut bytes);
        bytes.extend_from_slice(&[0, 0]);
        assert!(NbtReader::new(&bytes).read_level().is_err());
    }

    #[test]
    fn listing_uses_backup_and_isolates_damaged_worlds() {
        let files = files();
        let saves = files
            .paths()
            .instance_saves_dir_checked("instance")
            .unwrap();
        let good = saves.join("Good");
        let broken = saves.join("Broken");
        files.ensure_dir(&good).unwrap();
        files.ensure_dir(&broken).unwrap();
        files
            .write_atomic(good.join("level.dat"), b"broken")
            .unwrap();
        write_level(
            &files,
            &good.join("level.dat_old"),
            &level_bytes("Recovered", 20),
        );
        files
            .write_atomic(broken.join("level.dat"), b"broken")
            .unwrap();

        let worlds = list(&files, "instance").unwrap();
        assert_eq!(worlds.len(), 2);
        assert_eq!(worlds[0].name, "Recovered");
        assert_eq!(worlds[0].status, WorldStatus::Recovered);
        assert_eq!(worlds[1].status, WorldStatus::Damaged);
    }
}
