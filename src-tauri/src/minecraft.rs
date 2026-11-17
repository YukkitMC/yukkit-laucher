//! Minecraft version profiles (Mojang + Fabric), libraries, rules and assets.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const VERSION_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const RESOURCES: &str = "https://resources.download.minecraft.net";

#[derive(Deserialize)]
pub struct VersionManifest {
    pub versions: Vec<ManifestEntry>,
}

#[derive(Deserialize)]
pub struct ManifestEntry {
    pub id: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VersionProfile {
    pub id: String,
    #[serde(rename = "inheritsFrom", default)]
    pub inherits_from: Option<String>,
    #[serde(rename = "mainClass", default)]
    pub main_class: Option<String>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(rename = "minecraftArguments", default)]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "assetIndex", default)]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
    #[serde(rename = "javaVersion", default)]
    pub java_version: Option<JavaVersion>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<ArgValue>,
    #[serde(default)]
    pub jvm: Vec<ArgValue>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ArgValue {
    Plain(String),
    Ruled { rules: Vec<Rule>, value: StringOrList },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum StringOrList {
    One(String),
    Many(Vec<String>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub rules: Option<Vec<Rule>>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VersionDownloads {
    #[serde(default)]
    pub client: Option<Artifact>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JavaVersion {
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

pub fn current_os() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "osx",
        _ => "linux",
    }
}

impl Rule {
    fn matches(&self) -> bool {
        if let Some(os) = &self.os {
            if let Some(name) = &os.name {
                if name != current_os() {
                    return false;
                }
            }
            if let Some(arch) = &os.arch {
                if arch == "x86" && cfg!(target_pointer_width = "64") {
                    return false;
                }
            }
        }
        // Optional features (demo mode, custom resolution, quick play) are never enabled.
        if let Some(features) = &self.features {
            if features.values().any(|v| *v) {
                return false;
            }
        }
        true
    }
}

pub fn rules_allow(rules: &[Rule]) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule.matches() {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

pub fn expand_args(args: &[ArgValue]) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            ArgValue::Plain(s) => out.push(s.clone()),
            ArgValue::Ruled { rules, value } => {
                if rules_allow(rules) {
                    match value {
                        StringOrList::One(s) => out.push(s.clone()),
                        StringOrList::Many(list) => out.extend(list.iter().cloned()),
                    }
                }
            }
        }
    }
    out
}

/// `group:artifact:version[:classifier][@ext]` → `group/path/artifact/version/artifact-version[-classifier].ext`
pub fn maven_path(name: &str) -> Result<String> {
    let (coords, ext) = name.split_once('@').unwrap_or((name, "jar"));
    let parts: Vec<&str> = coords.split(':').collect();
    if parts.len() < 3 {
        bail!("Invalid library name: {name}");
    }
    let (group, artifact, version) = (parts[0], parts[1], parts[2]);
    let classifier = parts.get(3).map(|c| format!("-{c}")).unwrap_or_default();
    Ok(format!(
        "{}/{artifact}/{version}/{artifact}-{version}{classifier}.{ext}",
        group.replace('.', "/")
    ))
}

/// Identity of a library ignoring its version, used to let Fabric override vanilla libraries.
fn library_key(name: &str) -> String {
    let coords = name.split('@').next().unwrap_or(name);
    let parts: Vec<&str> = coords.split(':').collect();
    match parts.len() {
        0..=2 => coords.to_string(),
        3 => format!("{}:{}", parts[0], parts[1]),
        _ => format!("{}:{}:{}", parts[0], parts[1], parts[3]),
    }
}

/// Combines a loader profile (Fabric) with the vanilla profile it inherits from.
pub fn merge(mut child: VersionProfile, parent: VersionProfile) -> VersionProfile {
    let mut seen: HashSet<String> = child.libraries.iter().map(|l| library_key(&l.name)).collect();
    for lib in parent.libraries {
        if seen.insert(library_key(&lib.name)) {
            child.libraries.push(lib);
        }
    }
    child.main_class = child.main_class.or(parent.main_class);
    child.arguments = match (parent.arguments, child.arguments.take()) {
        (Some(p), Some(c)) => Some(Arguments {
            game: [p.game, c.game].concat(),
            jvm: [p.jvm, c.jvm].concat(),
        }),
        (p, c) => c.or(p),
    };
    child.minecraft_arguments = child.minecraft_arguments.or(parent.minecraft_arguments);
    child.asset_index = child.asset_index.or(parent.asset_index);
    child.downloads = child.downloads.or(parent.downloads);
    child.java_version = child.java_version.or(parent.java_version);
    child.kind = child.kind.or(parent.kind);
    child.inherits_from = None;
    child
}

pub struct LibFile {
    pub path: PathBuf,
    /// Empty when the file can't be downloaded (it must already be present).
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

pub fn resolve_libraries(profile: &VersionProfile, libraries_dir: &Path) -> Result<Vec<LibFile>> {
    let mut files = Vec::new();
    for lib in &profile.libraries {
        if let Some(rules) = &lib.rules {
            if !rules_allow(rules) {
                continue;
            }
        }
        match &lib.downloads {
            Some(downloads) => {
                // Old-style libraries without an artifact only carry natives we don't need.
                let Some(artifact) = &downloads.artifact else { continue };
                let rel = match &artifact.path {
                    Some(p) => p.clone(),
                    None => maven_path(&lib.name)?,
                };
                files.push(LibFile {
                    path: libraries_dir.join(rel),
                    url: artifact.url.clone(),
                    sha1: artifact.sha1.clone(),
                    size: artifact.size,
                });
            }
            None => {
                let mut base = lib.url.clone().unwrap_or_else(|| "https://libraries.minecraft.net/".into());
                if !base.ends_with('/') {
                    base.push('/');
                }
                let rel = maven_path(&lib.name)?;
                files.push(LibFile {
                    path: libraries_dir.join(&rel),
                    url: format!("{base}{rel}"),
                    sha1: lib.sha1.clone(),
                    size: lib.size,
                });
            }
        }
    }
    Ok(files)
}
