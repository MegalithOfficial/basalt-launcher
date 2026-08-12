use std::{
    collections::{HashMap, HashSet},
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use serde::Deserialize;

use super::{curseforge, model::ContentKind, model::Provider, modrinth, resolve_projects};
use crate::{content, db::ContentFile, error::Result, state::AppState};

const MURMUR_M: u32 = 0x5bd1_e995;
const MURMUR_R: u32 = 24;
const CURSEFORGE_SEED: u32 = 1;

fn is_ignored_byte(byte: u8) -> bool {
    matches!(byte, 0x09 | 0x0a | 0x0d | 0x20)
}

pub fn murmur2(data: &[u8], seed: u32) -> u32 {
    let mut hash = seed ^ (data.len() as u32);
    let mut chunks = data.chunks_exact(4);

    for chunk in chunks.by_ref() {
        let mut k = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        k = k.wrapping_mul(MURMUR_M);
        k ^= k >> MURMUR_R;
        k = k.wrapping_mul(MURMUR_M);
        hash = hash.wrapping_mul(MURMUR_M);
        hash ^= k;
    }

    let tail = chunks.remainder();
    if !tail.is_empty() {
        if tail.len() >= 3 {
            hash ^= (tail[2] as u32) << 16;
        }
        if tail.len() >= 2 {
            hash ^= (tail[1] as u32) << 8;
        }
        hash ^= tail[0] as u32;
        hash = hash.wrapping_mul(MURMUR_M);
    }

    hash ^= hash >> 13;
    hash = hash.wrapping_mul(MURMUR_M);
    hash ^= hash >> 15;
    hash
}

pub fn curseforge_fingerprint(bytes: &[u8]) -> u32 {
    let stripped: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|b| !is_ignored_byte(*b))
        .collect();
    murmur2(&stripped, CURSEFORGE_SEED)
}

pub(crate) fn curseforge_fingerprint_reader<R: Read + Seek>(
    reader: &mut R,
) -> std::io::Result<u32> {
    let mut filtered_len = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        filtered_len += buffer[..read]
            .iter()
            .filter(|byte| !is_ignored_byte(**byte))
            .count() as u64;
    }

    reader.seek(SeekFrom::Start(0))?;
    let mut hash = CURSEFORGE_SEED ^ filtered_len as u32;
    let mut block = [0_u8; 4];
    let mut block_len = 0;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        for byte in buffer[..read]
            .iter()
            .copied()
            .filter(|byte| !is_ignored_byte(*byte))
        {
            block[block_len] = byte;
            block_len += 1;
            if block_len == block.len() {
                let mut value = u32::from_le_bytes(block);
                value = value.wrapping_mul(MURMUR_M);
                value ^= value >> MURMUR_R;
                value = value.wrapping_mul(MURMUR_M);
                hash = hash.wrapping_mul(MURMUR_M);
                hash ^= value;
                block_len = 0;
            }
        }
    }

    if block_len >= 3 {
        hash ^= (block[2] as u32) << 16;
    }
    if block_len >= 2 {
        hash ^= (block[1] as u32) << 8;
    }
    if block_len >= 1 {
        hash ^= block[0] as u32;
        hash = hash.wrapping_mul(MURMUR_M);
    }
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(MURMUR_M);
    hash ^= hash >> 15;
    Ok(hash)
}

#[derive(Debug, Clone, Default)]
pub struct FileIdentity {
    pub sha1: String,
    pub murmur2: u32,
    pub mod_id: Option<String>,
    pub mod_version: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Deserialize)]
struct FabricMod {
    id: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct QuiltMod {
    quilt_loader: QuiltLoader,
}

#[derive(Deserialize)]
struct QuiltLoader {
    id: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    metadata: Option<QuiltMetadata>,
}

#[derive(Deserialize)]
struct QuiltMetadata {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct ForgeManifest {
    #[serde(default)]
    mods: Vec<ForgeMod>,
}

#[derive(Deserialize)]
struct ForgeMod {
    #[serde(rename = "modId")]
    mod_id: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(rename = "displayName", default)]
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct LegacyForgeMod {
    #[serde(default)]
    modid: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct PackMeta {
    pack: PackMetaInner,
}

#[derive(Deserialize)]
struct PackMetaInner {
    #[serde(default)]
    description: Option<serde_json::Value>,
}

fn placeholder(value: &str) -> bool {
    value.contains("${")
}

fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty() && !placeholder(v))
}

fn read_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    use std::io::Read;

    let mut entry = zip.by_name(name).ok()?;
    let mut body = String::new();
    entry.read_to_string(&mut body).ok()?;
    Some(body)
}

pub fn read_metadata(
    files: &crate::files::FileManager,
    path: &Path,
) -> Option<(Option<String>, Option<String>, Option<String>)> {
    let file = files.open(path).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;

    if let Some(body) = read_entry(&mut zip, "fabric.mod.json") {
        if let Ok(parsed) = serde_json::from_str::<FabricMod>(&body) {
            return Some((Some(parsed.id), clean(parsed.version), clean(parsed.name)));
        }
    }

    if let Some(body) = read_entry(&mut zip, "quilt.mod.json") {
        if let Ok(parsed) = serde_json::from_str::<QuiltMod>(&body) {
            return Some((
                Some(parsed.quilt_loader.id),
                clean(parsed.quilt_loader.version),
                clean(parsed.quilt_loader.metadata.and_then(|m| m.name)),
            ));
        }
    }

    for name in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Some(body) = read_entry(&mut zip, name) {
            if let Ok(parsed) = toml::from_str::<ForgeManifest>(&body) {
                if let Some(first) = parsed.mods.into_iter().next() {
                    return Some((
                        Some(first.mod_id),
                        clean(first.version),
                        clean(first.display_name),
                    ));
                }
            }
        }
    }

    if let Some(body) = read_entry(&mut zip, "mcmod.info") {
        if let Ok(parsed) = serde_json::from_str::<Vec<LegacyForgeMod>>(&body) {
            if let Some(first) = parsed.into_iter().next() {
                return Some((first.modid, clean(first.version), clean(first.name)));
            }
        }
    }

    if let Some(body) = read_entry(&mut zip, "pack.mcmeta") {
        if let Ok(parsed) = serde_json::from_str::<PackMeta>(&body) {
            let description = match parsed.pack.description {
                Some(serde_json::Value::String(s)) => clean(Some(s)),
                _ => None,
            };
            return Some((None, None, description));
        }
    }

    None
}

pub fn identify_file(files: &crate::files::FileManager, path: &Path) -> Result<FileIdentity> {
    let bytes = files.read(path)?;
    let (mod_id, mod_version, display_name) =
        read_metadata(files, path).unwrap_or((None, None, None));
    Ok(FileIdentity {
        sha1: crate::download::sha1_hex(&bytes),
        murmur2: curseforge_fingerprint(&bytes),
        mod_id,
        mod_version,
        display_name,
    })
}

fn provider_of(file: &ContentFile) -> Option<Provider> {
    file.provider
        .as_deref()
        .and_then(|provider| Provider::parse(provider).ok())
}

fn expected_provider(
    file: Option<&ContentFile>,
    pack_provider: Option<Provider>,
) -> Option<Provider> {
    let file_provider = file.and_then(provider_of);
    if file.is_some_and(|entry| entry.origin == "pack") {
        pack_provider.or(file_provider)
    } else {
        file_provider.or(pack_provider)
    }
}

fn needs_provider_identity(file: Option<&ContentFile>, pack_provider: Option<Provider>) -> bool {
    file.is_none_or(|entry| {
        entry.project_id.is_none()
            || (entry.origin == "pack"
                && pack_provider.is_some()
                && expected_provider(Some(entry), pack_provider) != provider_of(entry))
    })
}

pub async fn reconcile(
    state: &AppState,
    target: crate::search::resolve::Target<'_>,
    kind: &str,
) -> Result<()> {
    let content_kind = ContentKind::parse(kind)?;
    let dir = target.dir(state, content_kind)?;
    let items = content::list_in(&state.files, &dir)?;
    if items.is_empty() {
        return Ok(());
    }

    let known: HashMap<String, ContentFile> = target
        .installed(state, content_kind)
        .into_iter()
        .map(|f| (f.file_name.clone(), f))
        .collect();
    let pack_provider = target.pack_provider(state);
    let mut hashed: Vec<(String, String, u32)> = Vec::new();

    for item in &items {
        let existing = known.get(&item.file_name);
        if existing.is_some_and(|file| file.sha1.is_some() && file.murmur2.is_some()) {
            let file = existing.expect("checked above");
            hashed.push((
                item.file_name.clone(),
                file.sha1.clone().expect("checked above"),
                file.murmur2.unwrap_or(0) as u32,
            ));
            continue;
        }

        let path = content::resolve_path(&state.files, &dir, &item.file_name);
        let Ok(identity) = identify_file(&state.files, &path) else {
            continue;
        };
        target.merge_identity(
            state,
            content_kind,
            &item.file_name,
            Some(&identity.sha1),
            None,
            Some(identity.murmur2 as i64),
            identity.mod_id.as_deref(),
            identity.mod_version.as_deref(),
        )?;
        if existing.is_none() {
            if let Some(name) = &identity.display_name {
                target.set_fallback_title(state, content_kind, &item.file_name, name)?;
            }
        }
        hashed.push((item.file_name.clone(), identity.sha1, identity.murmur2));
    }

    for provider in [Provider::Modrinth, Provider::Curseforge] {
        if provider == Provider::Curseforge && curseforge::key(state).is_err() {
            continue;
        }
        let missing_metadata: Vec<(&String, &String)> = known
            .iter()
            .filter_map(|(file_name, file)| {
                if file.provider.as_deref() != Some(provider.as_str())
                    || (file.title.is_some() && file.icon_url.is_some())
                {
                    return None;
                }
                Some((file_name, file.project_id.as_ref()?))
            })
            .collect();
        if missing_metadata.is_empty() {
            continue;
        }

        let mut project_ids: Vec<String> = missing_metadata
            .iter()
            .map(|(_, project_id)| (*project_id).clone())
            .collect();
        project_ids.sort();
        project_ids.dedup();

        let Ok(projects) = resolve_projects(state, provider, &project_ids).await else {
            continue;
        };
        let project_info: HashMap<&str, &super::ProjectSummary> = projects
            .iter()
            .map(|project| (project.id.as_str(), project))
            .collect();

        for (file_name, project_id) in missing_metadata {
            let Some(project) = project_info.get(project_id.as_str()) else {
                continue;
            };
            target.merge_provider_identity(
                state,
                content_kind,
                file_name,
                provider.as_str(),
                project_id,
                None,
                Some(&project.title),
                project.icon_url.as_deref(),
            )?;
        }
    }

    let mut unlinked: Vec<(String, String, u32)> = hashed
        .iter()
        .filter(|(name, _, _)| needs_provider_identity(known.get(name), pack_provider))
        .cloned()
        .collect();

    if unlinked.is_empty() {
        return Ok(());
    }

    let curseforge_first: Vec<(String, String, u32)> = unlinked
        .iter()
        .filter(|(name, _, _)| {
            expected_provider(known.get(name), pack_provider) == Some(Provider::Curseforge)
        })
        .cloned()
        .collect();
    let tried_curseforge: HashSet<String> = curseforge_first
        .iter()
        .map(|(name, _, _)| name.clone())
        .collect();
    if !curseforge_first.is_empty() {
        let matched =
            link_curseforge_matches(state, target, content_kind, &curseforge_first).await?;
        unlinked.retain(|(name, _, _)| !matched.contains(name));
        if unlinked.is_empty() {
            return Ok(());
        }
    }

    let sha1s: Vec<String> = unlinked.iter().map(|(_, sha1, _)| sha1.clone()).collect();
    let by_hash = modrinth::versions_by_hash(state, &sha1s)
        .await
        .unwrap_or_default();

    let project_ids: Vec<String> = {
        let mut ids: Vec<String> = by_hash.values().map(|v| v.project_id.clone()).collect();
        ids.sort();
        ids.dedup();
        ids
    };
    let projects = modrinth::resolve_projects(state, &project_ids)
        .await
        .unwrap_or_default();
    let info: HashMap<&str, &super::ProjectSummary> =
        projects.iter().map(|p| (p.id.as_str(), p)).collect();

    let mut still_unknown = Vec::new();
    for (file_name, sha1, fingerprint) in &unlinked {
        match by_hash.get(sha1) {
            Some(version) => {
                let project = info.get(version.project_id.as_str());
                target.merge_provider_identity(
                    state,
                    content_kind,
                    file_name,
                    Provider::Modrinth.as_str(),
                    &version.project_id,
                    Some(&version.id),
                    project.map(|p| p.title.as_str()),
                    project.and_then(|p| p.icon_url.as_deref()),
                )?;
            }
            None => still_unknown.push((file_name.clone(), sha1.clone(), *fingerprint)),
        }
    }

    let curseforge_fallback: Vec<(String, String, u32)> = still_unknown
        .into_iter()
        .filter(|(name, _, _)| !tried_curseforge.contains(name))
        .collect();
    if curseforge_fallback.is_empty() {
        return Ok(());
    }

    link_curseforge_matches(state, target, content_kind, &curseforge_fallback).await?;

    Ok(())
}

async fn link_curseforge_matches(
    state: &AppState,
    target: crate::search::resolve::Target<'_>,
    content_kind: ContentKind,
    candidates: &[(String, String, u32)],
) -> Result<HashSet<String>> {
    if candidates.is_empty() || curseforge::key(state).is_err() {
        return Ok(HashSet::new());
    }
    let fingerprints: Vec<u32> = candidates
        .iter()
        .map(|(_, _, fingerprint)| *fingerprint)
        .collect();
    let Ok(matches) = curseforge::match_fingerprints(state, &fingerprints).await else {
        return Ok(HashSet::new());
    };
    let by_fingerprint: HashMap<u32, &curseforge::FingerprintMatch> =
        matches.iter().map(|m| (m.id as u32, m)).collect();

    let mod_ids: Vec<String> = matches.iter().map(|m| m.file.mod_id.to_string()).collect();
    let cf_projects = curseforge::resolve_projects(state, &mod_ids)
        .await
        .unwrap_or_default();
    let cf_info: HashMap<&str, &super::ProjectSummary> =
        cf_projects.iter().map(|p| (p.id.as_str(), p)).collect();

    let mut linked = HashSet::new();
    for (file_name, _, fingerprint) in candidates {
        let Some(entry) = by_fingerprint.get(fingerprint) else {
            continue;
        };
        let project_id = entry.file.mod_id.to_string();
        let project = cf_info.get(project_id.as_str());
        target.merge_provider_identity(
            state,
            content_kind,
            file_name,
            Provider::Curseforge.as_str(),
            &project_id,
            Some(&entry.file.id.to_string()),
            project.map(|p| p.title.as_str()),
            project.and_then(|p| p.icon_url.as_deref()),
        )?;
        linked.insert(file_name.clone());
    }
    Ok(linked)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{
        curseforge_fingerprint, curseforge_fingerprint_reader, expected_provider, is_ignored_byte,
        murmur2, needs_provider_identity,
    };
    use crate::{db::ContentFile, search::Provider};

    fn linked(provider: Provider, origin: &str) -> ContentFile {
        ContentFile {
            provider: Some(provider.as_str().to_string()),
            project_id: Some("project".to_string()),
            origin: origin.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn a_pack_file_prefers_the_platform_its_pack_came_from() {
        let stale = linked(Provider::Modrinth, "pack");
        assert_eq!(
            expected_provider(Some(&stale), Some(Provider::Curseforge)),
            Some(Provider::Curseforge)
        );
        assert!(needs_provider_identity(
            Some(&stale),
            Some(Provider::Curseforge)
        ));
    }

    #[test]
    fn a_user_installed_mod_keeps_its_own_platform() {
        let installed = linked(Provider::Modrinth, "user");
        assert_eq!(
            expected_provider(Some(&installed), Some(Provider::Curseforge)),
            Some(Provider::Modrinth)
        );
        assert!(!needs_provider_identity(
            Some(&installed),
            Some(Provider::Curseforge)
        ));
    }

    #[test]
    fn murmur_is_deterministic_and_content_sensitive() {
        assert_eq!(murmur2(b"basalt", 1), murmur2(b"basalt", 1));
        assert_ne!(murmur2(b"basalt", 1), murmur2(b"basalu", 1));
        assert_ne!(murmur2(b"basalt", 1), murmur2(b"basalt", 2));
    }

    #[test]
    fn murmur_handles_every_tail_length() {
        for len in 0..9 {
            let data = vec![b'x'; len];
            assert_eq!(murmur2(&data, 1), murmur2(&data, 1));
        }
    }

    #[test]
    fn fingerprint_ignores_whitespace_bytes() {
        assert_eq!(
            curseforge_fingerprint(b"abc"),
            curseforge_fingerprint(b" a\tb\r\nc ")
        );
        assert_ne!(
            curseforge_fingerprint(b"abc"),
            curseforge_fingerprint(b"abd")
        );
    }

    #[test]
    fn streaming_fingerprint_matches_the_in_memory_implementation() {
        let mut cases = (0..=8).map(|length| vec![b'x'; length]).collect::<Vec<_>>();
        cases.push(b" a\tb\r\nc ".to_vec());
        let mut across_buffer = vec![b'x'; 64 * 1024 - 1];
        across_buffer.extend_from_slice(b" \txyz\r\nmore");
        cases.push(across_buffer);

        for bytes in cases {
            let mut reader = Cursor::new(&bytes);
            assert_eq!(
                curseforge_fingerprint_reader(&mut reader).unwrap(),
                curseforge_fingerprint(&bytes)
            );
        }
    }

    #[test]
    fn ignored_bytes_are_the_curseforge_set() {
        assert!(is_ignored_byte(b' '));
        assert!(is_ignored_byte(b'\t'));
        assert!(is_ignored_byte(b'\n'));
        assert!(is_ignored_byte(b'\r'));
        assert!(!is_ignored_byte(b'a'));
        assert!(!is_ignored_byte(0));
    }
}
