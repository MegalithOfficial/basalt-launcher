use std::{
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
};

use tauri::State;

use crate::{
    error::{Error, Result},
    logging::{self, LogConfig, LogRecord, LogState},
    state::AppState,
};

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
const MAX_INFLATED_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, serde::Serialize)]
pub struct InstanceLogFile {
    pub name: String,
    pub size_bytes: u64,
    pub modified_ms: i64,
    pub compressed: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct LogHit {
    pub number: usize,
    pub line: String,
    pub ranges: Vec<[usize; 2]>,
    pub level: &'static str,
}

fn rank_name(rank: u8) -> &'static str {
    match rank {
        0 => "error",
        1 => "warn",
        3 => "debug",
        _ => "info",
    }
}

#[derive(Debug, serde::Serialize)]
pub struct LogSearch {
    pub hits: Vec<LogHit>,
    pub total_lines: usize,
    pub matched_lines: usize,
    pub truncated: bool,
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn match_ranges(line: &str, needle_lower: &str) -> Vec<[usize; 2]> {
    let hay = line.to_ascii_lowercase();
    let mut ranges = Vec::new();
    let mut from = 0;
    while let Some(found) = hay[from..].find(needle_lower) {
        let start = from + found;
        let end = start + needle_lower.len();
        ranges.push([utf16_len(&line[..start]), utf16_len(&line[..end])]);
        from = end;
        if from >= hay.len() {
            break;
        }
    }
    ranges
}

fn instance_logs_dir(state: &AppState, instance_id: &str) -> Result<PathBuf> {
    let instance = super::find_instance(state, instance_id)?;
    Ok(PathBuf::from(instance.dir).join("logs"))
}

fn is_log_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".log") || lower.ends_with(".txt") || lower.ends_with(".log.gz")
}

fn tail_text(bytes: &[u8]) -> String {
    let slice = if bytes.len() as u64 > MAX_LOG_BYTES {
        let cut = bytes.len() - MAX_LOG_BYTES as usize;
        let start = bytes[cut..]
            .iter()
            .position(|b| *b == b'\n')
            .map(|i| cut + i + 1)
            .unwrap_or(cut);
        &bytes[start..]
    } else {
        bytes
    };
    String::from_utf8_lossy(slice).into_owned()
}

#[tauri::command]
#[tracing::instrument(skip(logs), err)]
pub fn get_log_records(logs: State<LogState>, limit: Option<usize>) -> Result<Vec<LogRecord>> {
    Ok(logs.buffer.snapshot(limit.unwrap_or(2000)))
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn clear_log_records(logs: State<LogState>) -> Result<()> {
    logs.buffer.clear();
    tracing::info!("log view cleared");
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_log_config(state: State<AppState>) -> Result<LogConfig> {
    Ok(logging::config(&state.files))
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn set_log_level(state: State<AppState>, level: String) -> Result<LogConfig> {
    logging::set_level(&level)?;
    let mut settings = state.db.load_settings()?;
    settings.log_level = logging::normalize_level(&level).to_string();
    state.db.save_settings(&settings)?;
    Ok(logging::config(&state.files))
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_instance_logs(
    state: State<AppState>,
    instance_id: String,
) -> Result<Vec<InstanceLogFile>> {
    let dir = instance_logs_dir(&state, &instance_id)?;
    let mut files = Vec::new();

    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(files),
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_log_name(&name) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        files.push(InstanceLogFile {
            compressed: name.to_ascii_lowercase().ends_with(".gz"),
            name,
            size_bytes: meta.len(),
            modified_ms,
        });
    }

    files.sort_by(|a, b| {
        b.modified_ms
            .cmp(&a.modified_ms)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(files)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn search_instance_log(
    state: State<AppState>,
    instance_id: String,
    name: String,
    query: String,
    min_level: Option<String>,
    limit: Option<usize>,
) -> Result<LogSearch> {
    let text = read_instance_log(state, instance_id, name)?;
    Ok(search_text(
        &text,
        &query,
        min_level.as_deref(),
        limit.unwrap_or(20_000),
    ))
}

fn has_stamp(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.len() >= 10
        && bytes[0] == b'['
        && bytes[9] == b']'
        && bytes[3] == b':'
        && bytes[6] == b':'
        && bytes[1..3].iter().all(u8::is_ascii_digit)
        && bytes[4..6].iter().all(u8::is_ascii_digit)
        && bytes[7..9].iter().all(u8::is_ascii_digit)
}

fn fault_marker(line: &str) -> bool {
    let trimmed = line.trim_start();
    (trimmed.len() < line.len() && trimmed.starts_with("at "))
        || trimmed.starts_with("Caused by:")
        || trimmed.starts_with("Suppressed:")
        || line.contains("Exception")
}

fn line_rank(line: &str, previous: u8) -> u8 {
    if has_stamp(line) {
        if line.contains("/ERROR]") || line.contains("/FATAL]") {
            return 0;
        }
        if line.contains("/WARN]") {
            return 1;
        }
        if line.contains("/DEBUG]") || line.contains("/TRACE]") {
            return 3;
        }
        return if fault_marker(line) { 0 } else { 2 };
    }

    if fault_marker(line) {
        return 0;
    }
    if line.trim().is_empty() {
        return previous;
    }
    previous
}

fn rank_ceiling(min_level: Option<&str>) -> u8 {
    match min_level.unwrap_or("all") {
        "error" => 0,
        "warn" => 1,
        "info" => 2,
        "debug" => 3,
        _ => u8::MAX,
    }
}

fn search_text(text: &str, query: &str, min_level: Option<&str>, limit: usize) -> LogSearch {
    let needle = query.trim().to_ascii_lowercase();
    let ceiling = rank_ceiling(min_level);
    let mut hits = Vec::new();
    let mut total_lines = 0;
    let mut matched_lines = 0;

    let mut previous = 2;

    for (index, line) in text.lines().enumerate() {
        total_lines += 1;
        let rank = line_rank(line, previous);
        previous = rank;
        if ceiling != u8::MAX && rank > ceiling {
            continue;
        }
        let ranges = if needle.is_empty() {
            Vec::new()
        } else {
            let found = match_ranges(line, &needle);
            if found.is_empty() {
                continue;
            }
            found
        };
        matched_lines += 1;
        if hits.len() < limit {
            hits.push(LogHit {
                number: index + 1,
                line: line.to_string(),
                ranges,
                level: rank_name(rank),
            });
        }
    }

    LogSearch {
        truncated: matched_lines > hits.len(),
        hits,
        total_lines,
        matched_lines,
    }
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_instance_log(
    state: State<AppState>,
    instance_id: String,
    name: String,
) -> Result<()> {
    let path = log_path(&state, &instance_id, &name)?;
    std::fs::remove_file(&path)?;
    tracing::info!(file = %name, "instance log deleted");
    Ok(())
}

fn log_path(state: &AppState, instance_id: &str, name: &str) -> Result<PathBuf> {
    if !is_log_name(name) || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(Error::other(format!("not a log file: {name}")));
    }

    let dir = instance_logs_dir(state, instance_id)?;
    let path = dir.join(name);
    if path.parent() != Some(dir.as_path()) {
        return Err(Error::other(format!("not a log file: {name}")));
    }
    Ok(path)
}

fn read_instance_log(state: State<AppState>, instance_id: String, name: String) -> Result<String> {
    let path = log_path(&state, &instance_id, &name)?;
    let mut file = std::fs::File::open(&path)?;

    if name.to_ascii_lowercase().ends_with(".gz") {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(file)
            .take(MAX_INFLATED_BYTES)
            .read_to_end(&mut raw)?;
        return Ok(tail_text(&raw));
    }

    let len = file.metadata()?.len();
    if len > MAX_LOG_BYTES {
        file.seek(SeekFrom::Start(len - MAX_LOG_BYTES))?;
    }
    let mut raw = Vec::new();
    file.read_to_end(&mut raw)?;
    Ok(tail_text(&raw))
}

#[tauri::command]
pub fn frontend_log(
    level: String,
    scope: String,
    message: String,
    data: Option<String>,
) -> Result<()> {
    logging::record_frontend(&level, &scope, &message, data.as_deref());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_are_case_insensitive_and_utf16_based() {
        assert_eq!(match_ranges("Loading MODS", "mods"), vec![[8, 12]]);
        assert_eq!(match_ranges("aa aa aa", "aa"), vec![[0, 2], [3, 5], [6, 8]]);
        assert_eq!(
            match_ranges("no match here", "zzz"),
            Vec::<[usize; 2]>::new()
        );
        assert_eq!(match_ranges("🙂 warn", "warn"), vec![[3, 7]]);
    }

    #[test]
    fn search_reports_totals_and_keeps_every_line_when_idle() {
        let text = "one\ntwo\nthree two\n";
        let all = search_text(text, "", None, 100);
        assert_eq!(all.hits.len(), 3);
        assert_eq!(all.total_lines, 3);
        assert!(!all.truncated);

        let hit = search_text(text, "TWO", None, 100);
        assert_eq!(hit.matched_lines, 2);
        assert_eq!(hit.hits[0].number, 2);
        assert_eq!(hit.hits[1].ranges, vec![[6, 9]]);

        let capped = search_text(text, "o", None, 1);
        assert!(capped.truncated);
        assert_eq!(capped.hits.len(), 1);
    }

    #[test]
    fn level_filter_keeps_severe_lines_only() {
        assert_eq!(line_rank("[22:38:45] [main/ERROR]: boom", 2), 0);
        assert_eq!(line_rank("[22:38:45] [main/WARN]: hmm", 0), 1);
        assert_eq!(line_rank("[22:38:45] [main/INFO]: hello", 0), 2);
        assert_eq!(line_rank("[22:38:45] [main/DEBUG]: noisy", 0), 3);
        assert_eq!(line_rank("\tat com.example.Thing.run(Thing.java:12)", 2), 0);
        assert_eq!(line_rank("Caused by: java.io.IOException", 2), 0);

        let text = "[22:38:45] [main/INFO]: one\n[22:38:45] [main/WARN]: two\n[22:38:45] [main/DEBUG]: three\n";
        let warned = search_text(text, "", Some("warn"), 100);
        assert_eq!(warned.hits.len(), 1);
        assert_eq!(warned.total_lines, 3);
        assert_eq!(warned.matched_lines, 1);

        let everything = search_text(text, "", Some("all"), 100);
        assert_eq!(everything.hits.len(), 3);
    }

    #[test]
    fn stack_traces_stay_with_the_line_that_started_them() {
        let text = concat!(
            "[21:07:03] [Render thread/ERROR]: Error while loading the narrator\n",
            "com.mojang.text2speech.Narrator$InitializeException: Failed to load flite\n",
            "\tat knot//com.mojang.text2speech.NarratorLinux.loadNative(NarratorLinux.java:81)\n",
            "libflite.so: cannot open shared object file: No such file or directory\n",
            "Native library not found in resource path (/home/user/lib.jar)\n",
            "\t... 5 more\n",
            "[21:07:04] [Render thread/INFO]: back to normal\n",
            "still part of the info message\n",
        );

        let errors = search_text(text, "", Some("error"), 100);
        assert_eq!(errors.hits.len(), 6);
        assert!(errors.hits.iter().all(|h| h.level == "error"));
        assert_eq!(
            errors.hits[3].line,
            "libflite.so: cannot open shared object file: No such file or directory"
        );

        let all = search_text(text, "", None, 100);
        assert_eq!(all.hits[6].level, "info");
        assert_eq!(all.hits[7].level, "info");
    }
}
