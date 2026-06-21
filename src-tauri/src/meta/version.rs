use std::collections::HashMap;

use serde::Deserialize;

use crate::download::DownloadSpec;
use crate::paths::Paths;

#[derive(Debug, Clone, Deserialize)]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndexRef,
    pub assets: String,
    pub downloads: Downloads,
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(rename = "minecraftArguments", default)]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "javaVersion", default)]
    pub java_version: Option<JavaVersion>,
    #[serde(rename = "type")]
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
            let downloads = match &lib.downloads {
                Some(d) => d,
                None => continue,
            };

            if let Some(natives) = &lib.natives {
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

            if let Some(artifact) = &downloads.artifact {
                if let Some(spec) = artifact.to_spec(paths) {
                    let is_native = name_classifier(&lib.name)
                        .map_or(false, |c| c.starts_with("natives"));
                    if is_native {
                        resolved.natives.push(NativeSpec { spec, exclude });
                    } else {
                        resolved.classpath.push(spec);
                    }
                }
            }
        }

        resolved
    }

    pub fn client_spec(&self, paths: &Paths) -> DownloadSpec {
        DownloadSpec {
            url: self.downloads.client.url.clone(),
            dest: paths.version_jar(&self.id),
            sha1: Some(self.downloads.client.sha1.clone()),
            size: Some(self.downloads.client.size),
        }
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
