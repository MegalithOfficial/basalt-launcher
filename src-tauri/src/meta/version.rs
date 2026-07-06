use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::download::DownloadSpec;
use crate::paths::Paths;

const DEFAULT_MAVEN: &str = "https://libraries.minecraft.net/";

#[derive(Debug, Clone, Deserialize)]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(rename = "inheritsFrom", default)]
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub jar: Option<String>,
    #[serde(rename = "assetIndex", default)]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub downloads: Option<Downloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(rename = "minecraftArguments", default)]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "javaVersion", default)]
    pub java_version: Option<JavaVersion>,
    #[serde(rename = "type", default)]
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Arg>,
    #[serde(default)]
    pub jvm: Vec<Arg>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Arg {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    Single(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Downloads {
    pub client: Artifact,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub extract: Option<Extract>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JavaVersion {
    #[serde(default)]
    pub component: String,
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Clone)]
pub struct NativeSpec {
    pub spec: DownloadSpec,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedLibraries {
    pub classpath: Vec<DownloadSpec>,
    pub natives: Vec<NativeSpec>,
}

pub fn current_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "osx",
        other => other,
    }
}

fn current_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "arm64",
        other => other,
    }
}

fn arch_bits() -> &'static str {
    if cfg!(target_pointer_width = "64") {
        "64"
    } else {
        "32"
    }
}

pub fn rules_allow(rules: &[Rule]) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule.features.is_some() {
            continue;
        }
        let os_matches = rule.os.as_ref().map_or(true, |os| {
            os.name.as_ref().map_or(true, |n| n == current_os())
                && os.arch.as_ref().map_or(true, |a| a == current_arch())
        });
        if os_matches {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

fn name_classifier(name: &str) -> Option<&str> {
    name.splitn(5, ':').nth(3)
}

fn dedupe_key(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    match parts.len() {
        0 | 1 | 2 => name.to_string(),
        3 => format!("{}:{}", parts[0], parts[1]),
        _ => format!("{}:{}:{}", parts[0], parts[1], parts[3]),
    }
}

fn maven_path(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let classifier = parts.get(3).map(|c| format!("-{c}")).unwrap_or_default();
    Some(format!(
        "{group}/{artifact}/{version}/{artifact}-{version}{classifier}.jar"
    ))
}

impl Artifact {
    fn to_spec(&self, paths: &Paths) -> Option<DownloadSpec> {
        let path = self.path.as_ref()?;
        Some(DownloadSpec {
            url: self.url.clone(),
            dest: paths.libraries().join(path),
            sha1: Some(self.sha1.clone()),
            size: Some(self.size),
        })
    }
}

impl Library {
    fn to_spec(&self, paths: &Paths) -> Option<DownloadSpec> {
        if let Some(downloads) = &self.downloads {
            if let Some(artifact) = &downloads.artifact {
                return artifact.to_spec(paths);
            }
        }
        let path = maven_path(&self.name)?;
        let base = self.url.clone().unwrap_or_else(|| DEFAULT_MAVEN.to_string());
        let base = if base.ends_with('/') { base } else { format!("{base}/") };
        Some(DownloadSpec {
            url: format!("{base}{path}"),
            dest: paths.libraries().join(&path),
            sha1: self.sha1.clone(),
            size: self.size,
        })
    }
}

pub fn merge_versions(parent: VersionJson, child: VersionJson) -> VersionJson {
    let parent_id = parent.id.clone();
    let child_keys: HashSet<String> = child
        .libraries
        .iter()
        .map(|l| dedupe_key(&l.name))
        .collect();
    let mut libraries = child.libraries;
    libraries.extend(
        parent
            .libraries
            .into_iter()
            .filter(|l| !child_keys.contains(&dedupe_key(&l.name))),
    );

    let arguments = match (parent.arguments, child.arguments) {
        (Some(p), Some(c)) => Some(Arguments {
            game: p.game.into_iter().chain(c.game).collect(),
            jvm: p.jvm.into_iter().chain(c.jvm).collect(),
        }),
        (p, c) => c.or(p),
    };

    VersionJson {
        id: child.id,
        main_class: child.main_class,
        inherits_from: None,
        jar: child.jar.or(parent.jar).or(Some(parent_id)),
        asset_index: child.asset_index.or(parent.asset_index),
        assets: child.assets.or(parent.assets),
        downloads: child.downloads.or(parent.downloads),
        libraries,
        arguments,
        minecraft_arguments: child.minecraft_arguments.or(parent.minecraft_arguments),
        java_version: child.java_version.or(parent.java_version),
        kind: child.kind,
    }
}

impl VersionJson {
    pub fn resolve_libraries(&self, paths: &Paths) -> ResolvedLibraries {
        let mut resolved = ResolvedLibraries::default();

        for lib in &self.libraries {
            if !rules_allow(&lib.rules) {
                continue;
            }
            let exclude = lib
                .extract
                .as_ref()
                .map(|e| e.exclude.clone())
                .unwrap_or_default();

            if let (Some(natives), Some(downloads)) = (&lib.natives, &lib.downloads) {
                if let Some(template) = natives.get(current_os()) {
                    let classifier = template.replace("${arch}", arch_bits());
                    if let Some(classifiers) = &downloads.classifiers {
                        if let Some(artifact) = classifiers.get(&classifier) {
                            if let Some(spec) = artifact.to_spec(paths) {
                                resolved.natives.push(NativeSpec {
                                    spec,
                                    exclude: exclude.clone(),
                                });
                            }
                        }
                    }
                }
            }

            if let Some(spec) = lib.to_spec(paths) {
                let is_native = name_classifier(&lib.name)
                    .map_or(false, |c| c.starts_with("natives"));
                if is_native {
                    resolved.natives.push(NativeSpec { spec, exclude });
                } else {
                    resolved.classpath.push(spec);
                }
            }
        }

        resolved
    }

    pub fn jar_id(&self) -> &str {
        self.jar.as_deref().unwrap_or(&self.id)
    }

    pub fn assets_name(&self) -> String {
        self.assets
            .clone()
            .or_else(|| self.asset_index.as_ref().map(|a| a.id.clone()))
            .unwrap_or_else(|| "legacy".to_string())
    }

    pub fn client_spec(&self, paths: &Paths) -> Option<DownloadSpec> {
        let client = &self.downloads.as_ref()?.client;
        Some(DownloadSpec {
            url: client.url.clone(),
            dest: paths.version_jar(self.jar_id()),
            sha1: Some(client.sha1.clone()),
            size: Some(client.size),
        })
    }

    pub fn required_java_major(&self) -> u32 {
        self.java_version.as_ref().map_or(8, |j| j.major_version)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

impl AssetIndex {
    pub fn specs(&self, paths: &Paths) -> Vec<DownloadSpec> {
        self.objects
            .values()
            .map(|obj| {
                let sub = &obj.hash[0..2];
                DownloadSpec {
                    url: format!(
                        "https://resources.download.minecraft.net/{sub}/{}",
                        obj.hash
                    ),
                    dest: paths.assets_objects().join(sub).join(&obj.hash),
                    sha1: Some(obj.hash.clone()),
                    size: Some(obj.size),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_paths_and_dedupe() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.16.9").as_deref(),
            Some("net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar")
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-linux").as_deref(),
            Some("org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar")
        );
        assert_eq!(dedupe_key("org.ow2.asm:asm:9.7.1"), "org.ow2.asm:asm");
        assert_eq!(
            dedupe_key("org.lwjgl:lwjgl:3.3.3:natives-linux"),
            "org.lwjgl:lwjgl:natives-linux"
        );
    }

    #[test]
    fn merges_loader_profile_over_vanilla() {
        let parent: VersionJson = serde_json::from_str(
            r#"{
                "id": "1.21.1",
                "mainClass": "net.minecraft.client.main.Main",
                "type": "release",
                "assets": "17",
                "assetIndex": {"id": "17", "sha1": "a", "size": 1, "url": "u"},
                "downloads": {"client": {"sha1": "b", "size": 2, "url": "c"}},
                "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
                "arguments": {"game": ["--username"], "jvm": ["-Xss1M"]},
                "libraries": [{"name": "org.ow2.asm:asm:9.3"}]
            }"#,
        )
        .unwrap();
        let child: VersionJson = serde_json::from_str(
            r#"{
                "id": "fabric-loader-0.16.9-1.21.1",
                "inheritsFrom": "1.21.1",
                "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
                "type": "release",
                "arguments": {"game": [], "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "]},
                "libraries": [
                    {"name": "org.ow2.asm:asm:9.7.1", "url": "https://maven.fabricmc.net/", "sha1": "x", "size": 3}
                ]
            }"#,
        )
        .unwrap();

        let merged = merge_versions(parent, child);
        assert_eq!(merged.id, "fabric-loader-0.16.9-1.21.1");
        assert_eq!(merged.jar_id(), "1.21.1");
        assert_eq!(merged.main_class, "net.fabricmc.loader.impl.launch.knot.KnotClient");
        assert_eq!(merged.required_java_major(), 21);
        assert_eq!(merged.assets_name(), "17");
        assert_eq!(merged.libraries.len(), 1);
        assert!(merged.libraries[0].name.contains("9.7.1"));
        let args = merged.arguments.as_ref().unwrap();
        assert_eq!(args.jvm.len(), 2);
        assert!(merged.downloads.is_some());
    }
}
