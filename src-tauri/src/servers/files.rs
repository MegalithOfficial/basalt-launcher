use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    error::{Error, Result},
    files::FileManager,
};

use super::{properties, TextProblem};

pub const MAX_TEXT_BYTES: u64 = 2 * 1024 * 1024;
const SNIFF_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Properties,
    Json,
    Yaml,
    Toml,
    Text,
    Jar,
    Archive,
    Image,
}

impl FileKind {
    pub fn of(name: &str) -> Self {
        let extension = Path::new(name)
            .extension()
            .map(|value| value.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        match extension.as_str() {
            "properties" => FileKind::Properties,
            "json" | "mcmeta" => FileKind::Json,
            "yml" | "yaml" => FileKind::Yaml,
            "toml" => FileKind::Toml,
            "jar" => FileKind::Jar,
            "zip" | "gz" | "tgz" | "tar" | "rar" | "7z" | "mrpack" => FileKind::Archive,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" => FileKind::Image,
            _ => FileKind::Text,
        }
    }

    pub fn editable(self) -> bool {
        matches!(
            self,
            FileKind::Properties
                | FileKind::Json
                | FileKind::Yaml
                | FileKind::Toml
                | FileKind::Text
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerEntry {
    pub name: String,
    pub path: String,
    pub directory: bool,
    pub size_bytes: u64,
    pub modified_ms: i64,
    pub kind: FileKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerText {
    pub path: String,
    pub kind: FileKind,
    pub text: String,
}

pub fn resolve(dir: &Path, relative: &str) -> Result<PathBuf> {
    let mut path = dir.to_path_buf();
    for part in relative.split(['/', '\\']) {
        let part = part.trim();
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains(':') {
            return Err(outside(relative));
        }
        path.push(part);
    }
    if !path.starts_with(dir) {
        return Err(outside(relative));
    }
    Ok(path)
}

fn outside(relative: &str) -> Error {
    Error::other(format!("{relative} is outside this server folder"))
}

fn child(dir: &Path, relative: &str) -> Result<PathBuf> {
    let path = resolve(dir, relative)?;
    if path == dir {
        return Err(Error::other("The server folder itself cannot be changed."));
    }
    Ok(path)
}

fn joined(relative: &str, name: &str) -> String {
    let base = relative.trim_matches('/');
    if base.is_empty() {
        name.to_string()
    } else {
        format!("{base}/{name}")
    }
}

fn clean_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::other("Give it a name first."));
    }
    if name == "." || name == ".." || name.contains(['/', '\\']) {
        return Err(Error::other("A name cannot contain slashes."));
    }
    Ok(name)
}

pub fn entries(files: &FileManager, dir: &Path, relative: &str) -> Result<Vec<ServerEntry>> {
    let target = resolve(dir, relative)?;
    let mut entries = Vec::new();
    for path in files.read_dir(&target)? {
        let Some(name) = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
        else {
            continue;
        };
        let Ok(meta) = files.symlink_metadata(&path) else {
            continue;
        };
        entries.push(ServerEntry {
            kind: FileKind::of(&name),
            path: joined(relative, &name),
            directory: meta.is_dir(),
            size_bytes: meta.len(),
            modified_ms: meta
                .modified()
                .ok()
                .and_then(|time| time.into_std().duration_since(std::time::UNIX_EPOCH).ok())
                .map(|since| since.as_millis() as i64)
                .unwrap_or(0),
            name,
        });
    }
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn read_text(files: &FileManager, dir: &Path, relative: &str) -> Result<ServerText> {
    let path = child(dir, relative)?;
    let kind = FileKind::of(relative);
    if !kind.editable() {
        return Err(Error::other(
            "Basalt cannot open this kind of file as text.",
        ));
    }
    let meta = files.metadata(&path)?;
    if meta.is_dir() {
        return Err(Error::other("That is a folder, not a file."));
    }
    if meta.len() > MAX_TEXT_BYTES {
        return Err(Error::other(format!(
            "This file is larger than {} MB, so it cannot be edited here.",
            MAX_TEXT_BYTES / (1024 * 1024)
        )));
    }
    let bytes = files.read(&path)?;
    if bytes.iter().take(SNIFF_BYTES).any(|byte| *byte == 0) {
        return Err(Error::other("This file is binary, not text."));
    }
    let text = if kind == FileKind::Properties {
        properties::decode_latin1(&bytes)
    } else {
        String::from_utf8(bytes).map_err(|_| {
            Error::other("This file is not valid UTF-8, so Basalt will not touch it.")
        })?
    };
    Ok(ServerText {
        path: relative.to_string(),
        kind,
        text,
    })
}

pub fn write_text(
    files: &FileManager,
    dir: &Path,
    relative: &str,
    text: &str,
) -> Result<Option<TextProblem>> {
    let path = child(dir, relative)?;
    let kind = FileKind::of(relative);
    if !kind.editable() {
        return Err(Error::other(
            "Basalt cannot save this kind of file as text.",
        ));
    }
    if let Some(problem) = validate(kind, text) {
        return Ok(Some(problem));
    }
    let bytes = if kind == FileKind::Properties {
        properties::encode_latin1(text)
    } else {
        text.as_bytes().to_vec()
    };
    files.write_atomic(&path, &bytes)?;
    Ok(None)
}

pub fn create_dir(files: &FileManager, dir: &Path, relative: &str, name: &str) -> Result<String> {
    let name = clean_name(name)?;
    let path = joined(relative, name);
    let target = child(dir, &path)?;
    if files.exists(&target)? {
        return Err(Error::other(format!("{name} already exists here.")));
    }
    files.ensure_dir(&target)?;
    Ok(path)
}

pub fn rename(files: &FileManager, dir: &Path, relative: &str, name: &str) -> Result<String> {
    let name = clean_name(name)?;
    let source = child(dir, relative)?;
    let parent = relative
        .trim_matches('/')
        .rsplit_once('/')
        .map(|(head, _)| head)
        .unwrap_or_default();
    let path = joined(parent, name);
    let destination = child(dir, &path)?;
    if source == destination {
        return Ok(path);
    }
    if files.exists(&destination)? {
        return Err(Error::other(format!("{name} already exists here.")));
    }
    files.rename(&source, &destination)?;
    Ok(path)
}

pub fn delete(files: &FileManager, dir: &Path, relative: &str) -> Result<()> {
    let path = child(dir, relative)?;
    if files.metadata(&path)?.is_dir() {
        files.remove_managed_dir_all_if_exists(&path)?;
    } else {
        files.remove_file_if_exists(&path)?;
    }
    Ok(())
}

pub fn upload(
    files: &FileManager,
    dir: &Path,
    relative: &str,
    sources: &[String],
) -> Result<usize> {
    let target = resolve(dir, relative)?;
    files.ensure_dir(&target)?;
    let mut copied = 0;
    for source in sources {
        let source = Path::new(source);
        let Some(name) = source.file_name() else {
            continue;
        };
        if !files.is_external_file(source) {
            continue;
        }
        files.copy_external_into_sync(source, target.join(name))?;
        copied += 1;
    }
    Ok(copied)
}

pub fn validate(kind: FileKind, text: &str) -> Option<TextProblem> {
    match kind {
        FileKind::Properties => properties::validate(text),
        FileKind::Json => serde_json::from_str::<serde_json::Value>(text)
            .err()
            .map(|error| TextProblem {
                line: error.line(),
                column: error.column(),
                message: error.to_string(),
            }),
        FileKind::Toml => toml::from_str::<toml::Value>(text).err().map(|error| {
            let (line, column) = error
                .span()
                .map(|span| position_of(text, span.start))
                .unwrap_or((1, 1));
            TextProblem {
                line,
                column,
                message: error.message().to_string(),
            }
        }),
        FileKind::Yaml => yaml_rust2::YamlLoader::load_from_str(text)
            .err()
            .map(|error| TextProblem {
                line: error.marker().line(),
                column: error.marker().col() + 1,
                message: error.info().to_string(),
            }),
        _ => None,
    }
}

fn position_of(text: &str, offset: usize) -> (usize, usize) {
    let head = &text[..offset.min(text.len())];
    let line = head.matches('\n').count() + 1;
    let column = head
        .rsplit_once('\n')
        .map(|(_, tail)| tail.len())
        .unwrap_or(head.len())
        + 1;
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;

    fn sandbox(name: &str) -> (PathBuf, FileManager) {
        let root = std::env::temp_dir().join(format!("basalt-{name}-{}", uuid::Uuid::new_v4()));
        let files = FileManager::new(Paths::plain(root.clone())).unwrap();
        let dir = root.join("servers").join("s1");
        files.ensure_dir(&dir).unwrap();
        (dir, files)
    }

    #[test]
    fn relative_paths_cannot_climb_out_of_the_server() {
        let dir = Path::new("/data/servers/s1");
        assert_eq!(
            resolve(dir, "mods/fabric.jar").unwrap(),
            dir.join("mods/fabric.jar")
        );
        assert_eq!(resolve(dir, "/mods/").unwrap(), dir.join("mods"));
        assert_eq!(resolve(dir, "./a/./b").unwrap(), dir.join("a/b"));
        assert!(resolve(dir, "../other").is_err());
        assert!(resolve(dir, "mods/../../etc").is_err());
        assert!(resolve(dir, "mods\\..\\..\\etc").is_err());
        assert!(resolve(dir, "C:/windows").is_err());
    }

    #[test]
    fn the_server_folder_itself_is_off_limits() {
        let dir = Path::new("/data/servers/s1");
        assert!(child(dir, "").is_err());
        assert!(child(dir, "/").is_err());
        assert!(child(dir, ".").is_err());
        assert!(child(dir, "world").is_ok());
    }

    #[test]
    fn names_with_separators_are_refused() {
        assert!(clean_name("").is_err());
        assert!(clean_name("  ").is_err());
        assert!(clean_name("..").is_err());
        assert!(clean_name("a/b").is_err());
        assert_eq!(clean_name(" plugins ").unwrap(), "plugins");
    }

    #[test]
    fn listing_puts_folders_first_and_tags_every_kind() {
        let (dir, files) = sandbox("files-list");
        files.ensure_dir(dir.join("plugins")).unwrap();
        files.write_atomic(dir.join("server.jar"), b"jar").unwrap();
        files
            .write_atomic(dir.join("server.properties"), b"motd=hi\n")
            .unwrap();

        let entries = entries(&files, &dir, "").unwrap();

        assert_eq!(entries[0].name, "plugins");
        assert!(entries[0].directory);
        assert_eq!(entries[1].name, "server.jar");
        assert_eq!(entries[1].kind, FileKind::Jar);
        assert_eq!(entries[2].kind, FileKind::Properties);
        assert_eq!(entries[2].size_bytes, 8);
        assert_eq!(entries[2].path, "server.properties");
        std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).ok();
    }

    #[test]
    fn a_properties_file_survives_being_opened_and_saved() {
        let (dir, files) = sandbox("files-latin1");
        files
            .write_atomic(dir.join("server.properties"), b"motd=caf\xe9\n")
            .unwrap();

        let opened = read_text(&files, &dir, "server.properties").unwrap();
        assert_eq!(opened.text, "motd=café\n");
        assert_eq!(opened.kind, FileKind::Properties);

        assert!(write_text(&files, &dir, "server.properties", &opened.text)
            .unwrap()
            .is_none());
        assert_eq!(
            files.read(dir.join("server.properties")).unwrap(),
            b"motd=caf\xe9\n".to_vec()
        );
        std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).ok();
    }

    #[test]
    fn a_broken_file_is_reported_instead_of_being_written() {
        let (dir, files) = sandbox("files-invalid");
        files.write_atomic(dir.join("config.json"), b"{}").unwrap();

        let problem = write_text(&files, &dir, "config.json", "{\n  \"a\": ,\n}")
            .unwrap()
            .unwrap();

        assert_eq!(problem.line, 2);
        assert_eq!(files.read(dir.join("config.json")).unwrap(), b"{}".to_vec());
        std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).ok();
    }

    #[test]
    fn binary_and_oversized_files_stay_closed() {
        let (dir, files) = sandbox("files-binary");
        files
            .write_atomic(dir.join("world.dat"), b"nbt\0data")
            .unwrap();
        files.write_atomic(dir.join("server.jar"), b"pk").unwrap();

        assert!(read_text(&files, &dir, "world.dat").is_err());
        assert!(read_text(&files, &dir, "server.jar").is_err());
        std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).ok();
    }

    #[test]
    fn renaming_and_deleting_stay_inside_the_server() {
        let (dir, files) = sandbox("files-rename");
        files.ensure_dir(dir.join("plugins")).unwrap();
        files
            .write_atomic(dir.join("plugins/config.yml"), b"a: 1\n")
            .unwrap();

        let moved = rename(&files, &dir, "plugins/config.yml", "settings.yml").unwrap();
        assert_eq!(moved, "plugins/settings.yml");
        assert!(files.exists(dir.join("plugins/settings.yml")).unwrap());

        assert!(rename(&files, &dir, "plugins", "../escape").is_err());
        assert!(delete(&files, &dir, "..").is_err());

        delete(&files, &dir, "plugins").unwrap();
        assert!(!files.exists(dir.join("plugins")).unwrap());
        std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).ok();
    }

    #[test]
    fn new_folders_and_uploads_land_where_they_were_asked_to() {
        let (dir, files) = sandbox("files-upload");
        let outside = std::env::temp_dir().join(format!("basalt-drop-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("worldedit.jar"), b"plugin").unwrap();

        let created = create_dir(&files, &dir, "", "plugins").unwrap();
        assert_eq!(created, "plugins");
        assert!(create_dir(&files, &dir, "", "plugins").is_err());

        let copied = upload(
            &files,
            &dir,
            "plugins",
            &[
                outside.join("worldedit.jar").display().to_string(),
                outside.join("missing.jar").display().to_string(),
            ],
        )
        .unwrap();

        assert_eq!(copied, 1);
        assert_eq!(
            files.read(dir.join("plugins/worldedit.jar")).unwrap(),
            b"plugin".to_vec()
        );
        std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).ok();
        std::fs::remove_dir_all(outside).ok();
    }

    #[test]
    fn every_format_reports_where_it_broke() {
        assert!(validate(FileKind::Yaml, "a: 1\nb: 2\n").is_none());
        assert!(validate(FileKind::Toml, "a = 1\n").is_none());
        assert!(validate(FileKind::Json, "{\"a\": 1}").is_none());
        assert!(validate(FileKind::Text, "anything goes\n\0").is_none());

        let yaml = validate(FileKind::Yaml, "a: 1\n  b: 2\n").unwrap();
        assert_eq!(yaml.line, 2);

        let toml = validate(FileKind::Toml, "a = 1\nb =\n").unwrap();
        assert_eq!(toml.line, 2);

        let properties = validate(FileKind::Properties, "a=1\na=2\n").unwrap();
        assert_eq!(properties.line, 2);
    }
}
