pub mod redact;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Fix {
    None,
    OpenModsFolder,
    InstallJava { major: u32 },
    FindContent { query: String },
    RaiseMemory { megabytes: u32 },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Diagnosis {
    pub id: &'static str,
    pub title: String,
    pub detail: String,
    pub subjects: Vec<String>,
    pub fix: Fix,
}

pub struct Context {
    pub memory: crate::config::MemoryLimits,
    pub total_memory_mb: u64,
}

fn after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.find(marker).map(|at| &line[at + marker.len()..])
}

fn until<'a>(text: &'a str, stop: &[char]) -> &'a str {
    let text = text.trim_start();
    let end = text.find(stop).unwrap_or(text.len());
    text[..end].trim()
}

fn quoted(text: &str) -> Option<String> {
    let start = text.find('\'')?;
    let rest = &text[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

fn java_from_class_version(value: &str) -> Option<u32> {
    let digits: String = value
        .trim_start()
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    let major: u32 = digits.parse().ok()?;
    (major >= 45).then(|| major - 44)
}

fn dedupe(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn duplicate_mods(lines: &[&str]) -> Option<Diagnosis> {
    let mut subjects = Vec::new();
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = after(&lower, "duplicate mod id:") {
            let index = line.len() - rest.len();
            subjects.push(until(&line[index..], &[',', ' ', ')']).to_string());
        } else if lower.contains("duplicate mod") || lower.contains("found duplicate") {
            if let Some(name) = quoted(line) {
                subjects.push(name);
            }
        }
    }
    let subjects = dedupe(subjects.into_iter().filter(|v| !v.is_empty()).collect());
    if subjects.is_empty() {
        return None;
    }
    Some(Diagnosis {
        id: "duplicate-mod",
        title: "The same mod is installed twice".to_string(),
        detail:
            "Two copies of one mod are in the mods folder, usually an old file left behind next to a newer one. Minecraft refuses to start until one is removed."
                .to_string(),
        subjects,
        fix: Fix::OpenModsFolder,
    })
}

fn java_mismatch(lines: &[&str]) -> Option<Diagnosis> {
    for line in lines {
        if !line.contains("UnsupportedClassVersionError") {
            continue;
        }
        let needs = after(line, "class file version ").and_then(java_from_class_version);
        let has = after(line, "class file versions up to ").and_then(java_from_class_version);
        let subject = after(line, "UnsupportedClassVersionError: ")
            .map(|rest| until(rest, &[' ']).replace('/', "."))
            .filter(|value| !value.is_empty());
        let Some(needs) = needs else { continue };
        return Some(Diagnosis {
            id: "java-version",
            title: format!("This pack needs Java {needs}"),
            detail: match has {
                Some(has) => format!(
                    "The instance launched with Java {has}, but its mods were built for Java {needs}."
                ),
                None => format!("Its mods were built for Java {needs}."),
            },
            subjects: subject.into_iter().collect(),
            fix: Fix::InstallJava { major: needs },
        });
    }
    None
}

fn missing_dependency(lines: &[&str]) -> Option<Diagnosis> {
    let mut subjects = Vec::new();
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if !(lower.contains("which is missing")
            || lower.contains("missing or unsupported mandatory"))
        {
            continue;
        }
        if let Some(rest) = after(&lower, " of ") {
            let index = line.len() - rest.len();
            let name = until(&line[index..], &[',', '!']).to_string();
            if !name.is_empty() {
                subjects.push(name);
            }
        }
    }
    let subjects = dedupe(subjects);
    if subjects.is_empty() {
        return None;
    }
    let query = subjects.first().cloned().unwrap_or_default();
    Some(Diagnosis {
        id: "missing-dependency",
        title: if subjects.len() == 1 {
            format!("{query} is missing")
        } else {
            format!("{} required mods are missing", subjects.len())
        },
        detail:
            "A mod needs another mod that is not installed. Installing it usually fixes the crash on its own."
                .to_string(),
        subjects,
        fix: Fix::FindContent { query },
    })
}

fn mixin_failure(lines: &[&str]) -> Option<Diagnosis> {
    let mut subjects = Vec::new();
    let mut hit = false;
    for line in lines {
        if line.contains("MixinApplyError")
            || line.contains("Mixin apply failed")
            || line.contains("mixin.injection.throwables")
        {
            hit = true;
            if let Some(rest) = after(line, "Mixin apply failed ") {
                let config = until(rest, &[' ', '-']).to_string();
                let name = config.split('.').next().unwrap_or(&config).to_string();
                if !name.is_empty() {
                    subjects.push(name);
                }
            }
        }
    }
    if !hit {
        return None;
    }
    Some(Diagnosis {
        id: "mixin-failure",
        title: "A mod could not patch the game".to_string(),
        detail:
            "One mod tried to change code that another mod had already changed, or that this Minecraft version no longer has. Updating or removing the mod named below usually clears it."
                .to_string(),
        subjects: dedupe(subjects),
        fix: Fix::None,
    })
}

fn out_of_memory(lines: &[&str], context: &Context) -> Option<Diagnosis> {
    let reason = lines.iter().find_map(|line| {
        after(line, "OutOfMemoryError: ").map(|rest| until(rest, &['\r']).to_string())
    })?;
    let suggested = context
        .memory
        .suggested_max_after_oom(context.total_memory_mb);
    Some(Diagnosis {
        id: "out-of-memory",
        title: "The game ran out of memory".to_string(),
        detail: format!(
            "Java gave up while allocating ({reason}). This instance is capped at {} MB.",
            context.memory.max_mb
        ),
        subjects: Vec::new(),
        fix: suggested
            .map(|megabytes| Fix::RaiseMemory { megabytes })
            .unwrap_or(Fix::None),
    })
}

pub fn analyze(text: &str, context: &Context) -> Vec<Diagnosis> {
    let lines: Vec<&str> = text.lines().collect();
    [
        duplicate_mods(&lines),
        java_mismatch(&lines),
        missing_dependency(&lines),
        out_of_memory(&lines, context),
        mixin_failure(&lines),
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> Context {
        Context {
            memory: crate::config::MemoryLimits::new(512, 4096).unwrap(),
            total_memory_mb: 16_384,
        }
    }

    #[test]
    fn names_the_mod_that_appears_twice() {
        let found = analyze("[main/ERROR]: Duplicate mod ID: sodium", &context());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "duplicate-mod");
        assert_eq!(found[0].subjects, vec!["sodium".to_string()]);
        assert_eq!(found[0].fix, Fix::OpenModsFolder);
    }

    #[test]
    fn reads_the_required_java_release_from_the_class_version() {
        let line = "java.lang.UnsupportedClassVersionError: net/example/Mod has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0";
        let found = analyze(line, &context());
        assert_eq!(found[0].id, "java-version");
        assert_eq!(found[0].title, "This pack needs Java 21");
        assert!(found[0].detail.contains("Java 17"));
        assert_eq!(found[0].fix, Fix::InstallJava { major: 21 });
    }

    #[test]
    fn picks_the_dependency_out_of_a_fabric_message() {
        let line = "Mod 'Sodium' (sodium) 0.5.3 requires version 0.90.0 or later of fabric-api, which is missing!";
        let found = analyze(line, &context());
        assert_eq!(found[0].id, "missing-dependency");
        assert_eq!(found[0].subjects, vec!["fabric-api".to_string()]);
        assert_eq!(
            found[0].fix,
            Fix::FindContent {
                query: "fabric-api".to_string()
            }
        );
    }

    #[test]
    fn suggests_more_memory_than_the_instance_has() {
        let found = analyze("java.lang.OutOfMemoryError: Java heap space", &context());
        assert_eq!(found[0].id, "out-of-memory");
        assert!(found[0].detail.contains("4096 MB"));
        assert_eq!(found[0].fix, Fix::RaiseMemory { megabytes: 8192 });
    }

    #[test]
    fn does_not_suggest_more_memory_than_the_machine_has() {
        let context = Context {
            memory: crate::config::MemoryLimits::new(512, 4096).unwrap(),
            total_memory_mb: 6144,
        };
        let found = analyze("java.lang.OutOfMemoryError: Java heap space", &context);
        assert_eq!(found[0].fix, Fix::RaiseMemory { megabytes: 6144 });
    }

    #[test]
    fn reports_the_mod_behind_a_failed_mixin() {
        let line = "Mixin apply failed sodium.mixins.json:MixinWorldRenderer -> net.minecraft.Foo";
        let found = analyze(line, &context());
        assert_eq!(found[0].id, "mixin-failure");
        assert_eq!(found[0].subjects, vec!["sodium".to_string()]);
    }

    #[test]
    fn a_clean_log_produces_nothing() {
        let log = "[main/INFO]: Loading 120 mods\n[main/INFO]: Stopping worker threads";
        assert!(analyze(log, &context()).is_empty());
    }
}
