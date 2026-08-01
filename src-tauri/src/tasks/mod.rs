use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

const EMIT_INTERVAL: Duration = Duration::from_millis(100);
const MAX_FINISHED: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    GameInstall,
    JavaInstall,
    LoaderInstall,
    ModpackInstall,
    ContentInstall,
    ContentUpdate,
    WorldImport,
    InstanceImport,
    AppUpdate,
    InstanceRepair,
    InstanceDuplicate,
}

impl TaskKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GameInstall => "game_install",
            Self::JavaInstall => "java_install",
            Self::LoaderInstall => "loader_install",
            Self::ModpackInstall => "modpack_install",
            Self::ContentInstall => "content_install",
            Self::ContentUpdate => "content_update",
            Self::WorldImport => "world_import",
            Self::InstanceImport => "instance_import",
            Self::AppUpdate => "app_update",
            Self::InstanceRepair => "instance_repair",
            Self::InstanceDuplicate => "instance_duplicate",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "game_install" | "GameInstall" => Some(Self::GameInstall),
            "java_install" | "JavaInstall" => Some(Self::JavaInstall),
            "loader_install" | "LoaderInstall" => Some(Self::LoaderInstall),
            "modpack_install" | "ModpackInstall" => Some(Self::ModpackInstall),
            "content_install" | "ContentInstall" => Some(Self::ContentInstall),
            "content_update" | "ContentUpdate" => Some(Self::ContentUpdate),
            "world_import" | "WorldImport" => Some(Self::WorldImport),
            "instance_import" | "InstanceImport" => Some(Self::InstanceImport),
            "app_update" | "AppUpdate" => Some(Self::AppUpdate),
            "instance_repair" | "InstanceRepair" => Some(Self::InstanceRepair),
            "instance_duplicate" | "InstanceDuplicate" => Some(Self::InstanceDuplicate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl TaskState {
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            TaskState::Succeeded | TaskState::Failed | TaskState::Cancelled
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: String,
    pub kind: TaskKind,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon_url: Option<String>,
    pub instance_id: Option<String>,
    pub project_id: Option<String>,
    pub state: TaskState,
    pub stage: String,
    pub completed: u64,
    pub total: u64,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub error: Option<String>,
    pub retries: u64,
    pub retry_note: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct TaskSpec {
    pub title: String,
    pub subtitle: Option<String>,
    pub icon_url: Option<String>,
    pub instance_id: Option<String>,
    pub project_id: Option<String>,
    pub total: u64,
    pub total_bytes: u64,
}

fn is_recoverable(kind: TaskKind) -> bool {
    matches!(
        kind,
        TaskKind::ModpackInstall
            | TaskKind::ContentInstall
            | TaskKind::ContentUpdate
            | TaskKind::WorldImport
    )
}

pub struct Tasks {
    inner: Mutex<Vec<Task>>,
    tokens: Mutex<HashMap<String, CancellationToken>>,
    db: crate::db::Db,
}

impl Tasks {
    pub fn new(db: crate::db::Db) -> Self {
        Self {
            inner: Mutex::new(Vec::new()),
            tokens: Mutex::new(HashMap::new()),
            db,
        }
    }

    pub fn cancel(&self, id: &str) -> bool {
        let token = self.tokens.lock().unwrap().get(id).cloned();
        match token {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    pub fn list(&self) -> Vec<Task> {
        self.inner.lock().unwrap().clone()
    }

    pub fn has_active(&self, instance_id: &str, kind: TaskKind) -> bool {
        self.inner.lock().unwrap().iter().any(|task| {
            task.instance_id.as_deref() == Some(instance_id)
                && task.kind == kind
                && task.state == TaskState::Running
        })
    }

    pub fn clear_finished(&self) {
        self.inner
            .lock()
            .unwrap()
            .retain(|t| !t.state.is_finished());
    }

    fn prune(list: &mut Vec<Task>) {
        let finished = list.iter().filter(|t| t.state.is_finished()).count();
        if finished <= MAX_FINISHED {
            return;
        }
        let mut excess = finished - MAX_FINISHED;
        list.retain(|t| {
            if excess > 0 && t.state.is_finished() {
                excess -= 1;
                false
            } else {
                true
            }
        });
    }

    pub fn start(self: &Arc<Self>, app: &AppHandle, kind: TaskKind, spec: TaskSpec) -> TaskHandle {
        let task = Task {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            title: spec.title,
            subtitle: spec.subtitle,
            icon_url: spec.icon_url,
            instance_id: spec.instance_id,
            project_id: spec.project_id,
            state: TaskState::Running,
            stage: "preparing".to_string(),
            completed: 0,
            total: spec.total,
            downloaded_bytes: 0,
            total_bytes: spec.total_bytes,
            error: None,
            retries: 0,
            retry_note: None,
            started_at: chrono::Utc::now().timestamp(),
            finished_at: None,
        };

        let id = task.id.clone();
        {
            let mut list = self.inner.lock().unwrap();
            list.push(task.clone());
            Self::prune(&mut list);
        }
        let token = CancellationToken::new();
        self.tokens
            .lock()
            .unwrap()
            .insert(id.clone(), token.clone());

        if is_recoverable(kind) {
            let _ = self.db.begin_operation(&crate::db::PendingOperation {
                id: id.clone(),
                kind,
                instance_id: task.instance_id.clone(),
                title: task.title.clone(),
                payload: None,
                started_at: task.started_at,
            });
        }

        emit(app, &task);

        TaskHandle {
            id,
            app: app.clone(),
            tasks: Arc::clone(self),
            last_emit: Mutex::new(Instant::now()),
            token,
            written: Mutex::new(Vec::new()),
        }
    }

    fn mutate<F>(&self, id: &str, apply: F) -> Option<Task>
    where
        F: FnOnce(&mut Task),
    {
        let mut list = self.inner.lock().unwrap();
        let task = list.iter_mut().find(|t| t.id == id)?;
        if task.state.is_finished() {
            return None;
        }
        apply(task);
        Some(task.clone())
    }
}

fn emit(app: &AppHandle, task: &Task) {
    let _ = app.emit("task:update", task);
}

pub struct TaskHandle {
    id: String,
    app: AppHandle,
    tasks: Arc<Tasks>,
    last_emit: Mutex<Instant>,
    token: CancellationToken,
    written: Mutex<Vec<PathBuf>>,
}

impl TaskHandle {
    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    pub fn written(&self) -> &Mutex<Vec<PathBuf>> {
        &self.written
    }

    fn should_emit(&self) -> bool {
        let mut last = self.last_emit.lock().unwrap();
        if last.elapsed() >= EMIT_INTERVAL {
            *last = Instant::now();
            true
        } else {
            false
        }
    }

    fn force_emit(&self) {
        *self.last_emit.lock().unwrap() = Instant::now();
    }

    pub fn stage(&self, stage: &str) {
        if let Some(task) = self.tasks.mutate(&self.id, |t| {
            t.stage = stage.to_string();
        }) {
            self.force_emit();
            emit(&self.app, &task);
        }
    }

    pub fn set_total(&self, total: u64, total_bytes: u64) {
        if let Some(task) = self.tasks.mutate(&self.id, |t| {
            t.total = total;
            t.total_bytes = total_bytes;
        }) {
            self.force_emit();
            emit(&self.app, &task);
        }
    }

    pub fn progress(&self, completed: u64, total: u64, downloaded_bytes: u64, total_bytes: u64) {
        let mut cleared_retry = false;
        let updated = self.tasks.mutate(&self.id, |t| {
            t.completed = completed;
            t.total = total;
            t.downloaded_bytes = downloaded_bytes;
            t.total_bytes = total_bytes;
            if t.retry_note.is_some() {
                t.retry_note = None;
                cleared_retry = true;
            }
        });
        if let Some(task) = updated {
            if cleared_retry {
                self.force_emit();
                emit(&self.app, &task);
            } else if self.should_emit() {
                emit(&self.app, &task);
            }
        }
    }

    pub fn note_retry(&self, attempt: u32, max: u32, reason: &str) {
        if let Some(task) = self.tasks.mutate(&self.id, |t| {
            t.retries += 1;
            t.retry_note = Some(format!("Retrying {attempt} of {max}: {reason}"));
        }) {
            self.force_emit();
            emit(&self.app, &task);
        }
    }

    fn settle(&self, state: TaskState, error: Option<String>) {
        let mut list = self.tasks.inner.lock().unwrap();
        let Some(task) = list.iter_mut().find(|t| t.id == self.id) else {
            return;
        };
        if task.state.is_finished() {
            return;
        }
        task.state = state;
        task.error = error;
        task.finished_at = Some(chrono::Utc::now().timestamp());
        if state == TaskState::Succeeded && task.total > 0 {
            task.completed = task.total;
            task.downloaded_bytes = task.total_bytes;
        }
        task.stage = match state {
            TaskState::Succeeded => "done".to_string(),
            TaskState::Failed => "failed".to_string(),
            TaskState::Cancelled => "cancelled".to_string(),
            _ => task.stage.clone(),
        };
        let snapshot = task.clone();
        drop(list);
        self.tasks.tokens.lock().unwrap().remove(&self.id);
        let _ = self.tasks.db.end_operation(&self.id);
        emit(&self.app, &snapshot);
    }

    pub fn succeed(&self) {
        self.settle(TaskState::Succeeded, None);
    }

    pub fn cancelled(&self) {
        self.settle(TaskState::Cancelled, None);
    }

    pub fn fail(&self, error: impl std::fmt::Display) {
        self.settle(TaskState::Failed, Some(error.to_string()));
    }

    pub fn finish<T>(&self, result: &crate::error::Result<T>) {
        match result {
            Ok(_) => self.succeed(),
            Err(crate::error::Error::Cancelled) => self.cancelled(),
            Err(e) => self.fail(e),
        }
    }
}

impl Drop for TaskHandle {
    fn drop(&mut self) {
        self.tasks.tokens.lock().unwrap().remove(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> crate::db::Db {
        crate::db::Db::open_in_memory().unwrap()
    }

    fn task(id: &str, state: TaskState) -> Task {
        Task {
            id: id.into(),
            kind: TaskKind::ContentInstall,
            title: id.into(),
            subtitle: None,
            icon_url: None,
            instance_id: None,
            project_id: None,
            state,
            stage: "x".into(),
            completed: 0,
            total: 0,
            downloaded_bytes: 0,
            total_bytes: 0,
            error: None,
            retries: 0,
            retry_note: None,
            started_at: 0,
            finished_at: None,
        }
    }

    #[test]
    fn finished_states_are_terminal() {
        assert!(TaskState::Succeeded.is_finished());
        assert!(TaskState::Failed.is_finished());
        assert!(TaskState::Cancelled.is_finished());
        assert!(!TaskState::Running.is_finished());
    }

    #[test]
    fn task_kind_parses_current_and_legacy_names() {
        assert_eq!(
            TaskKind::parse("modpack_install"),
            Some(TaskKind::ModpackInstall)
        );
        assert_eq!(
            TaskKind::parse("ModpackInstall"),
            Some(TaskKind::ModpackInstall)
        );
        assert_eq!(
            TaskKind::parse("instance_repair"),
            Some(TaskKind::InstanceRepair)
        );
        assert_eq!(
            TaskKind::parse("InstanceDuplicate"),
            Some(TaskKind::InstanceDuplicate)
        );
        assert_eq!(TaskKind::parse("unknown"), None);
    }

    #[test]
    fn clear_finished_keeps_running_tasks() {
        let tasks = Tasks::new(test_db());
        {
            let mut list = tasks.inner.lock().unwrap();
            list.push(task("a", TaskState::Succeeded));
            list.push(task("b", TaskState::Running));
            list.push(task("c", TaskState::Failed));
        }
        tasks.clear_finished();
        let ids: Vec<String> = tasks.list().into_iter().map(|t| t.id).collect();
        assert_eq!(ids, vec!["b"]);
    }

    #[test]
    fn prune_drops_oldest_finished_beyond_the_cap() {
        let mut list: Vec<Task> = (0..MAX_FINISHED + 5)
            .map(|i| task(&i.to_string(), TaskState::Succeeded))
            .collect();
        list.push(task("live", TaskState::Running));
        Tasks::prune(&mut list);

        assert_eq!(list.len(), MAX_FINISHED + 1);
        assert_eq!(list[0].id, "5");
        assert!(list.iter().any(|t| t.id == "live"));
    }

    #[test]
    fn mutate_ignores_finished_tasks() {
        let tasks = Tasks::new(test_db());
        {
            let mut list = tasks.inner.lock().unwrap();
            list.push(task("done", TaskState::Succeeded));
        }
        let result = tasks.mutate("done", |t| t.stage = "changed".into());
        assert!(result.is_none());
        assert_eq!(tasks.list()[0].stage, "x");
    }
}
