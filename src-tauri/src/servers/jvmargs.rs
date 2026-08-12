use std::path::Path;

use crate::{config::MemoryLimits, error::Result, files::FileManager};

pub const FILE: &str = "user_jvm_args.txt";

pub fn read(files: &FileManager, dir: &Path) -> Option<String> {
    let bytes = files.read(dir.join(FILE)).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn declared_memory(text: &str) -> (Option<String>, Option<String>) {
    let mut min = None;
    let mut max = None;
    for line in text.lines() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        for token in line.split_whitespace() {
            if let Some(value) = token.strip_prefix("-Xms") {
                min = Some(value.to_string());
            } else if let Some(value) = token.strip_prefix("-Xmx") {
                max = Some(value.to_string());
            }
        }
    }
    (min, max)
}

pub fn with_memory(text: &str, memory: MemoryLimits) -> String {
    let mut lines: Vec<String> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            lines.push(line.to_string());
            continue;
        }
        let kept = trimmed
            .split_whitespace()
            .filter(|token| !token.starts_with("-Xms") && !token.starts_with("-Xmx"))
            .collect::<Vec<_>>()
            .join(" ");
        if !kept.is_empty() {
            lines.push(kept);
        }
    }

    lines.push(format!("-Xms{}M -Xmx{}M", memory.min_mb, memory.max_mb));
    let mut rendered = lines.join("\n");
    rendered.push('\n');
    rendered
}

pub fn apply(files: &FileManager, dir: &Path, memory: MemoryLimits) -> Result<()> {
    let existing = read(files, dir)
        .unwrap_or_else(|| "# Basalt keeps the memory this server starts with here.\n".to_string());
    files.write_atomic(dir.join(FILE), with_memory(&existing, memory).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> MemoryLimits {
        MemoryLimits::new(2048, 6144).unwrap()
    }

    #[test]
    fn the_memory_the_file_already_declares_is_read_back() {
        let text = "# comment\n-Xms1G -Xmx8G\n-XX:+UseG1GC\n";
        let (min, max) = declared_memory(text);
        assert_eq!(min.as_deref(), Some("1G"));
        assert_eq!(max.as_deref(), Some("8G"));
    }

    #[test]
    fn a_file_without_memory_says_so() {
        let (min, max) = declared_memory("-XX:+UseG1GC\n");
        assert!(min.is_none() && max.is_none());
    }

    #[test]
    fn the_examples_in_the_comments_are_not_settings() {
        let text = concat!(
            "# For example, to set the maximum to 3GB: -Xmx3G\n",
            "# To set the minimum to 2.5GB: -Xms2500M\n",
            "# Uncomment the next line to set it.\n",
            "# -Xmx4G\n",
        );
        let (min, max) = declared_memory(text);
        assert!(min.is_none(), "read {min:?} out of a comment");
        assert!(max.is_none(), "read {max:?} out of a comment");
    }

    #[test]
    fn writing_replaces_the_memory_and_keeps_everything_else() {
        let text = "# the author left a note\n-Xms1G -Xmx8G -XX:+UseG1GC\n-Dsome.flag=true\n";
        let out = with_memory(text, limits());

        assert!(out.contains("# the author left a note"));
        assert!(out.contains("-XX:+UseG1GC"));
        assert!(out.contains("-Dsome.flag=true"));
        assert!(out.contains("-Xms2048M -Xmx6144M"));
        assert!(!out.contains("-Xmx8G"));
        assert!(!out.contains("-Xms1G"));
    }

    #[test]
    fn writing_twice_leaves_one_pair() {
        let once = with_memory("-Xmx8G\n", limits());
        let twice = with_memory(&once, limits());
        assert_eq!(twice.matches("-Xmx").count(), 1);
        assert_eq!(twice.matches("-Xms").count(), 1);
    }
}
