use std::{
    collections::{BTreeMap, HashSet},
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
};

use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::{
    error::{Error, Result},
    files::FileManager,
    tasks::TaskHandle,
};

use super::metadata::{
    game_mode, read_compressed_level, world_from_dir, WorldMetadata, WorldStatus,
};

const MAX_LEVEL_FILE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SOURCE_ENTRIES: usize = 250_000;
const MAX_TOTAL_WORLD_BYTES: u64 = 1024 * 1024 * 1024 * 1024;
const MAX_SINGLE_FILE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MAX_COMPRESSION_RATIO: u64 = 1_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportSourceKind {
    Directory,
    Zip,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportCandidate {
    pub id: String,
    pub archive_root: String,
    pub name: String,
    pub last_played_ms: Option<i64>,
    pub version_name: Option<String>,
    pub data_version: Option<i32>,
    pub game_mode: String,
    pub hardcore: bool,
    pub status: WorldStatus,
    pub error: Option<String>,
    pub file_count: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportInspection {
    pub source_kind: ImportSourceKind,
    pub candidates: Vec<ImportCandidate>,
}

#[derive(Debug, Clone)]
struct CandidateRoot {
    summary: ImportCandidate,
    root: PathBuf,
}

#[derive(Debug, Clone)]
struct ArchiveEntry {
    index: usize,
    path: PathBuf,
    size: u64,
    compressed_size: u64,
    directory: bool,
}

struct ArchiveScan {
    candidates: Vec<CandidateRoot>,
    entries: Vec<ArchiveEntry>,
}

fn candidate_id(root: &Path) -> String {
    uuid::Uuid::new_v3(
        &uuid::Uuid::NAMESPACE_OID,
        root.to_string_lossy().as_bytes(),
    )
    .to_string()
}

fn candidate_summary(
    root: &Path,
    folder_name: String,
    metadata: Option<WorldMetadata>,
    status: WorldStatus,
    error: Option<String>,
    file_count: u64,
    total_bytes: u64,
) -> ImportCandidate {
    let metadata = metadata.unwrap_or_default();
    ImportCandidate {
        id: candidate_id(root),
        archive_root: if root.as_os_str().is_empty() {
            ".".to_string()
        } else {
            root.to_string_lossy().into_owned()
        },
        name: metadata
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(folder_name),
        last_played_ms: metadata.last_played_ms,
        version_name: metadata.version_name,
        data_version: metadata.data_version,
        game_mode: game_mode(metadata.game_type),
        hardcore: metadata.hardcore,
        status,
        error,
        file_count,
        total_bytes,
    }
}

fn sanitize_archive_path(name: &str) -> Result<PathBuf> {
    if name.is_empty()
        || name.contains('\\')
        || name.contains('\0')
        || name.starts_with('/')
        || name.starts_with('~')
    {
        return Err(Error::other(format!("unsafe archive path {name:?}")));
    }
    let mut path = PathBuf::new();
    for component in Path::new(name).components() {
        match component {
            Component::Normal(value) => path.push(value),
            Component::CurDir => {}
            _ => return Err(Error::other(format!("unsafe archive path {name:?}"))),
        }
    }
    if path.as_os_str().is_empty() {
        return Err(Error::other(format!("unsafe archive path {name:?}")));
    }
    Ok(path)
}

fn is_special_zip_entry(mode: Option<u32>, directory: bool) -> bool {
    let Some(mode) = mode else {
        return false;
    };
    let kind = mode & 0o170000;
    kind != 0 && kind != 0o100000 && !(directory && kind == 0o040000)
}

fn read_zip_metadata(
    archive: &mut zip::ZipArchive<std::fs::File>,
    index: usize,
) -> Result<WorldMetadata> {
    let entry = archive
        .by_index(index)
        .map_err(|error| Error::other(format!("reading level metadata from ZIP: {error}")))?;
    if entry.size() > MAX_LEVEL_FILE_BYTES {
        return Err(Error::other("level metadata exceeds the safety limit"));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    let mut limited = entry.take(MAX_LEVEL_FILE_BYTES + 1);
    limited.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_LEVEL_FILE_BYTES {
        return Err(Error::other("level metadata exceeds the safety limit"));
    }
    read_compressed_level(Cursor::new(bytes))
}

fn inspect_archive_handle(archive: &mut zip::ZipArchive<std::fs::File>) -> Result<ArchiveScan> {
    if archive.len() > MAX_SOURCE_ENTRIES {
        return Err(Error::other("ZIP contains too many entries"));
    }

    let mut entries = Vec::with_capacity(archive.len());
    let mut seen = HashSet::new();
    let mut metadata_files: BTreeMap<PathBuf, (Option<usize>, Option<usize>)> = BTreeMap::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| Error::other(format!("reading ZIP entry: {error}")))?;
        let path = sanitize_archive_path(entry.name())?;
        let key = path.to_string_lossy().to_lowercase();
        if !seen.insert(key) {
            return Err(Error::other(format!(
                "ZIP contains duplicate or case-conflicting path {}",
                path.display()
            )));
        }
        let directory = entry.is_dir();
        if is_special_zip_entry(entry.unix_mode(), directory) {
            return Err(Error::other(format!(
                "ZIP contains a special file at {}",
                path.display()
            )));
        }
        if entry.size() > MAX_SINGLE_FILE_BYTES {
            return Err(Error::other(format!(
                "ZIP entry {} exceeds the per-file safety limit",
                path.display()
            )));
        }
        if entry.size() > 64 * 1024 * 1024
            && entry.compressed_size() > 0
            && entry.size() / entry.compressed_size() > MAX_COMPRESSION_RATIO
        {
            return Err(Error::other(format!(
                "ZIP entry {} has a suspicious compression ratio",
                path.display()
            )));
        }
        if !directory {
            let file_name = path.file_name().and_then(|name| name.to_str());
            if matches!(file_name, Some("level.dat" | "level.dat_old")) {
                let root = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
                let pair = metadata_files.entry(root).or_default();
                if file_name == Some("level.dat") {
                    pair.0 = Some(index);
                } else {
                    pair.1 = Some(index);
                }
            }
        }
        entries.push(ArchiveEntry {
            index,
            path,
            size: entry.size(),
            compressed_size: entry.compressed_size(),
            directory,
        });
    }

    let mut candidates = Vec::new();
    for (root, (primary, backup)) in metadata_files {
        let (metadata, status, error) = match primary
            .map(|index| read_zip_metadata(archive, index))
            .transpose()
        {
            Ok(Some(metadata)) => (Some(metadata), WorldStatus::Ok, None),
            primary_result => {
                let primary_error = match primary_result {
                    Ok(None) => "level.dat is missing".to_string(),
                    Err(error) => error.to_string(),
                    Ok(Some(_)) => unreachable!(),
                };
                match backup
                    .map(|index| read_zip_metadata(archive, index))
                    .transpose()
                {
                    Ok(Some(metadata)) => (
                        Some(metadata),
                        WorldStatus::Recovered,
                        Some(format!("level.dat could not be read: {primary_error}")),
                    ),
                    backup_result => {
                        let backup_error = match backup_result {
                            Ok(None) => "level.dat_old is missing".to_string(),
                            Err(error) => error.to_string(),
                            Ok(Some(_)) => unreachable!(),
                        };
                        (
                            None,
                            WorldStatus::Damaged,
                            Some(format!(
                                "level.dat could not be read: {primary_error}; level.dat_old could not be read: {backup_error}"
                            )),
                        )
                    }
                }
            }
        };
        let matching = entries
            .iter()
            .filter(|entry| root.as_os_str().is_empty() || entry.path.starts_with(&root))
            .collect::<Vec<_>>();
        let file_count = matching.iter().filter(|entry| !entry.directory).count() as u64;
        let total_bytes = matching.iter().try_fold(0_u64, |total, entry| {
            total
                .checked_add(entry.size)
                .filter(|total| *total <= MAX_TOTAL_WORLD_BYTES)
                .ok_or_else(|| Error::other("world exceeds the expanded-size safety limit"))
        })?;
        let folder_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Imported World")
            .to_string();
        candidates.push(CandidateRoot {
            summary: candidate_summary(
                &root,
                folder_name,
                metadata,
                status,
                error,
                file_count,
                total_bytes,
            ),
            root,
        });
    }
    candidates.sort_by(|a, b| {
        a.summary
            .name
            .to_lowercase()
            .cmp(&b.summary.name.to_lowercase())
    });
    Ok(ArchiveScan {
        candidates,
        entries,
    })
}

fn inspect_zip(files: &FileManager, source: &Path) -> Result<ArchiveScan> {
    let metadata = files.external_symlink_metadata(source)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(Error::other("selected ZIP is not a regular file"));
    }
    let file = files.open_external(source)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| Error::other(format!("opening world ZIP: {error}")))?;
    inspect_archive_handle(&mut archive)
}

fn directory_files(files: &FileManager, root: &Path) -> Result<Vec<(PathBuf, u64)>> {
    let mut pending = vec![root.to_path_buf()];
    let mut output = Vec::new();
    while let Some(directory) = pending.pop() {
        for path in files.read_external_dir(&directory)? {
            let metadata = files.external_symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(Error::other(format!(
                    "world contains a symbolic link at {}",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                if metadata.len() > MAX_SINGLE_FILE_BYTES {
                    return Err(Error::other(format!(
                        "world file {} exceeds the per-file safety limit",
                        path.display()
                    )));
                }
                output.push((path, metadata.len()));
                if output.len() > MAX_SOURCE_ENTRIES {
                    return Err(Error::other("world contains too many files"));
                }
            } else {
                return Err(Error::other(format!(
                    "world contains a special file at {}",
                    path.display()
                )));
            }
        }
    }
    Ok(output)
}

fn read_external_metadata(files: &FileManager, path: &Path) -> Result<WorldMetadata> {
    let metadata = files.external_symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_LEVEL_FILE_BYTES
    {
        return Err(Error::other("level metadata is not a safe regular file"));
    }
    read_compressed_level(files.open_external(path)?)
}

fn inspect_directory(files: &FileManager, source: &Path) -> Result<CandidateRoot> {
    let metadata = files.external_symlink_metadata(source)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::other("selected world is not a regular directory"));
    }
    let primary = source.join("level.dat");
    let backup = source.join("level.dat_old");
    let (world_metadata, status, error) = match read_external_metadata(files, &primary) {
        Ok(metadata) => (Some(metadata), WorldStatus::Ok, None),
        Err(primary_error) => match read_external_metadata(files, &backup) {
            Ok(metadata) => (
                Some(metadata),
                WorldStatus::Recovered,
                Some(format!("level.dat could not be read: {primary_error}")),
            ),
            Err(backup_error) => {
                return Err(Error::other(format!(
                    "selected directory is not a readable Minecraft world: {primary_error}; {backup_error}"
                )));
            }
        },
    };
    let source_files = directory_files(files, source)?;
    let total_bytes = source_files.iter().try_fold(0_u64, |total, (_, size)| {
        total
            .checked_add(*size)
            .filter(|total| *total <= MAX_TOTAL_WORLD_BYTES)
            .ok_or_else(|| Error::other("world exceeds the expanded-size safety limit"))
    })?;
    let folder_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Imported World")
        .to_string();
    Ok(CandidateRoot {
        summary: candidate_summary(
            Path::new(""),
            folder_name,
            world_metadata,
            status,
            error,
            source_files.len() as u64,
            total_bytes,
        ),
        root: PathBuf::new(),
    })
}

fn is_zip(source: &Path) -> bool {
    source
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
}

pub fn inspect_source(files: &FileManager, source: &Path) -> Result<ImportInspection> {
    if is_zip(source) {
        let scan = inspect_zip(files, source)?;
        if scan.candidates.is_empty() {
            return Err(Error::other("no Minecraft worlds were found in the ZIP"));
        }
        Ok(ImportInspection {
            source_kind: ImportSourceKind::Zip,
            candidates: scan
                .candidates
                .into_iter()
                .map(|candidate| candidate.summary)
                .collect(),
        })
    } else {
        let candidate = inspect_directory(files, source)?;
        Ok(ImportInspection {
            source_kind: ImportSourceKind::Directory,
            candidates: vec![candidate.summary],
        })
    }
}

fn safe_folder_name(value: &str) -> String {
    let mut name = value
        .chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
        })
        .take(80)
        .collect::<String>();
    name = name.trim_matches([' ', '.']).trim().to_string();
    if name.is_empty() {
        "Imported World".to_string()
    } else {
        name
    }
}

fn unique_destination(files: &FileManager, saves: &Path, name: &str) -> Result<PathBuf> {
    let base = safe_folder_name(name);
    for copy in 1..=10_000 {
        let name = if copy == 1 {
            base.clone()
        } else {
            format!("{base} ({copy})")
        };
        let path = saves.join(name);
        if !files.exists(&path)? {
            return Ok(path);
        }
    }
    Err(Error::other("could not choose a unique world folder"))
}

struct CancelReader<R> {
    inner: R,
    token: CancellationToken,
}

impl<R: Read> Read for CancelReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.token.is_cancelled() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "cancelled",
            ));
        }
        self.inner.read(buffer)
    }
}

fn validate_staged(files: &FileManager, staging: &Path) -> Result<()> {
    let world = world_from_dir(files, staging.to_path_buf())
        .ok_or_else(|| Error::other("imported files do not contain a Minecraft world"))?;
    if world.status == WorldStatus::Damaged {
        return Err(Error::other(world.error.unwrap_or_else(|| {
            "imported world metadata is damaged".to_string()
        })));
    }
    Ok(())
}

fn finish_staged(files: &FileManager, saves: &Path, staging: &Path, name: &str) -> Result<()> {
    validate_staged(files, staging)?;
    let destination = unique_destination(files, saves, name)?;
    files.rename(staging, destination)
}

fn import_directory(
    files: &FileManager,
    source: &Path,
    candidate: &CandidateRoot,
    saves: &Path,
    task: &TaskHandle,
) -> Result<()> {
    let source_files = directory_files(files, source)?;
    task.set_total(source_files.len() as u64, candidate.summary.total_bytes);
    let staging = saves.join(format!(".basalt-import-{}", uuid::Uuid::new_v4()));
    files.ensure_dir(&staging)?;
    let result = (|| {
        let token = task.token();
        let mut copied = 0;
        for (index, (path, size)) in source_files.iter().enumerate() {
            if token.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let relative = path
                .strip_prefix(source)
                .map_err(|_| Error::other("world source changed during import"))?;
            let mut reader = CancelReader {
                inner: files.open_external(path)?,
                token: token.clone(),
            };
            match files.copy_reader_into_sync(&mut reader, staging.join(relative)) {
                Ok(actual) if actual == *size => copied += actual,
                Ok(_) => return Err(Error::other("world source changed during import")),
                Err(_) if token.is_cancelled() => return Err(Error::Cancelled),
                Err(error) => return Err(error),
            }
            task.progress(
                index as u64 + 1,
                source_files.len() as u64,
                copied,
                candidate.summary.total_bytes,
            );
        }
        task.stage("validating");
        finish_staged(files, saves, &staging, &candidate.summary.name)
    })();
    if result.is_err() {
        let _ = files.remove_managed_dir_all_if_exists(&staging);
    }
    result
}

fn import_archive(
    files: &FileManager,
    source: &Path,
    selected_ids: &HashSet<String>,
    saves: &Path,
    task: &TaskHandle,
) -> Result<usize> {
    let metadata = files.external_symlink_metadata(source)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(Error::other("selected ZIP is not a regular file"));
    }
    let mut archive = zip::ZipArchive::new(files.open_external(source)?)
        .map_err(|error| Error::other(format!("opening world ZIP: {error}")))?;
    let scan = inspect_archive_handle(&mut archive)?;
    let selected = scan
        .candidates
        .iter()
        .filter(|candidate| selected_ids.contains(&candidate.summary.id))
        .collect::<Vec<_>>();
    if selected.len() != selected_ids.len() {
        return Err(Error::other("world selection no longer matches the ZIP"));
    }
    if selected
        .iter()
        .any(|candidate| candidate.summary.status == WorldStatus::Damaged)
    {
        return Err(Error::other("damaged worlds cannot be imported"));
    }
    for (index, candidate) in selected.iter().enumerate() {
        for other in selected.iter().skip(index + 1) {
            if candidate.root.starts_with(&other.root) || other.root.starts_with(&candidate.root) {
                return Err(Error::other(
                    "ZIP contains nested world roots that cannot be imported together safely",
                ));
            }
        }
    }
    let total_files = selected
        .iter()
        .map(|candidate| candidate.summary.file_count)
        .sum();
    let total_bytes = selected
        .iter()
        .map(|candidate| candidate.summary.total_bytes)
        .sum();
    task.set_total(total_files, total_bytes);

    let token = task.token();
    let mut completed = 0;
    let mut copied = 0;
    for candidate in selected {
        let staging = saves.join(format!(".basalt-import-{}", uuid::Uuid::new_v4()));
        files.ensure_dir(&staging)?;
        let result = (|| {
            for entry in scan.entries.iter().filter(|entry| {
                candidate.root.as_os_str().is_empty() || entry.path.starts_with(&candidate.root)
            }) {
                if token.is_cancelled() {
                    return Err(Error::Cancelled);
                }
                let relative = if candidate.root.as_os_str().is_empty() {
                    entry.path.as_path()
                } else {
                    entry
                        .path
                        .strip_prefix(&candidate.root)
                        .map_err(|_| Error::other("invalid candidate root"))?
                };
                if relative.as_os_str().is_empty() {
                    continue;
                }
                let destination = staging.join(relative);
                if entry.directory {
                    files.ensure_dir(destination)?;
                    continue;
                }
                let zip_entry = archive
                    .by_index(entry.index)
                    .map_err(|error| Error::other(format!("reading ZIP entry: {error}")))?;
                if zip_entry.size() != entry.size
                    || zip_entry.compressed_size() != entry.compressed_size
                {
                    return Err(Error::other("ZIP changed during import"));
                }
                let mut reader = CancelReader {
                    inner: zip_entry,
                    token: token.clone(),
                };
                match files.copy_reader_into_sync(&mut reader, destination) {
                    Ok(actual) if actual == entry.size => copied += actual,
                    Ok(_) => return Err(Error::other("ZIP entry ended unexpectedly")),
                    Err(_) if token.is_cancelled() => return Err(Error::Cancelled),
                    Err(error) => return Err(error),
                }
                completed += 1;
                task.progress(completed, total_files, copied, total_bytes);
            }
            task.stage("validating");
            finish_staged(files, saves, &staging, &candidate.summary.name)
        })();
        if result.is_err() {
            let _ = files.remove_managed_dir_all_if_exists(&staging);
            return result.map(|_| 0);
        }
    }
    Ok(selected_ids.len())
}

pub fn import_selected(
    files: &FileManager,
    instance_id: &str,
    source: &Path,
    candidate_ids: &[String],
    task: &TaskHandle,
) -> Result<usize> {
    if candidate_ids.is_empty() {
        return Err(Error::other("select at least one world to import"));
    }
    let saves = files
        .paths()
        .instance_saves_dir_checked(instance_id)
        .ok_or_else(|| Error::other("invalid instance id"))?;
    files.ensure_dir(&saves)?;
    task.stage("importing");
    let selected_ids = candidate_ids.iter().cloned().collect::<HashSet<_>>();
    if selected_ids.len() != candidate_ids.len() {
        return Err(Error::other("duplicate world selection"));
    }

    if is_zip(source) {
        import_archive(files, source, &selected_ids, &saves, task)
    } else {
        let candidate = inspect_directory(files, source)?;
        if selected_ids.len() != 1 || !selected_ids.contains(&candidate.summary.id) {
            return Err(Error::other(
                "world selection no longer matches the source directory",
            ));
        }
        if candidate.summary.status == WorldStatus::Damaged {
            return Err(Error::other("damaged worlds cannot be imported"));
        }
        import_directory(files, source, &candidate, &saves, task)?;
        Ok(1)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::{write::GzEncoder, Compression};
    use zip::write::SimpleFileOptions;

    use super::{inspect_source, sanitize_archive_path};
    use crate::{files::FileManager, paths::Paths};

    fn files() -> FileManager {
        let root =
            std::env::temp_dir().join(format!("basalt-world-import-{}", uuid::Uuid::new_v4()));
        FileManager::new(Paths::plain(root)).unwrap()
    }

    fn named_tag(tag: u8, name: &str, payload: &[u8], out: &mut Vec<u8>) {
        out.push(tag);
        out.extend_from_slice(&(name.len() as u16).to_be_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(payload);
    }

    fn compressed_level(name: &str) -> Vec<u8> {
        let mut name_payload = Vec::new();
        name_payload.extend_from_slice(&(name.len() as u16).to_be_bytes());
        name_payload.extend_from_slice(name.as_bytes());
        let mut data = Vec::new();
        named_tag(8, "LevelName", &name_payload, &mut data);
        data.push(0);
        let mut root = vec![10, 0, 0];
        named_tag(10, "Data", &data, &mut root);
        root.push(0);

        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&root).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn archive_inspection_detects_multiple_world_roots() {
        let files = files();
        let archive_path =
            std::env::temp_dir().join(format!("basalt-worlds-{}.zip", uuid::Uuid::new_v4()));
        let archive_file = std::fs::File::create(&archive_path).unwrap();
        let mut archive = zip::ZipWriter::new(archive_file);
        for name in ["One", "Two"] {
            archive
                .start_file(
                    format!("backup/{name}/level.dat"),
                    SimpleFileOptions::default(),
                )
                .unwrap();
            archive.write_all(&compressed_level(name)).unwrap();
            archive
                .start_file(
                    format!("backup/{name}/region/r.0.0.mca"),
                    SimpleFileOptions::default(),
                )
                .unwrap();
            archive.write_all(b"region").unwrap();
        }
        archive.finish().unwrap();

        let inspection = inspect_source(&files, &archive_path).unwrap();
        assert_eq!(inspection.candidates.len(), 2);
        assert_eq!(inspection.candidates[0].name, "One");
        assert_eq!(inspection.candidates[1].name, "Two");
        assert_ne!(inspection.candidates[0].id, inspection.candidates[1].id);
        std::fs::remove_file(archive_path).unwrap();
    }

    #[test]
    fn archive_paths_cannot_escape_or_use_windows_separators() {
        assert!(sanitize_archive_path("../world/level.dat").is_err());
        assert!(sanitize_archive_path("/world/level.dat").is_err());
        assert!(sanitize_archive_path(r"..\world\level.dat").is_err());
        assert!(sanitize_archive_path("world/level.dat").is_ok());
    }
}
