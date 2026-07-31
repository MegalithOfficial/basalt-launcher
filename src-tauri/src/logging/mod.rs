use std::{
    cell::Cell,
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tracing::{
    field::{Field, Visit},
    span::Attributes,
    Event, Subscriber,
};
use tracing_appender::{
    non_blocking::WorkerGuard,
    rolling::{Builder as RollingBuilder, Rotation},
};
use tracing_subscriber::{
    layer::{Context, Layer, SubscriberExt},
    registry::LookupSpan,
    reload,
    util::SubscriberInitExt,
    EnvFilter, Registry,
};

use crate::{
    error::{Error, Result},
    files::FileManager,
};

const RING_CAPACITY: usize = 5000;
const EVENT_CHANNEL_CAPACITY: usize = 1024;
const LOG_FILE_RETENTION: usize = 7;
const FILE_PREFIX: &str = "basalt";
const FILE_SUFFIX: &str = "log";
const ENV_VAR: &str = "BASALT_LOG";
pub const DEFAULT_LEVEL: &str = "info";
pub const LEVELS: [&str; 5] = ["error", "warn", "info", "debug", "trace"];

#[derive(Debug, Clone, Serialize)]
pub struct LogRecord {
    pub seq: u64,
    pub ts: i64,
    pub level: String,
    pub source: String,
    pub target: String,
    pub span: Option<String>,
    pub message: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogConfig {
    pub level: String,
    pub directory: String,
    pub file: String,
    pub env_override: Option<String>,
    pub levels: Vec<String>,
}

pub struct LogBuffer {
    records: Mutex<VecDeque<LogRecord>>,
    seq: AtomicU64,
}

impl LogBuffer {
    fn new() -> Self {
        Self {
            records: Mutex::new(VecDeque::with_capacity(512)),
            seq: AtomicU64::new(1),
        }
    }

    fn push(&self, mut record: LogRecord) -> LogRecord {
        record.seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let mut guard = self.records.lock().unwrap();
        guard.push_back(record.clone());
        while guard.len() > RING_CAPACITY {
            guard.pop_front();
        }
        record
    }

    pub fn snapshot(&self, limit: usize) -> Vec<LogRecord> {
        let guard = self.records.lock().unwrap();
        let skip = guard.len().saturating_sub(limit.max(1));
        guard.iter().skip(skip).cloned().collect()
    }

    pub fn clear(&self) {
        self.records.lock().unwrap().clear();
    }
}

pub struct LogState {
    pub buffer: Arc<LogBuffer>,
}

#[derive(Default)]
struct FieldVisitor {
    message: String,
    fields: BTreeMap<String, String>,
}

impl FieldVisitor {
    fn put(&mut self, field: &Field, value: String) {
        if field.name() == "message" {
            self.message = value;
        } else {
            self.fields.insert(field.name().to_string(), value);
        }
    }

    fn inline(&self) -> String {
        self.fields
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.put(field, format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.put(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.put(field, value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.put(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.put(field, value.to_string());
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.put(field, value.to_string());
    }
}

struct SpanFields(String);

thread_local! {
    static EMITTING: Cell<bool> = const { Cell::new(false) };
}

struct CaptureLayer {
    buffer: Arc<LogBuffer>,
    tx: Sender<LogRecord>,
}

impl<S> Layer<S> for CaptureLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &tracing::Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut visitor = FieldVisitor::default();
        attrs.record(&mut visitor);
        span.extensions_mut().insert(SpanFields(visitor.inline()));
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if EMITTING.with(Cell::get) {
            return;
        }

        let metadata = event.metadata();
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        let mut trail: Vec<String> = Vec::new();
        if let Some(scope) = ctx.event_scope(event) {
            for span in scope.from_root() {
                let extensions = span.extensions();
                match extensions.get::<SpanFields>() {
                    Some(fields) if !fields.0.is_empty() => {
                        trail.push(format!("{}{{{}}}", span.name(), fields.0))
                    }
                    _ => trail.push(span.name().to_string()),
                }
            }
        }

        let target = metadata.target().to_string();
        let source = if target == "frontend" || target.starts_with("frontend::") {
            "frontend"
        } else {
            "backend"
        };

        let record = LogRecord {
            seq: 0,
            ts: chrono::Utc::now().timestamp_millis(),
            level: metadata.level().as_str().to_lowercase(),
            source: source.to_string(),
            target,
            span: (!trail.is_empty()).then(|| trail.join(" > ")),
            message: visitor.message,
            fields: visitor.fields,
        };

        let _ = self.tx.try_send(self.buffer.push(record));
    }
}

static RELOAD: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();
static GUARD: OnceLock<WorkerGuard> = OnceLock::new();
static LEVEL: OnceLock<Mutex<String>> = OnceLock::new();

pub fn normalize_level(level: &str) -> &'static str {
    match level.trim().to_lowercase().as_str() {
        "error" => "error",
        "warn" | "warning" => "warn",
        "debug" => "debug",
        "trace" => "trace",
        _ => "info",
    }
}

fn directives(level: &str) -> String {
    format!(
        "{},hyper=warn,hyper_util=warn,h2=warn,rustls=warn,reqwest=warn,tokio_util=warn,\
         tao=warn,wry=warn,zbus=warn,tauri=info,os_info=warn,mio=warn,want=warn",
        normalize_level(level)
    )
}

fn level_slot() -> &'static Mutex<String> {
    LEVEL.get_or_init(|| Mutex::new(DEFAULT_LEVEL.to_string()))
}

pub fn current_level() -> String {
    level_slot().lock().unwrap().clone()
}

pub fn env_override() -> Option<String> {
    std::env::var(ENV_VAR).ok()
}

pub fn log_file(files: &FileManager) -> PathBuf {
    let stamp = chrono::Utc::now().format("%Y-%m-%d");
    files
        .paths()
        .logs()
        .join(format!("{FILE_PREFIX}.{stamp}.{FILE_SUFFIX}"))
}

pub fn config(files: &FileManager) -> LogConfig {
    let paths = files.paths();
    LogConfig {
        level: current_level(),
        directory: paths.logs().display().to_string(),
        file: log_file(files).display().to_string(),
        env_override: env_override(),
        levels: LEVELS.iter().map(|l| l.to_string()).collect(),
    }
}

pub fn set_level(level: &str) -> Result<()> {
    let level = normalize_level(level);
    *level_slot().lock().unwrap() = level.to_string();

    if env_override().is_some() {
        tracing::warn!(level, "log level not applied, BASALT_LOG is set");
        return Ok(());
    }

    let handle = RELOAD
        .get()
        .ok_or_else(|| Error::other("logging is not initialised"))?;
    let filter = EnvFilter::try_new(directives(level))
        .map_err(|e| Error::other(format!("invalid log filter: {e}")))?;
    handle
        .reload(filter)
        .map_err(|e| Error::other(format!("could not apply log level: {e}")))?;
    tracing::info!(level, "log level changed");
    Ok(())
}

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".to_string());
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "non-string panic payload".to_string());
        tracing::error!(target: "panic", location, "panic: {payload}");
        previous(info);
    }));
}

fn spawn_bridge(app: AppHandle, mut rx: Receiver<LogRecord>) {
    tauri::async_runtime::spawn(async move {
        let mut batch: Vec<LogRecord> = Vec::with_capacity(64);
        while let Some(first) = rx.recv().await {
            batch.push(first);
            while batch.len() < 512 {
                match rx.try_recv() {
                    Ok(record) => batch.push(record),
                    Err(_) => break,
                }
            }
            EMITTING.with(|flag| flag.set(true));
            let _ = app.emit("log:record", &batch);
            EMITTING.with(|flag| flag.set(false));
            batch.clear();
            tokio::time::sleep(Duration::from_millis(120)).await;
        }
    });
}

pub fn init(app: &AppHandle, files: &FileManager, level: &str) -> Result<LogState> {
    let directory = files.paths().logs();
    files.ensure_dir(&directory)?;

    let appender = RollingBuilder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix(FILE_PREFIX)
        .filename_suffix(FILE_SUFFIX)
        .max_log_files(LOG_FILE_RETENTION)
        .build(&directory)
        .map_err(|e| Error::other(format!("could not open log file: {e}")))?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let _ = GUARD.set(guard);

    let level = normalize_level(level);
    *level_slot().lock().unwrap() = level.to_string();

    let requested = env_override().unwrap_or_else(|| directives(level));
    let filter = EnvFilter::try_new(&requested)
        .unwrap_or_else(|_| EnvFilter::new(directives(DEFAULT_LEVEL)));
    let (filter_layer, handle) = reload::Layer::new(filter);
    let _ = RELOAD.set(handle);

    let buffer = Arc::new(LogBuffer::new());
    let (tx, rx) = channel(EVENT_CHANNEL_CAPACITY);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_writer(writer);
    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_writer(std::io::stderr);
    let capture_layer = CaptureLayer {
        buffer: buffer.clone(),
        tx,
    };

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(file_layer)
        .with(console_layer)
        .with(capture_layer)
        .init();

    spawn_bridge(app.clone(), rx);
    install_panic_hook();

    Ok(LogState { buffer })
}

macro_rules! frontend_event {
    ($level:ident, $scope:expr, $message:expr, $data:expr) => {
        match $data {
            Some(data) => {
                tracing::$level!(target: "frontend", scope = $scope, data = data, "{}", $message)
            }
            None => tracing::$level!(target: "frontend", scope = $scope, "{}", $message),
        }
    };
}

pub fn record_frontend(level: &str, scope: &str, message: &str, data: Option<&str>) {
    match normalize_level(level) {
        "error" => frontend_event!(error, scope, message, data),
        "warn" => frontend_event!(warn, scope, message, data),
        "debug" => frontend_event!(debug, scope, message, data),
        "trace" => frontend_event!(trace, scope, message, data),
        _ => frontend_event!(info, scope, message, data),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{directives, normalize_level, LogBuffer, LogRecord};

    fn record(message: &str) -> LogRecord {
        LogRecord {
            seq: 0,
            ts: 0,
            level: "info".to_string(),
            source: "backend".to_string(),
            target: "test".to_string(),
            span: None,
            message: message.to_string(),
            fields: BTreeMap::new(),
        }
    }

    #[test]
    fn normalizes_unknown_levels_to_info() {
        assert_eq!(normalize_level("WARNING"), "warn");
        assert_eq!(normalize_level("Trace"), "trace");
        assert_eq!(normalize_level("nonsense"), "info");
    }

    #[test]
    fn directives_pin_noisy_dependencies() {
        let filter = directives("debug");
        assert!(filter.starts_with("debug,"));
        assert!(filter.contains("hyper=warn"));
    }

    #[test]
    fn buffer_assigns_sequence_and_drops_oldest() {
        let buffer = LogBuffer::new();
        for i in 0..super::RING_CAPACITY + 10 {
            buffer.push(record(&i.to_string()));
        }
        let snapshot = buffer.snapshot(super::RING_CAPACITY);
        assert_eq!(snapshot.len(), super::RING_CAPACITY);
        assert_eq!(snapshot[0].message, "10");
        assert!(snapshot[0].seq < snapshot[1].seq);
    }

    #[test]
    fn snapshot_limits_to_the_newest_records() {
        let buffer = LogBuffer::new();
        for i in 0..50 {
            buffer.push(record(&i.to_string()));
        }
        let snapshot = buffer.snapshot(5);
        assert_eq!(snapshot.len(), 5);
        assert_eq!(snapshot[4].message, "49");
    }
}
