//! Installs or updates a game version (Minecraft + Fabric + Fabric API + Yukkit Toolkit)
//! and records how to launch it.
//!
//! Layout inside the install folder:
//! ```text
//! runtimes/            Java, downloaded once per major version
//! libraries/ assets/   shared Minecraft files
//! versions/<mc>/       Minecraft client jars
//! cache/mods/          downloaded mod jars, reused across reinstalls
//! instances/<id>/      the game folder of each version (worlds, options, mods)
//! ```

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use futures_util::{stream, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::catalog::GameVersion;
use crate::errors::{friendly, FriendlyExt};
use crate::minecraft::{self, ArgValue, AssetIndex, VersionManifest, VersionProfile};
use crate::net::{download, file_ok, get_json};
use crate::progress::Reporter;
use crate::settings::Settings;
use crate::sources::{self, ToolkitRelease};
use crate::java;

/// Everything needed to start an installed version, saved so it can start without internet.
#[derive(Serialize, Deserialize, Clone)]
pub struct LaunchPlan {
    pub java: PathBuf,
    pub java_major: u32,
    pub main_class: String,
    pub classpath: Vec<PathBuf>,
    pub jvm_args: Vec<ArgValue>,
    pub game_args: Vec<ArgValue>,
    pub legacy_args: Option<String>,
    pub version_id: String,
    pub version_type: String,
    pub asset_index: String,
    pub assets_dir: PathBuf,
    pub libraries_dir: PathBuf,
    pub natives_dir: PathBuf,
    pub game_dir: PathBuf,
    pub client_jar: PathBuf,
    pub mods_dir: PathBuf,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Installed {
    pub version_id: String,
    pub minecraft: String,
    pub toolkit: ToolkitRelease,
    pub fabric_api: String,
    /// Mod files the launcher put in the mods folder (the player's own mods are left alone).
    pub managed_mods: Vec<String>,
    pub plan: LaunchPlan,
}

pub fn instance_dir(root: &Path, version_id: &str) -> PathBuf {
    root.join("instances").join(version_id)
}

fn installed_path(root: &Path, version_id: &str) -> PathBuf {
    instance_dir(root, version_id).join("yukkit-installed.json")
}

pub fn load_installed(root: &Path, version_id: &str) -> Option<Installed> {
    let text = std::fs::read_to_string(installed_path(root, version_id)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Removes what the launcher installed for a version. Worlds, settings and the player's own mods stay.
pub fn uninstall(root: &Path, version_id: &str) -> Result<()> {
    if let Some(installed) = load_installed(root, version_id) {
        for name in &installed.managed_mods {
            let _ = std::fs::remove_file(installed.plan.mods_dir.join(name));
        }
    }
    let path = installed_path(root, version_id);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

struct Job {
    url: String,
    path: PathBuf,
    sha1: Option<String>,
    size: Option<u64>,
}

async fn run_jobs(jobs: Vec<Job>, rep: &Reporter, step: u8, title: &str, noun: &str) -> Result<()> {
    let total = jobs.len().max(1);
    let pending: Vec<Job> = jobs.into_iter().filter(|j| !file_ok(&j.path, j.size)).collect();
    let done = AtomicUsize::new(total - pending.len().min(total));
    if pending.is_empty() {
        rep.step(step, title, format!("All {noun} are up to date"), Some(1.0));
        return Ok(());
    }
    rep.step(
        step,
        title,
        format!("Downloading {} {noun}", pending.len()),
        Some(done.load(Ordering::SeqCst) as f64 / total as f64),
    );

    let done = &done;
    stream::iter(pending)
        .map(|job| async move {
            download(&job.url, &job.path, job.sha1.as_deref(), |_, _| {}).await?;
            let n = done.fetch_add(1, Ordering::SeqCst) + 1;
            rep.update(step, title, format!("{n} of {total} {noun}"), Some(n as f64 / total as f64));
            Ok::<(), anyhow::Error>(())
        })
        .buffer_unordered(16)
        .try_collect::<Vec<()>>()
        .await?;
    rep.step(step, title, format!("All {noun} are up to date"), Some(1.0));
    Ok(())
}

/// The Fabric Loader for a version: the pinned one, or the newest stable.
async fn fabric_loader(version: &GameVersion) -> Result<String> {
    if !version.fabric_loader.trim().is_empty() {
        return Ok(version.fabric_loader.trim().to_string());
    }
    #[derive(Deserialize)]
    struct Entry {
        loader: Loader,
    }
    #[derive(Deserialize)]
    struct Loader {
        version: String,
        #[serde(default)]
        stable: bool,
    }
    let entries: Vec<Entry> =
        get_json(&format!("https://meta.fabricmc.net/v2/versions/loader/{}", version.minecraft)).await?;
    entries
        .iter()
        .find(|e| e.loader.stable)
        .or(entries.first())
        .map(|e| e.loader.version.clone())
        .ok_or_else(|| friendly("Fabric isn't ready for this version", "Fabric hasn't been released for this Minecraft version yet."))
}

pub async fn install(app: &AppHandle, settings: &Settings, version: &GameVersion) -> Result<Installed> {
    let rep = Reporter::new(app.clone());
    let root = settings.install_dir();
    tokio::fs::create_dir_all(&root).await.friendly(
        "Can't use the install location",
        "The launcher couldn't create its folder. Choose a different install location in Settings (for example, a folder inside Documents).",
    )?;
    let previous = load_installed(&root, &version.id);

    // 1. What should be installed?
    let t1 = "Checking for updates";
    rep.step(1, t1, format!("Looking for the newest Yukkit Toolkit for {}", version.minecraft), None);
    let toolkit = match (&previous, settings.auto_update) {
        // Keep the installed build when automatic updates are off and its jar is still cached.
        (Some(prev), false) if file_ok(&root.join("cache").join("mods").join(&prev.toolkit.jar_name), None) => {
            prev.toolkit.clone()
        }
        _ => sources::latest_toolkit(version, settings.previews).await?,
    };
    let fabric_api = sources::fabric_api(&version.minecraft).await?;
    let loader = fabric_loader(version).await?;
    rep.step(1, t1, format!("Yukkit Toolkit {} for Minecraft {}", toolkit.tag, version.minecraft), Some(1.0));

    // 2. Minecraft + Fabric version profiles
    let t2 = "Preparing Minecraft";
    rep.step(2, t2, format!("Loading Minecraft {} and Fabric {loader}", version.minecraft), None);
    let profile_url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
        version.minecraft, loader
    );
    let fabric: VersionProfile = get_json(&profile_url).await.friendly(
        "Couldn't load the Fabric mod loader",
        "Yukkit Toolkit runs on Fabric, and Fabric's setup info couldn't be downloaded. Try again in a moment.",
    )?;
    let parent_id = fabric.inherits_from.clone().unwrap_or_else(|| version.minecraft.clone());
    let mojang: VersionManifest = get_json(minecraft::VERSION_MANIFEST).await.friendly(
        "Couldn't reach Minecraft's servers",
        "The launcher couldn't load the list of Minecraft versions from Mojang. Check your connection and try again.",
    )?;
    let parent_url = mojang
        .versions
        .iter()
        .find(|v| v.id == parent_id)
        .map(|v| v.url.clone())
        .ok_or_else(|| {
            friendly(
                "Minecraft version not found",
                "Mojang doesn't list the Minecraft version this build needs. Please try again later.",
            )
        })?;
    let vanilla: VersionProfile = get_json(&parent_url).await?;
    let client_download = vanilla.downloads.as_ref().and_then(|d| d.client.clone());
    let profile = minecraft::merge(fabric, vanilla);
    rep.step(2, t2, "Version information loaded", Some(1.0));

    // 3. Java
    let java_major = profile
        .java_version
        .as_ref()
        .map(|j| j.major_version)
        .unwrap_or(0)
        .max(version.java);
    let java_bin = if settings.java_path.trim().is_empty() {
        java::ensure(java_major, &root.join("runtimes"), &rep).await?
    } else {
        rep.step(3, "Setting up Java", "Using the Java you chose in Settings", Some(1.0));
        PathBuf::from(settings.java_path.trim())
    };

    // 4. Game files: client jar + libraries
    let t4 = "Downloading Minecraft";
    let libraries_dir = root.join("libraries");
    let client_jar = root.join("versions").join(&parent_id).join(format!("{parent_id}.jar"));
    let mut jobs = Vec::new();
    if let Some(client) = client_download {
        jobs.push(Job { url: client.url, path: client_jar.clone(), sha1: client.sha1, size: client.size });
    }
    let mut classpath = Vec::new();
    for lib in minecraft::resolve_libraries(&profile, &libraries_dir)? {
        if !classpath.contains(&lib.path) {
            classpath.push(lib.path.clone());
        }
        if !lib.url.is_empty() {
            jobs.push(Job { url: lib.url, path: lib.path, sha1: lib.sha1, size: lib.size });
        }
    }
    classpath.push(client_jar.clone());
    run_jobs(jobs, &rep, 4, t4, "game files").await?;

    // 5. Assets (sounds, languages, textures)
    let t5 = "Downloading sounds & textures";
    let assets_dir = root.join("assets");
    let index_ref = profile.asset_index.clone().context("The Minecraft version has no asset index")?;
    let index_path = assets_dir.join("indexes").join(format!("{}.json", index_ref.id));
    if !file_ok(&index_path, None) {
        download(&index_ref.url, &index_path, index_ref.sha1.as_deref(), |_, _| {}).await?;
    }
    let index: AssetIndex = serde_json::from_str(&tokio::fs::read_to_string(&index_path).await?)?;
    let mut asset_jobs = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for object in index.objects.values() {
        if object.hash.len() < 2 || !seen.insert(object.hash.clone()) {
            continue;
        }
        let prefix = &object.hash[..2];
        asset_jobs.push(Job {
            url: format!("{}/{prefix}/{}", minecraft::RESOURCES, object.hash),
            path: assets_dir.join("objects").join(prefix).join(&object.hash),
            sha1: Some(object.hash.clone()),
            size: Some(object.size),
        });
    }
    run_jobs(asset_jobs, &rep, 5, t5, "files").await?;

    // 6. Fabric API + Yukkit Toolkit
    let t6 = "Installing Yukkit Toolkit";
    let game_dir = instance_dir(&root, &version.id);
    let mods_dir = game_dir.join("mods");
    let cache = root.join("cache").join("mods");
    tokio::fs::create_dir_all(&mods_dir).await?;
    tokio::fs::create_dir_all(&cache).await?;

    if let Some(prev) = &previous {
        for name in &prev.managed_mods {
            let _ = tokio::fs::remove_file(mods_dir.join(name)).await;
        }
    }

    let mods = [
        ("Fabric API", fabric_api.file_name.clone(), fabric_api.url.clone(), fabric_api.sha1.clone()),
        ("Yukkit Toolkit", toolkit.jar_name.clone(), toolkit.jar_url.clone(), None),
    ];
    let mut managed_mods = Vec::new();
    for (label, file_name, url, sha1) in mods {
        if file_name.is_empty() || file_name.contains(['/', '\\']) || file_name.contains("..") {
            return Err(friendly(
                "A download had an unsafe file name",
                "The launcher refused a mod file with a suspicious name. Please report this on GitHub.",
            )
            .context(format!("Rejected file name {file_name:?} for {label}")));
        }
        let cached = cache.join(&file_name);
        if !file_ok(&cached, None) {
            rep.step(6, t6, format!("Downloading {label}"), Some(0.0));
            download(&url, &cached, sha1.as_deref(), |d, t| rep.bytes(6, t6, d, t)).await?;
        }
        tokio::fs::copy(&cached, mods_dir.join(&file_name)).await?;
        managed_mods.push(file_name);
    }

    let natives_dir = root.join("versions").join(&profile.id).join("natives");
    tokio::fs::create_dir_all(&natives_dir).await?;
    let args = profile.arguments.clone().unwrap_or_default();
    let installed = Installed {
        version_id: version.id.clone(),
        minecraft: version.minecraft.clone(),
        toolkit,
        fabric_api: fabric_api.version,
        managed_mods,
        plan: LaunchPlan {
            java: java_bin,
            java_major,
            main_class: profile.main_class.clone().context("The version profile has no main class")?,
            classpath,
            jvm_args: args.jvm,
            game_args: args.game,
            legacy_args: profile.minecraft_arguments.clone(),
            version_id: profile.id.clone(),
            version_type: profile.kind.clone().unwrap_or_else(|| "release".into()),
            asset_index: index_ref.id,
            assets_dir,
            libraries_dir,
            natives_dir,
            game_dir,
            client_jar,
            mods_dir,
        },
    };
    tokio::fs::write(installed_path(&root, &version.id), serde_json::to_string_pretty(&installed)?).await?;
    rep.step(6, t6, "Yukkit Toolkit is installed", Some(1.0));
    Ok(installed)
}

/// What the Play page shows about a local install.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Checks {
    pub java: bool,
    pub minecraft: bool,
    pub toolkit: bool,
}

pub fn checks(installed: Option<&Installed>, settings: &Settings) -> Checks {
    let Some(inst) = installed else {
        return Checks { java: false, minecraft: false, toolkit: false };
    };
    let p = &inst.plan;
    let java = if settings.java_path.trim().is_empty() {
        p.java.is_file()
    } else {
        Path::new(settings.java_path.trim()).is_file()
    };
    let minecraft = p.client_jar.is_file() && p.classpath.iter().all(|f| f.is_file());
    let toolkit = inst.managed_mods.iter().all(|m| p.mods_dir.join(m).is_file());
    Checks { java, minecraft, toolkit }
}
