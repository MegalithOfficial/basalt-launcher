use std::collections::{HashMap, HashSet};

use serde::Serialize;
use tauri::AppHandle;

use super::{download_url, fetch_version, model::*, resolve_projects};
use crate::{
    content,
    db::ContentFile,
    download::{self, DownloadSpec},
    error::Result,
    state::AppState,
    tasks::{TaskKind, TaskSpec},
};

const MAX_DEPTH: u8 = 5;

#[derive(Debug, Clone, Serialize)]
pub struct InstalledItem {
    pub file_name: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub is_dependency: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedFile {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub file_name: String,
    pub version_name: String,
    pub url: String,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub size: Option<u64>,
    pub is_dependency: bool,
    pub replaces: Option<String>,
    pub dependencies: Vec<VersionDependency>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkippedProject {
    pub project_id: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    pub project_id: String,
    pub title: String,
    pub file_name: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct InstallPlan {
    pub primary: Option<PlannedFile>,
    pub dependencies: Vec<PlannedFile>,
    pub already_present: Vec<ProjectSummary>,
    pub skipped: Vec<SkippedProject>,
    pub conflicts: Vec<Conflict>,
    pub total_bytes: u64,
}

impl InstallPlan {
    pub fn files(&self) -> Vec<&PlannedFile> {
        self.primary
            .iter()
            .chain(self.dependencies.iter())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.primary.is_none() && self.dependencies.is_empty()
    }
}

fn is_trailing_noise(part: &str) -> bool {
    if part.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }
    if let Some(rest) = part.strip_prefix("mc") {
        if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    matches!(
        part,
        "fabric" | "forge" | "neoforge" | "quilt" | "mc" | "universal" | "release"
    )
}

pub fn normalize_stem(file_name: &str) -> String {
    let base = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name)
        .to_lowercase();

    let mut parts: Vec<&str> = base
        .split(['-', '_', '+'])
        .filter(|p| !p.is_empty())
        .collect();
    while parts.len() > 1 && is_trailing_noise(parts[parts.len() - 1]) {
        parts.pop();
    }

    if parts.is_empty() {
        base
    } else {
        parts.join("-")
    }
}

struct InstalledIndex {
    by_project: HashMap<String, ContentFile>,
    mod_ids: HashSet<String>,
    stems: HashSet<String>,
}

fn index_installed(state: &AppState, instance_id: &str, kind: ContentKind) -> InstalledIndex {
    let files = state
        .db
        .content_files(instance_id, kind.as_str())
        .unwrap_or_default();

    let mut by_project = HashMap::new();
    let mut mod_ids = HashSet::new();
    let mut stems = HashSet::new();

    for file in files {
        if let Some(project_id) = &file.project_id {
            by_project.insert(project_id.clone(), file.clone());
        }
        if let Some(mod_id) = &file.mod_id {
            mod_ids.insert(mod_id.to_lowercase());
        }
        stems.insert(normalize_stem(&file.file_name));
    }

    InstalledIndex {
        by_project,
        mod_ids,
        stems,
    }
}

fn planned_from(
    version: &ProjectVersion,
    summary: Option<&ProjectSummary>,
    project_id: &str,
    is_dependency: bool,
    replaces: Option<String>,
) -> Result<PlannedFile> {
    let (url, file) = download_url(version)?;
    Ok(PlannedFile {
        project_id: project_id.to_string(),
        version_id: version.id.clone(),
        title: summary
            .map(|s| s.title.clone())
            .unwrap_or_else(|| version.name.clone()),
        icon_url: summary.and_then(|s| s.icon_url.clone()),
        file_name: file.file_name.clone(),
        version_name: version.name.clone(),
        url,
        sha1: file.sha1.clone(),
        sha512: file.sha512.clone(),
        size: file.size,
        is_dependency,
        replaces,
        dependencies: version.dependencies.clone(),
    })
}

pub fn rollback_written(files: &crate::files::FileManager, task: &crate::tasks::TaskHandle) {
    let paths = std::mem::take(&mut *task.written().lock().unwrap());
    for path in paths {
        let _ = files.remove_file_if_exists(path);
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn plan(
    state: &AppState,
    provider: Provider,
    project_id: &str,
    instance_id: &str,
    kind: ContentKind,
    game_version: &str,
    loader: Option<&str>,
    version_id: Option<&str>,
    with_dependencies: bool,
) -> Result<InstallPlan> {
    let installed = index_installed(state, instance_id, kind);
    let mut plan = InstallPlan::default();

    let version = fetch_version(
        state,
        provider,
        project_id,
        kind,
        game_version,
        loader,
        version_id,
    )
    .await?;

    let summaries = resolve_projects(state, provider, &[project_id.to_string()])
        .await
        .unwrap_or_default();
    let summary = summaries.first();

    let replaces = installed
        .by_project
        .get(project_id)
        .map(|f| f.file_name.clone())
        .filter(|name| name != &version.file_name);

    plan.primary = Some(planned_from(
        &version, summary, project_id, false, replaces,
    )?);

    if !with_dependencies || !kind.uses_loaders() {
        plan.total_bytes = plan.files().iter().filter_map(|f| f.size).sum();
        return Ok(plan);
    }

    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(project_id.to_string());

    let mut planned_stems: HashSet<String> = HashSet::new();
    planned_stems.insert(normalize_stem(&version.file_name));

    let mut queue: Vec<(VersionDependency, u8)> = version
        .dependencies
        .iter()
        .cloned()
        .map(|d| (d, 1u8))
        .collect();

    while let Some((dependency, depth)) = queue.pop() {
        if depth > MAX_DEPTH {
            continue;
        }

        if dependency.dependency_type == "incompatible" {
            if let Some(existing) = installed.by_project.get(&dependency.project_id) {
                let title = existing
                    .title
                    .clone()
                    .unwrap_or_else(|| existing.file_name.clone());
                plan.conflicts.push(Conflict {
                    project_id: dependency.project_id.clone(),
                    title,
                    file_name: Some(existing.file_name.clone()),
                    reason: "Declared incompatible with this project".to_string(),
                });
            }
            continue;
        }

        if dependency.dependency_type != "required" {
            continue;
        }
        if !visited.insert(dependency.project_id.clone()) {
            continue;
        }

        let info = resolve_projects(state, provider, &[dependency.project_id.clone()])
            .await
            .ok()
            .and_then(|mut list| list.pop());

        if let Some(existing) = installed.by_project.get(&dependency.project_id) {
            if let Some(summary) = info.clone() {
                plan.already_present.push(summary);
            } else {
                let _ = existing;
            }
            continue;
        }

        let title = info
            .as_ref()
            .map(|i| i.title.clone())
            .unwrap_or_else(|| dependency.project_id.clone());

        let dependency_version = fetch_version(
            state,
            provider,
            &dependency.project_id,
            kind,
            game_version,
            loader,
            dependency.version_id.as_deref(),
        )
        .await;

        let dependency_version = match dependency_version {
            Ok(v) => v,
            Err(e) => {
                plan.skipped.push(SkippedProject {
                    project_id: dependency.project_id.clone(),
                    title,
                    icon_url: info.and_then(|i| i.icon_url),
                    reason: e.to_string(),
                });
                continue;
            }
        };

        let stem = normalize_stem(&dependency_version.file_name);
        if installed.stems.contains(&stem) || !planned_stems.insert(stem) {
            if let Some(summary) = info {
                plan.already_present.push(summary);
            }
            continue;
        }

        match planned_from(
            &dependency_version,
            info.as_ref(),
            &dependency.project_id,
            true,
            None,
        ) {
            Ok(file) => {
                queue.extend(
                    dependency_version
                        .dependencies
                        .iter()
                        .cloned()
                        .map(|d| (d, depth + 1)),
                );
                plan.dependencies.push(file);
            }
            Err(e) => plan.skipped.push(SkippedProject {
                project_id: dependency.project_id.clone(),
                title,
                icon_url: info.and_then(|i| i.icon_url),
                reason: e.to_string(),
            }),
        }
    }

    for mod_id in &installed.mod_ids {
        let _ = mod_id;
    }

    plan.total_bytes = plan.files().iter().filter_map(|f| f.size).sum();
    Ok(plan)
}

pub async fn apply(
    app: Option<&AppHandle>,
    state: &AppState,
    plan: &InstallPlan,
    provider: Provider,
    instance_id: &str,
    kind: ContentKind,
    pack_version_id: Option<&str>,
) -> Result<Vec<InstalledItem>> {
    if plan.is_empty() {
        return Ok(Vec::new());
    }

    let dir = content::dir_for(&state.paths, instance_id, kind.as_str())?;
    state.files.ensure_dir(&dir)?;

    let files = plan.files();
    let total = files.len();
    let specs: Vec<DownloadSpec> = files
        .iter()
        .map(|f| DownloadSpec {
            url: f.url.clone(),
            dest: dir.join(&f.file_name),
            sha1: f.sha1.clone(),
            size: f.size,
        })
        .collect();

    let task = app.map(|app| {
        let instance_name = state
            .db
            .list_instances(&state.files)
            .ok()
            .and_then(|list| list.into_iter().find(|i| i.id == instance_id))
            .map(|i| i.name);
        state.tasks.start(
            app,
            match pack_version_id {
                Some(_) => TaskKind::ModpackInstall,
                None => TaskKind::ContentInstall,
            },
            TaskSpec {
                title: plan
                    .primary
                    .as_ref()
                    .map(|f| f.title.clone())
                    .unwrap_or_else(|| "Content".to_string()),
                subtitle: instance_name.map(|name| format!("into {name}")),
                icon_url: plan.primary.as_ref().and_then(|f| f.icon_url.clone()),
                instance_id: Some(instance_id.to_string()),
                project_id: plan.primary.as_ref().map(|f| f.project_id.clone()),
                total: total as u64,
                total_bytes: plan.total_bytes,
            },
        )
    });

    if let Some(task) = &task {
        task.stage("downloading");
    }

    let retry_hook = task
        .as_ref()
        .map(|t| move |attempt: u32, max: u32, reason: &str| t.note_retry(attempt, max, reason));

    let concurrency = state.db.load_settings()?.concurrent_downloads;
    let outcome = {
        let task_ref = task.as_ref();
        download::download_many_cancellable(
            &state.network,
            &state.files,
            specs,
            concurrency,
            move |progress| {
                if let Some(task) = task_ref {
                    task.progress(
                        progress.completed as u64,
                        progress.total as u64,
                        progress.downloaded_bytes,
                        progress.total_bytes,
                    );
                }
            },
            task.as_ref().map(|t| t.token()),
            task.as_ref().map(|t| t.written()),
            retry_hook
                .as_ref()
                .map(|h| h as &(dyn Fn(u32, u32, &str) + Send + Sync)),
        )
        .await
    };

    if let Err(e) = outcome {
        if let Some(task) = &task {
            rollback_written(&state.files, task);
            match &e {
                crate::error::Error::Cancelled => task.cancelled(),
                other => task.fail(other),
            }
        }
        return Err(e);
    }

    if let Some(task) = &task {
        task.stage("recording");
    }

    let now = chrono::Utc::now().timestamp();
    let mut written = Vec::with_capacity(total);

    for file in files {
        if let Some(old) = &file.replaces {
            let _ = content::delete(&state.files, instance_id, kind.as_str(), old);
            let _ = state
                .db
                .delete_content_file(instance_id, kind.as_str(), old);
        }

        let record = ContentFile {
            file_name: file.file_name.clone(),
            sha1: file.sha1.clone(),
            sha512: file.sha512.clone(),
            murmur2: None,
            provider: Some(provider.as_str().to_string()),
            project_id: Some(file.project_id.clone()),
            version_id: Some(file.version_id.clone()),
            title: Some(file.title.clone()),
            icon_url: file.icon_url.clone(),
            mod_id: None,
            mod_version: None,
            dependencies: serde_json::to_string(&file.dependencies).ok(),
            origin: if pack_version_id.is_some() {
                "pack".to_string()
            } else if file.is_dependency {
                "dependency".to_string()
            } else {
                "user".to_string()
            },
            pack_version_id: pack_version_id.map(str::to_owned),
            installed_at: now,
        };
        let _ = state
            .db
            .record_content_file(instance_id, kind.as_str(), &record);
        written.push(InstalledItem {
            file_name: file.file_name.clone(),
            title: file.title.clone(),
            icon_url: file.icon_url.clone(),
            is_dependency: file.is_dependency,
        });
    }

    if let Some(task) = &task {
        task.succeed();
    }

    Ok(written)
}

#[derive(Debug, Clone, Serialize)]
pub struct OrphanFile {
    pub file_name: String,
    pub title: String,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemovalPlan {
    pub dependents: Vec<String>,
    pub from_pack: bool,
    pub orphans: Vec<OrphanFile>,
}

fn required_project_ids(file: &crate::db::ContentFile) -> Vec<String> {
    file.dependencies
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Vec<VersionDependency>>(raw).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d.dependency_type == "required")
        .map(|d| d.project_id)
        .collect()
}

fn requires(file: &crate::db::ContentFile, project_id: &str) -> bool {
    required_project_ids(file).iter().any(|id| id == project_id)
}

pub fn plan_removal(
    state: &AppState,
    instance_id: &str,
    kind: ContentKind,
    file_name: &str,
) -> RemovalPlan {
    let files = state
        .db
        .content_files(instance_id, kind.as_str())
        .unwrap_or_default();

    let Some(target) = files.iter().find(|f| f.file_name == file_name).cloned() else {
        return RemovalPlan {
            dependents: Vec::new(),
            from_pack: false,
            orphans: Vec::new(),
        };
    };

    let dependents = target
        .project_id
        .as_deref()
        .map(|project_id| {
            files
                .iter()
                .filter(|f| {
                    f.file_name != file_name
                        && f.project_id.as_deref() != Some(project_id)
                        && requires(f, project_id)
                })
                .map(|f| f.title.clone().unwrap_or_else(|| f.file_name.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let from_pack = target.origin == "pack";
    let mut removing: std::collections::HashSet<String> =
        std::collections::HashSet::from([file_name.to_string()]);
    let mut orphans = Vec::new();
    let mut queue = vec![target];

    while let Some(current) = queue.pop() {
        for project_id in required_project_ids(&current) {
            let Some(candidate) = files.iter().find(|f| {
                f.origin == "dependency"
                    && f.project_id.as_deref() == Some(project_id.as_str())
                    && !removing.contains(&f.file_name)
            }) else {
                continue;
            };

            let still_needed = files.iter().any(|f| {
                f.file_name != candidate.file_name
                    && !removing.contains(&f.file_name)
                    && requires(f, &project_id)
            });
            if still_needed {
                continue;
            }

            removing.insert(candidate.file_name.clone());
            orphans.push(OrphanFile {
                file_name: candidate.file_name.clone(),
                title: candidate
                    .title
                    .clone()
                    .unwrap_or_else(|| candidate.file_name.clone()),
                icon_url: candidate.icon_url.clone(),
            });
            queue.push(candidate.clone());
        }
    }

    RemovalPlan {
        dependents,
        from_pack,
        orphans,
    }
}

pub fn dependents_of(
    state: &AppState,
    instance_id: &str,
    kind: ContentKind,
    project_id: &str,
) -> Vec<String> {
    let files = state
        .db
        .content_files(instance_id, kind.as_str())
        .unwrap_or_default();

    files
        .into_iter()
        .filter(|file| {
            file.project_id.as_deref() != Some(project_id)
                && file
                    .dependencies
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<Vec<VersionDependency>>(raw).ok())
                    .is_some_and(|deps| {
                        deps.iter()
                            .any(|d| d.dependency_type == "required" && d.project_id == project_id)
                    })
        })
        .map(|file| file.title.unwrap_or(file.file_name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::normalize_stem;

    #[test]
    fn stems_ignore_version_and_loader_suffixes() {
        assert_eq!(normalize_stem("fabric-api-0.92.2+1.20.1.jar"), "fabric-api");
        assert_eq!(normalize_stem("sodium-fabric-0.5.8.jar"), "sodium");
        assert_eq!(normalize_stem("jei-1.20.1-forge-15.3.0.4.jar"), "jei");
        assert_eq!(normalize_stem("Iris-1.7.0.jar"), "iris");
    }

    #[test]
    fn mc_prefixed_version_tags_are_stripped() {
        assert_eq!(
            normalize_stem("sodium-fabric-0.8.12+mc1.21.1.jar"),
            "sodium"
        );
        assert_eq!(
            normalize_stem("lithium-fabric-mc1.21.1-0.13.1.jar"),
            "lithium"
        );
        assert_eq!(normalize_stem("mcmod-helper-1.0.jar"), "mcmod-helper");
    }

    #[test]
    fn different_mods_keep_different_stems() {
        assert_ne!(
            normalize_stem("sodium-0.5.8.jar"),
            normalize_stem("lithium-0.11.2.jar")
        );
    }

    #[test]
    fn versionless_names_survive() {
        assert_eq!(normalize_stem("cloth-config.jar"), "cloth-config");
        assert_eq!(normalize_stem("1.20.1.jar"), "1.20.1");
    }

    #[test]
    fn leading_loader_words_are_kept() {
        assert_eq!(normalize_stem("fabric-api-0.92.2.jar"), "fabric-api");
        assert_eq!(
            normalize_stem("forge-config-api-port-9.0.jar"),
            "forge-config-api-port"
        );
    }
}
