use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::{
    db::SkinRecord,
    error::{Error, Result},
    state::AppState,
};

const PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const NAME_LOOKUP_URL: &str = "https://api.mojang.com/users/profiles/minecraft";
const SESSION_URL: &str = "https://sessionserver.mojang.com/session/minecraft/profile";
const MAX_SKIN_BYTES: u64 = 512 * 1024;
const TEXTURE_BASE: &str = "https://textures.minecraft.net/texture/";
const TEXTURE_MARKER: &str = "textures.minecraft.net/texture/";
const NAMEMC_MARKER: &str = "namemc.com/skin/";
const NAMEMC_BASE: &str = "https://s.namemc.com/i/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Classic,
    Slim,
}

impl Variant {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "slim" | "alex" => Variant::Slim,
            _ => Variant::Classic,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Variant::Slim => "slim",
            Variant::Classic => "classic",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SkinEntry {
    pub id: String,
    pub name: String,
    pub variant: String,
    pub source: Option<String>,
    pub data_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapeEntry {
    pub id: String,
    pub alias: String,
    pub url: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Appearance {
    pub uuid: String,
    pub name: String,
    pub skin_url: Option<String>,
    pub variant: String,
    pub capes: Vec<CapeEntry>,
    pub active_cape_id: Option<String>,
    pub library_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProfileSkin {
    #[serde(default)]
    state: String,
    url: String,
    #[serde(default)]
    variant: String,
}

#[derive(Debug, Deserialize)]
struct ProfileCape {
    id: String,
    #[serde(default)]
    state: String,
    url: String,
    #[serde(default)]
    alias: String,
}

#[derive(Debug, Deserialize)]
struct ProfileResponse {
    id: String,
    name: String,
    #[serde(default)]
    skins: Vec<ProfileSkin>,
    #[serde(default)]
    capes: Vec<ProfileCape>,
}

fn is_active(state: &str) -> bool {
    state.eq_ignore_ascii_case("ACTIVE")
}

impl From<ProfileResponse> for Appearance {
    fn from(profile: ProfileResponse) -> Self {
        let active_skin = profile
            .skins
            .iter()
            .find(|s| is_active(&s.state))
            .or_else(|| profile.skins.first());
        let capes: Vec<CapeEntry> = profile
            .capes
            .iter()
            .map(|cape| CapeEntry {
                id: cape.id.clone(),
                alias: if cape.alias.is_empty() {
                    "Cape".to_string()
                } else {
                    cape.alias.clone()
                },
                url: cape.url.clone(),
                active: is_active(&cape.state),
            })
            .collect();
        let active_cape_id = capes.iter().find(|c| c.active).map(|c| c.id.clone());

        Appearance {
            uuid: profile.id,
            name: profile.name,
            skin_url: active_skin.map(|s| s.url.clone()),
            variant: active_skin
                .map(|s| Variant::parse(&s.variant).as_str())
                .unwrap_or("classic")
                .to_string(),
            capes,
            active_cape_id,
            library_id: None,
        }
    }
}

async fn token(state: &AppState) -> Result<String> {
    let account = crate::launch::ensure_account(state).await?;
    Ok(account.mc_access_token)
}

async fn profile(state: &AppState, token: &str) -> Result<Appearance> {
    let request = state.network.get(PROFILE_URL).bearer_auth(token);
    let response = state.network.send(request).await?.error_for_status()?;
    let parsed: ProfileResponse = response.json().await?;
    Ok(parsed.into())
}

fn worn_key(uuid: &str) -> String {
    format!("worn_skin:{uuid}")
}

fn remember_worn(state: &AppState, uuid: &str, skin_id: &str) {
    if let Err(e) = state.db.put_kv(&worn_key(uuid), skin_id) {
        tracing::warn!(error = %e, "could not remember the worn skin");
    }
}

pub fn worn_skin(state: &AppState, uuid: &str) -> Result<Option<SkinEntry>> {
    let Some(id) = state.db.get_kv(&worn_key(uuid))? else {
        return Ok(None);
    };
    let Some(record) = state.db.find_skin(&id)? else {
        return Ok(None);
    };
    Ok(entry_for(&state.files, record))
}

async fn current(state: &AppState, token: &str) -> Result<Appearance> {
    let mut worn = profile(state, token).await?;
    worn.library_id = capture_worn(state, &worn).await;
    if let Some(id) = &worn.library_id {
        remember_worn(state, &worn.uuid, id);
    }
    Ok(worn)
}

#[tracing::instrument(skip_all, err)]
pub async fn appearance(state: &AppState) -> Result<Appearance> {
    let token = token(state).await?;
    current(state, &token).await
}

async fn capture_worn(state: &AppState, current: &Appearance) -> Option<String> {
    let url = current.skin_url.as_ref()?;
    let bytes = state
        .network
        .send(state.network.get(url))
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .bytes()
        .await
        .ok()?;

    let hash = content_hash(&bytes);
    match state.db.find_skin_by_hash(&hash) {
        Ok(Some(existing)) => return Some(existing),
        Ok(None) => {}
        Err(e) => {
            tracing::warn!(error = %e, "could not look up worn skin");
            return None;
        }
    }

    match store(
        state,
        &bytes,
        &current.name,
        Variant::parse(&current.variant),
        Some("worn"),
    ) {
        Ok(entry) => {
            tracing::info!(name = %entry.name, "saved the worn skin to the library");
            Some(entry.id)
        }
        Err(e) => {
            tracing::warn!(error = %e, "could not save the worn skin");
            None
        }
    }
}

pub fn is_skin_shaped(width: u32, height: u32) -> bool {
    if width < 64 || !width.is_multiple_of(64) {
        return false;
    }
    let scale = width / 64;
    height == 32 * scale || height == 64 * scale
}

fn validate_png(bytes: &[u8]) -> Result<()> {
    const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() as u64 > MAX_SKIN_BYTES {
        return Err(Error::other("That skin file is too large."));
    }
    if !bytes.starts_with(PNG_MAGIC) {
        return Err(Error::other("A skin must be a PNG image."));
    }
    let decoded =
        image::load_from_memory(bytes).map_err(|_| Error::other("That PNG could not be read."))?;
    let (width, height) = (decoded.width(), decoded.height());
    if !is_skin_shaped(width, height) {
        return Err(Error::other(format!(
            "A skin must be 64x64 or 64x32 pixels, but that image is {width}x{height}."
        )));
    }
    Ok(())
}

#[tracing::instrument(skip_all, fields(variant = variant.as_str(), bytes = bytes.len()), err)]
pub async fn upload(state: &AppState, bytes: Vec<u8>, variant: Variant) -> Result<Appearance> {
    validate_png(&bytes)?;
    let token = token(state).await?;

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("skin.png")
        .mime_str("image/png")
        .map_err(|e| Error::other(format!("could not build skin upload: {e}")))?;
    let form = reqwest::multipart::Form::new()
        .text("variant", variant.as_str())
        .part("file", part);

    let request = state
        .network
        .post(format!("{PROFILE_URL}/skins"))
        .bearer_auth(&token)
        .multipart(form);
    let response = state.network.send_once(request).await?;

    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        tracing::error!(%status, detail, "skin upload rejected");
        return Err(Error::other(format!(
            "Mojang rejected the skin ({status})."
        )));
    }

    tracing::info!("skin applied");
    profile(state, &token).await
}

#[tracing::instrument(skip_all, err)]
pub async fn reset(state: &AppState) -> Result<Appearance> {
    let token = token(state).await?;
    let request = state
        .network
        .delete(format!("{PROFILE_URL}/skins/active"))
        .bearer_auth(&token);
    state.network.send(request).await?.error_for_status()?;
    tracing::info!("skin reset to default");
    profile(state, &token).await
}

#[tracing::instrument(skip_all, err)]
pub async fn set_cape(state: &AppState, cape_id: Option<&str>) -> Result<Appearance> {
    let token = token(state).await?;
    let url = format!("{PROFILE_URL}/capes/active");
    let request = match cape_id {
        Some(id) => state
            .network
            .put(&url)
            .bearer_auth(&token)
            .json(&serde_json::json!({ "capeId": id })),
        None => state.network.delete(&url).bearer_auth(&token),
    };
    state.network.send(request).await?.error_for_status()?;
    tracing::info!(cape = ?cape_id, "cape updated");
    current(state, &token).await
}

#[derive(Debug, Deserialize)]
struct NameLookup {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct SessionProfile {
    #[serde(default)]
    properties: Vec<SessionProperty>,
}

#[derive(Debug, Deserialize)]
struct SessionProperty {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct TexturePayload {
    #[serde(default, rename = "profileName")]
    profile_name: Option<String>,
    #[serde(default)]
    textures: Textures,
}

#[derive(Debug, Default, Deserialize)]
struct Textures {
    #[serde(rename = "SKIN")]
    skin: Option<TextureEntry>,
}

#[derive(Debug, Deserialize)]
struct TextureEntry {
    url: String,
    #[serde(default)]
    metadata: Option<TextureMetadata>,
}

#[derive(Debug, Deserialize)]
struct TextureMetadata {
    #[serde(default)]
    model: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedPlayer {
    pub name: String,
    pub skin_url: String,
    pub variant: Variant,
}

/// Resolves a username to its current skin the same way public skin sites do:
/// name to uuid, then the session profile's base64 texture payload.
#[tracing::instrument(skip(state), err)]
pub async fn resolve_uuid(state: &AppState, uuid: &str) -> Result<ResolvedPlayer> {
    let session: SessionProfile = state
        .network
        .send(state.network.get(format!("{SESSION_URL}/{uuid}")))
        .await?
        .error_for_status()?
        .json()
        .await?;
    textures_from(session, uuid)
}

fn textures_from(session: SessionProfile, label: &str) -> Result<ResolvedPlayer> {
    let encoded = session
        .properties
        .iter()
        .find(|p| p.name == "textures")
        .ok_or_else(|| Error::other("That profile has no textures."))?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&encoded.value)
        .map_err(|e| Error::other(format!("could not decode textures: {e}")))?;
    let payload: TexturePayload = serde_json::from_slice(&decoded)?;
    let name = payload
        .profile_name
        .clone()
        .unwrap_or_else(|| label.to_string());
    let skin = payload
        .textures
        .skin
        .ok_or_else(|| Error::other(format!("{name} has no custom skin.")))?;
    let variant = skin
        .metadata
        .as_ref()
        .map(|m| Variant::parse(&m.model))
        .unwrap_or(Variant::Classic);
    Ok(ResolvedPlayer {
        name,
        skin_url: skin.url,
        variant,
    })
}

#[tracing::instrument(skip(state), err)]
pub async fn resolve_player(state: &AppState, name: &str) -> Result<ResolvedPlayer> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::other("Enter a player name."));
    }

    let response = state
        .network
        .send(state.network.get(format!("{NAME_LOOKUP_URL}/{name}")))
        .await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND
        || response.status() == reqwest::StatusCode::NO_CONTENT
    {
        return Err(Error::NotFound(format!("No player named {name}")));
    }
    let looked_up: NameLookup = response.error_for_status()?.json().await?;

    let session: SessionProfile = state
        .network
        .send(state.network.get(format!("{SESSION_URL}/{}", looked_up.id)))
        .await?
        .error_for_status()?
        .json()
        .await?;

    let resolved = textures_from(session, &looked_up.name)?;
    tracing::info!(player = %resolved.name, variant = resolved.variant.as_str(), "resolved player skin");
    Ok(resolved)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkinRef {
    Texture(String),
    NameMc(String),
    Uuid(String),
    Name(String),
}

fn hex_run_after(haystack: &str, marker: &str, len: usize) -> Option<String> {
    let start = haystack.find(marker)? + marker.len();
    let run: String = haystack[start..]
        .chars()
        .take_while(|c| c.is_ascii_hexdigit())
        .collect();
    (run.len() >= len).then(|| run[..len].to_lowercase())
}

fn base64_blobs(input: &str) -> Vec<String> {
    let mut blobs = Vec::new();
    let mut current = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '+' || ch == '/' || ch == '=' {
            current.push(ch);
        } else {
            if current.len() >= 40 {
                blobs.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    if current.len() >= 40 {
        blobs.push(current);
    }
    blobs
}

fn uuid_from_int_array(input: &str) -> Option<String> {
    let start = input.find("[I;")? + 3;
    let end = input[start..].find(']')? + start;
    let parts: Vec<i32> = input[start..end]
        .split(',')
        .map(|p| p.trim().parse::<i32>())
        .collect::<std::result::Result<Vec<_>, _>>()
        .ok()?;
    if parts.len() != 4 {
        return None;
    }
    Some(
        parts
            .iter()
            .map(|n| format!("{:08x}", *n as u32))
            .collect::<String>(),
    )
}

fn normalized_uuid(value: &str) -> Option<String> {
    let stripped: String = value.chars().filter(|c| *c != '-').collect();
    (stripped.len() == 32 && stripped.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| stripped.to_lowercase())
}

pub fn parse_reference(input: &str) -> Result<SkinRef> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(Error::other("Enter a name, UUID or skin link."));
    }

    if let Some(hash) = hex_run_after(trimmed, TEXTURE_MARKER, 64) {
        return Ok(SkinRef::Texture(format!("{TEXTURE_BASE}{hash}")));
    }

    for blob in base64_blobs(trimmed) {
        let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(&blob) else {
            continue;
        };
        let text = String::from_utf8_lossy(&decoded);
        if let Some(hash) = hex_run_after(&text, TEXTURE_MARKER, 64) {
            return Ok(SkinRef::Texture(format!("{TEXTURE_BASE}{hash}")));
        }
    }

    if let Some(id) = hex_run_after(trimmed, NAMEMC_MARKER, 16) {
        return Ok(SkinRef::NameMc(id));
    }

    if let Some(uuid) = uuid_from_int_array(trimmed) {
        return Ok(SkinRef::Uuid(uuid));
    }

    let bare = trimmed
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    if bare.len() == 64 && bare.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(SkinRef::Texture(format!(
            "{TEXTURE_BASE}{}",
            bare.to_lowercase()
        )));
    }
    if let Some(uuid) = normalized_uuid(bare) {
        return Ok(SkinRef::Uuid(uuid));
    }
    if bare.len() == 16 && bare.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(SkinRef::NameMc(bare.to_lowercase()));
    }

    if trimmed.len() > 16
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(Error::other(
            "That does not look like a player name, UUID or skin link.",
        ));
    }
    Ok(SkinRef::Name(trimmed.to_string()))
}

fn content_hash(bytes: &[u8]) -> String {
    match image::load_from_memory(bytes) {
        Ok(decoded) => crate::download::sha1_hex(decoded.to_rgba8().as_raw()),
        Err(_) => crate::download::sha1_hex(bytes),
    }
}

fn unique_name(state: &AppState, base: &str) -> String {
    let Ok(existing) = state.db.list_skins() else {
        return base.to_string();
    };
    if !existing.iter().any(|s| s.name == base) {
        return base.to_string();
    }
    for n in 2..999 {
        let candidate = format!("{base} ({n})");
        if !existing.iter().any(|s| s.name == candidate) {
            return candidate;
        }
    }
    base.to_string()
}

fn detect_variant(bytes: &[u8]) -> Variant {
    let Ok(image) = image::load_from_memory(bytes) else {
        return Variant::Classic;
    };
    let rgba = image.to_rgba8();
    if rgba.width() < 64 || rgba.height() < 64 {
        return Variant::Classic;
    }
    let scale = rgba.width() / 64;
    let transparent = |x: u32, y: u32| rgba.get_pixel(x * scale, y * scale).0[3] == 0;
    if transparent(54, 20) || transparent(46, 52) {
        Variant::Slim
    } else {
        Variant::Classic
    }
}

fn data_url(bytes: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn entry_for(files: &crate::files::FileManager, record: SkinRecord) -> Option<SkinEntry> {
    let bytes = files
        .read(files.paths().skins().join(&record.file_name))
        .ok()?;
    Some(SkinEntry {
        id: record.id,
        name: record.name,
        variant: record.variant,
        source: record.source,
        data_url: data_url(&bytes),
    })
}

pub fn library(state: &AppState) -> Result<Vec<SkinEntry>> {
    Ok(state
        .db
        .list_skins()?
        .into_iter()
        .filter_map(|record| entry_for(&state.files, record))
        .collect())
}

fn store(
    state: &AppState,
    bytes: &[u8],
    name: &str,
    variant: Variant,
    source: Option<&str>,
) -> Result<SkinEntry> {
    validate_png(bytes)?;
    let hash = content_hash(bytes);
    if let Some(existing) = state.db.find_skin_by_hash(&hash)? {
        if let Some(record) = state.db.find_skin(&existing)? {
            tracing::debug!(name = %record.name, "skin already in the library");
            if let Some(entry) = entry_for(&state.files, record) {
                return Ok(entry);
            }
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    let file_name = format!("{id}.png");
    state
        .files
        .write_atomic(state.paths.skins().join(&file_name), bytes)?;

    let record = SkinRecord {
        id,
        name: unique_name(state, name.trim()),
        variant: variant.as_str().to_string(),
        file_name,
        source: source.map(|s| s.to_string()),
        hash: Some(hash),
        remote_hash: None,
        added_at: chrono::Utc::now().timestamp(),
    };
    state.db.insert_skin(&record)?;
    tracing::info!(name = %record.name, variant = variant.as_str(), "skin saved to library");

    entry_for(&state.files, record).ok_or_else(|| Error::other("skin vanished after saving"))
}

#[tracing::instrument(skip(state), err)]
pub fn add_from_file(
    state: &AppState,
    path: &str,
    name: Option<&str>,
    variant: &str,
) -> Result<SkinEntry> {
    let bytes = state.files.read_external(path)?;
    let fallback = std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Skin".to_string());
    let label = name.map(str::to_string).unwrap_or(fallback);
    store(state, &bytes, &label, Variant::parse(variant), Some("file"))
}

async fn download(state: &AppState, url: &str) -> Result<Vec<u8>> {
    let response = state.network.send(state.network.get(url)).await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(Error::NotFound(format!("no skin at {url}")));
    }
    Ok(response.error_for_status()?.bytes().await?.to_vec())
}

#[tracing::instrument(skip(state), err)]
pub async fn add_from_reference(state: &AppState, reference: &str) -> Result<SkinEntry> {
    let parsed = parse_reference(reference)?;
    tracing::debug!(?parsed, "resolved skin reference");

    match parsed {
        SkinRef::Name(name) => {
            let resolved = resolve_player(state, &name).await?;
            let bytes = download(state, &resolved.skin_url).await?;
            store(
                state,
                &bytes,
                &resolved.name,
                resolved.variant,
                Some(&format!("player:{}", resolved.name)),
            )
        }
        SkinRef::Uuid(uuid) => {
            let resolved = resolve_uuid(state, &uuid).await?;
            let bytes = download(state, &resolved.skin_url).await?;
            store(
                state,
                &bytes,
                &resolved.name,
                resolved.variant,
                Some(&format!("player:{}", resolved.name)),
            )
        }
        SkinRef::Texture(url) => {
            let bytes = download(state, &url).await?;
            let variant = detect_variant(&bytes);
            let label = url.rsplit('/').next().unwrap_or("texture");
            store(
                state,
                &bytes,
                &format!("Texture {}", &label[..label.len().min(8)]),
                variant,
                Some("texture"),
            )
        }
        SkinRef::NameMc(id) => {
            let url = format!("{NAMEMC_BASE}{id}.png");
            match download(state, &url).await {
                Ok(bytes) => {
                    let variant = detect_variant(&bytes);
                    store(
                        state,
                        &bytes,
                        &format!("NameMC {}", &id[..8]),
                        variant,
                        Some("namemc"),
                    )
                }
                Err(Error::NotFound(_)) => {
                    tracing::debug!(id, "not a namemc skin, trying it as a player name");
                    let resolved = resolve_player(state, &id).await?;
                    let bytes = download(state, &resolved.skin_url).await?;
                    store(
                        state,
                        &bytes,
                        &resolved.name,
                        resolved.variant,
                        Some(&format!("player:{}", resolved.name)),
                    )
                }
                Err(e) => Err(e),
            }
        }
    }
}

#[tracing::instrument(skip(state), err)]
pub fn remove(state: &AppState, id: &str) -> Result<()> {
    if let Some(record) = state.db.find_skin(id)? {
        let _ = state
            .files
            .remove_file_if_exists(state.paths.skins().join(&record.file_name));
    }
    state.db.delete_skin(id)?;
    tracing::info!("skin removed from library");
    Ok(())
}

#[tracing::instrument(skip(state), err)]
pub async fn apply_saved(state: &AppState, id: &str, variant: Option<&str>) -> Result<Appearance> {
    let record = state
        .db
        .find_skin(id)?
        .ok_or_else(|| Error::NotFound(format!("skin {id}")))?;
    let bytes = state
        .files
        .read(state.paths.skins().join(&record.file_name))?;
    let chosen = Variant::parse(variant.unwrap_or(&record.variant));
    let mut applied = upload(state, bytes, chosen).await?;
    if chosen.as_str() != record.variant {
        state.db.set_skin_variant(&record.id, chosen.as_str())?;
    }
    remember_remote(state, &record.id, &applied).await;
    remember_worn(state, &applied.uuid, &record.id);
    applied.library_id = Some(record.id);
    Ok(applied)
}

async fn remember_remote(state: &AppState, id: &str, applied: &Appearance) {
    let Some(url) = applied.skin_url.as_ref() else {
        return;
    };
    let Ok(bytes) = download(state, url).await else {
        return;
    };
    let hash = content_hash(&bytes);
    match state.db.set_skin_remote_hash(id, &hash) {
        Ok(()) => tracing::debug!(
            id,
            "linked the re-encoded texture back to the library entry"
        ),
        Err(e) => tracing::warn!(error = %e, "could not link the re-encoded texture"),
    }
}

#[tracing::instrument(skip(state), err)]
pub fn reconcile_library(state: &AppState) -> Result<usize> {
    let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut removed = 0;

    for record in state.db.list_skins()? {
        let path = state.paths.skins().join(&record.file_name);
        let Ok(bytes) = state.files.read(&path) else {
            tracing::warn!(name = %record.name, "skin file is missing, dropping the entry");
            state.db.delete_skin(&record.id)?;
            removed += 1;
            continue;
        };

        let hash = content_hash(&bytes);
        if record.hash.as_deref() != Some(hash.as_str()) {
            state.db.update_skin_hash(&record.id, &hash)?;
        }

        match seen.get(&hash) {
            Some(kept) => {
                tracing::info!(
                    name = %record.name,
                    kept = %kept,
                    "removing a duplicate of a skin already in the library"
                );
                let _ = state.files.remove_file_if_exists(&path);
                state.db.delete_skin(&record.id)?;
                removed += 1;
            }
            None => {
                seen.insert(hash, record.id);
            }
        }
    }

    state.db.enforce_unique_skin_hashes()?;
    if removed > 0 {
        tracing::info!(removed, "cleaned up duplicate skins");
    }
    Ok(removed)
}

#[tracing::instrument(skip(state), err)]
pub fn rename(state: &AppState, id: &str, name: &str) -> Result<SkinEntry> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::other("Give the skin a name."));
    }
    if name.chars().count() > 48 {
        return Err(Error::other("That name is too long."));
    }
    state.db.rename_skin(id, name)?;
    let record = state
        .db
        .find_skin(id)?
        .ok_or_else(|| Error::NotFound(format!("skin {id}")))?;
    tracing::info!(name, "skin renamed");
    entry_for(&state.files, record).ok_or_else(|| Error::other("skin file is missing"))
}

#[cfg(test)]
mod tests {
    use super::{
        content_hash, is_active, is_skin_shaped, parse_reference, reconcile_library, store,
        validate_png, SkinRef, Variant,
    };
    use crate::{db::SkinRecord, paths::Paths, state::AppState};

    const PNG_HEADER: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    #[test]
    fn parses_variants_case_insensitively() {
        assert_eq!(Variant::parse("SLIM"), Variant::Slim);
        assert_eq!(Variant::parse("slim"), Variant::Slim);
        assert_eq!(Variant::parse("alex"), Variant::Slim);
        assert_eq!(Variant::parse("CLASSIC"), Variant::Classic);
        assert_eq!(Variant::parse("anything"), Variant::Classic);
    }

    #[test]
    fn detects_active_state_regardless_of_case() {
        assert!(is_active("ACTIVE"));
        assert!(is_active("active"));
        assert!(!is_active("INACTIVE"));
    }

    #[test]
    fn parses_every_supported_reference() {
        assert_eq!(
            parse_reference("Notch").unwrap(),
            SkinRef::Name("Notch".to_string())
        );
        assert_eq!(
            parse_reference("069a79f4-44e9-4726-a5be-fca90e38aaf5").unwrap(),
            SkinRef::Uuid("069a79f444e94726a5befca90e38aaf5".to_string())
        );
        assert_eq!(
            parse_reference("069a79f444e94726a5befca90e38aaf5").unwrap(),
            SkinRef::Uuid("069a79f444e94726a5befca90e38aaf5".to_string())
        );
        assert_eq!(
            parse_reference("https://namemc.com/skin/cfa9d2e548cf3562").unwrap(),
            SkinRef::NameMc("cfa9d2e548cf3562".to_string())
        );
        assert_eq!(
            parse_reference("cfa9d2e548cf3562").unwrap(),
            SkinRef::NameMc("cfa9d2e548cf3562".to_string())
        );
        let hash = "4efd260a4694e771e2c2a60900a32c7654b7f51027ab2c874fd328d62f480c5b";
        assert_eq!(
            parse_reference(&format!("http://textures.minecraft.net/texture/{hash}")).unwrap(),
            SkinRef::Texture(format!("https://textures.minecraft.net/texture/{hash}"))
        );
        assert_eq!(
            parse_reference(hash).unwrap(),
            SkinRef::Texture(format!("https://textures.minecraft.net/texture/{hash}"))
        );
    }

    #[test]
    fn pulls_the_texture_out_of_a_give_command() {
        let command = r#"/give @p minecraft:player_head[profile={id:[I;-1529965605,-316450687,-1694901467,-112887058],properties:[{name:"textures",value:"e3RleHR1cmVzOntTS0lOOnt1cmw6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvNGVmZDI2MGE0Njk0ZTc3MWUyYzJhNjA5MDBhMzJjNzY1NGI3ZjUxMDI3YWIyYzg3NGZkMzI4ZDYyZjQ4MGM1YiJ9fX0="}]}]"#;
        assert_eq!(
            parse_reference(command).unwrap(),
            SkinRef::Texture(
                "https://textures.minecraft.net/texture/4efd260a4694e771e2c2a60900a32c7654b7f51027ab2c874fd328d62f480c5b"
                    .to_string()
            )
        );
    }

    #[test]
    fn reads_a_minecraft_int_array_uuid() {
        assert_eq!(
            super::uuid_from_int_array("id:[I;-1529965605,-316450687,-1694901467,-112887058]"),
            Some("a4ce93dbed2358819af9db25f9457aee".to_string())
        );
        assert_eq!(super::uuid_from_int_array("id:[I;1,2]"), None);
    }

    #[test]
    fn rejects_nonsense_references() {
        assert!(parse_reference("").is_err());
        assert!(parse_reference("not a valid name!").is_err());
    }

    fn png_of(colour: [u8; 4]) -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        let image = image::RgbaImage::from_pixel(64, 64, image::Rgba(colour));
        image
            .write_to(&mut buffer, image::ImageFormat::Png)
            .unwrap();
        buffer.into_inner()
    }

    fn scratch_state() -> (AppState, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("basalt-skins-{}", uuid::Uuid::new_v4()));
        let paths = Paths::plain(root.clone());
        crate::files::FileManager::new(paths.clone())
            .unwrap()
            .ensure_base_dirs()
            .unwrap();
        let files = crate::files::FileManager::new(paths).unwrap();
        let db = crate::db::Db::open(&files).unwrap();
        (AppState::new(files, db), root)
    }

    #[test]
    fn hashing_ignores_how_the_png_was_encoded() {
        let png = png_of([12, 34, 56, 255]);
        let decoded = image::load_from_memory(&png).unwrap();
        let mut reencoded = std::io::Cursor::new(Vec::new());
        decoded
            .write_to(&mut reencoded, image::ImageFormat::Png)
            .unwrap();
        let reencoded = reencoded.into_inner();
        assert_eq!(content_hash(&png), content_hash(&reencoded));
        assert_ne!(content_hash(&png), content_hash(&png_of([12, 34, 57, 255])));
    }

    #[test]
    fn saving_the_same_skin_twice_keeps_one_entry() {
        let (state, root) = scratch_state();
        let png = png_of([200, 150, 120, 255]);

        let first = store(&state, &png, "Player", Variant::Classic, Some("worn")).unwrap();
        let second = store(&state, &png, "Player", Variant::Classic, Some("worn")).unwrap();

        assert_eq!(first.id, second.id, "the same skin was stored twice");
        assert_eq!(state.db.list_skins().unwrap().len(), 1);

        let other = store(
            &state,
            &png_of([10, 20, 30, 255]),
            "Player",
            Variant::Classic,
            None,
        )
        .unwrap();
        assert_ne!(first.id, other.id);
        assert_eq!(state.db.list_skins().unwrap().len(), 2);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn reconciling_collapses_duplicates_written_before_hashing() {
        let (state, root) = scratch_state();
        let png = png_of([90, 120, 160, 255]);

        for (i, name) in ["Player", "Player (2)", "Player (3)"].iter().enumerate() {
            let file_name = format!("legacy-{i}.png");
            std::fs::write(state.paths.skins().join(&file_name), &png).unwrap();
            state
                .db
                .insert_skin(&SkinRecord {
                    id: format!("legacy-{i}"),
                    name: name.to_string(),
                    variant: "classic".to_string(),
                    file_name,
                    source: Some("worn".to_string()),
                    hash: None,
                    remote_hash: None,
                    added_at: 100 + i as i64,
                })
                .unwrap();
        }
        assert_eq!(state.db.list_skins().unwrap().len(), 3);

        let removed = reconcile_library(&state).unwrap();
        assert_eq!(removed, 2);
        let left = state.db.list_skins().unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].hash.as_deref(), Some(content_hash(&png).as_str()));

        assert_eq!(reconcile_library(&state).unwrap(), 0);
        let again = store(&state, &png, "Player", Variant::Classic, Some("worn")).unwrap();
        assert_eq!(again.id, left[0].id);
        assert_eq!(state.db.list_skins().unwrap().len(), 1);

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn rejects_non_png_and_oversized_files() {
        assert!(validate_png(b"GIF89a not a png").is_err());
        assert!(validate_png(&vec![0u8; 600 * 1024]).is_err());
        assert!(validate_png(PNG_HEADER).is_err());
    }

    #[test]
    fn only_accepts_skin_shaped_images() {
        assert!(is_skin_shaped(64, 64));
        assert!(is_skin_shaped(64, 32));
        assert!(is_skin_shaped(128, 128));
        assert!(is_skin_shaped(128, 64));
        assert!(!is_skin_shaped(397, 281));
        assert!(!is_skin_shaped(64, 48));
        assert!(!is_skin_shaped(32, 32));
    }

    #[test]
    fn refuses_a_png_that_is_not_a_skin() {
        let mut buffer = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(397, 281, image::Rgba([1, 2, 3, 255]))
            .write_to(&mut buffer, image::ImageFormat::Png)
            .unwrap();
        let error = validate_png(&buffer.into_inner()).unwrap_err().to_string();
        assert!(error.contains("397x281"), "unexpected message: {error}");
    }
}
