//! Installs or updates everything LiquidBounce needs, and records how to launch it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use futures_util::{stream, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::api::{Api, Build, ModSource};
use crate::errors::{friendly, FriendlyExt};
use crate::minecraft::{self, ArgValue, AssetIndex, VersionManifest, VersionProfile};
use crate::net::{download, file_ok, get_json};
use crate::progress::Reporter;
use crate::settings::Settings;
use crate::{download_page, java};

/// Everything needed to start the installed game, saved so it can start without internet.
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
    pub build: Build,
    pub plan: LaunchPlan,
}

fn installed_path(root: &Path) -> PathBuf {
    root.join("installed.json")
}

pub fn load_installed(root: &Path) -> Option<Installed> {
    let text = std::fs::read_to_string(installed_path(root)).ok()?;
    serde_json::from_str(&text).ok()
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
    rep.step(step, title, format!("Downloading {} {noun}", pending.len()), Some(done.load(Ordering::SeqCst) as f64 / total as f64));

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

pub async fn install(app: &AppHandle, settings: &Settings) -> Result<Installed> {
    let rep = Reporter::new(app.clone());
    let root = settings.install_dir();
    tokio::fs::create_dir_all(&root).await.friendly(
        "Can't use the install location",
        "The launcher couldn't create its folder. Choose a different install location in Settings (for example, a folder inside Documents).",
    )?;

    // 1. What should be installed?
    let t1 = "Checking for updates";
    rep.step(1, t1, "Contacting LiquidBounce", None);
    let api = Api::connect(&settings.session_token).await?;
    let latest = api.latest_build(settings.nightly).await?;
    let manifest = api.launch_manifest(latest.build_id).await?;
    let build = manifest.build.clone();
    if manifest.subsystem.name != "fabric" {
        return Err(friendly(
            "This LiquidBounce version isn't supported yet",
            "The newest LiquidBounce build uses a mod loader this launcher doesn't support. Please check for a launcher update.",
        ));
    }
    rep.step(1, t1, format!("LiquidBounce {} for Minecraft {}", build.lb_version, build.mc_version), Some(1.0));

    // 2. Minecraft + Fabric version profiles
    let t2 = "Preparing Minecraft";
    rep.step(2, t2, format!("Loading Minecraft {}", build.mc_version), None);
    let profile_url = manifest
        .subsystem
        .manifest
        .replace("{MINECRAFT_VERSION}", &build.mc_version)
        .replace("{FABRIC_LOADER_VERSION}", &build.fabric_loader_version);
    let fabric: VersionProfile = get_json(&profile_url).await.friendly(
        "Couldn't load the Fabric mod loader",
        "LiquidBounce runs on Fabric, and its setup info couldn't be downloaded. Try again in a moment.",
    )?;
    let parent_id = fabric.inherits_from.clone().unwrap_or_else(|| build.mc_version.clone());
    let mojang: VersionManifest = get_json(minecraft::VERSION_MANIFEST).await.friendly(
        "Couldn't reach Minecraft's servers",
        "The launcher couldn't load the list of Minecraft versions from Mojang. Check your connection and try again.",
    )?;
    let parent_url = mojang
        .versions
        .iter()
        .find(|v| v.id == parent_id)
        .map(|v| v.url.clone())
        .ok_or_else(|| friendly("Minecraft version not found", "Mojang doesn't list the Minecraft version this LiquidBounce build needs. Please try again later."))?;
    let vanilla: VersionProfile = get_json(&parent_url).await?;
    let client_download = vanilla.downloads.as_ref().and_then(|d| d.client.clone());
    let profile = minecraft::merge(fabric, vanilla);
    rep.step(2, t2, "Version information loaded", Some(1.0));

    // 3. Java
    let java_major = build
        .jre_version
        .filter(|v| *v > 0)
        .or(profile.java_version.as_ref().map(|j| j.major_version))
        .unwrap_or(21);
    let java_bin = if settings.java_path.trim().is_empty() {
        java::ensure(java_major, &root.join("runtimes"), &rep).await?
    } else {
        rep.step(3, "Setting up Java", "Using the Java you chose in Settings", Some(1.0));
        PathBuf::from(settings.java_path.trim())
    };

    // 4. Game files: client jar + libraries
    let t4 = "Downloading Minecraft";
    let libraries_dir = root.join("libraries");
    let version_dir = root.join("versions").join(&parent_id);
    let client_jar = version_dir.join(format!("{parent_id}.jar"));
    let mut jobs = Vec::new();
    if let Some(client) = client_download {
        jobs.push(Job { url: client.url, path: client_jar.clone(), sha1: client.sha1, size: client.size });
    }
    let libraries = minecraft::resolve_libraries(&profile, &libraries_dir)?;
    let mut classpath = Vec::new();
    for lib in libraries {
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
    let index_ref = profile
        .asset_index
        .clone()
        .context("The Minecraft version has no asset index")?;
    let index_path = assets_dir.join("indexes").join(format!("{}.json", index_ref.id));
    if !file_ok(&index_path, None) {
        download(&index_ref.url, &index_path, index_ref.sha1.as_deref(), |_, _| {}).await?;
    }
    let index: AssetIndex = serde_json::from_str(&tokio::fs::read_to_string(&index_path).await?)?;
    let mut asset_jobs = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for object in index.objects.values() {
        if !seen.insert(object.hash.clone()) {
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

    // 6. LiquidBounce and its required mods
    let t6 = "Installing LiquidBounce";
    let game_dir = root.join("gameDir").join(&build.branch);
    let mods_dir = game_dir.join("mods");
    let cache = root.join("mod_cache");
    tokio::fs::create_dir_all(&mods_dir).await?;
    tokio::fs::create_dir_all(&cache).await?;
    clear_jars(&mods_dir).await?;

    let wanted: Vec<_> = manifest.mods.iter().filter(|m| m.required || m.enabled).collect();
    for (i, m) in wanted.iter().enumerate() {
        let label = format!("{} ({} of {})", m.name, i + 1, wanted.len());
        let cached = match &m.source {
            ModSource::Repository { repository, artifact } => {
                let mut base = manifest
                    .repositories
                    .get(repository)
                    .with_context(|| format!("Unknown repository {repository}"))?
                    .clone();
                if !base.ends_with('/') {
                    base.push('/');
                }
                let rel = minecraft::maven_path(artifact)?;
                let cached = cache.join(&rel);
                if !file_ok(&cached, None) {
                    rep.step(6, t6, format!("Downloading {label}"), Some(0.0));
                    download(&format!("{base}{rel}"), &cached, None, |d, t| rep.bytes(6, t6, d, t)).await?;
                }
                cached
            }
            ModSource::Skip { artifact_name, url, extract } => {
                let cached = cache.join(format!("{artifact_name}.jar"));
                if !file_ok(&cached, None) {
                    rep.step(6, t6, "Finish the LiquidBounce download page that just opened", None);
                    let pid = download_page::obtain_file_id(app, url).await?;
                    rep.step(6, t6, format!("Downloading {label}"), Some(0.0));
                    let tmp = cache.join(format!("{artifact_name}.download"));
                    download(&api.file_url(&pid), &tmp, None, |d, t| rep.bytes(6, t6, d, t)).await?;
                    if *extract {
                        extract_first_jar(&tmp, &cached).await?;
                        let _ = tokio::fs::remove_file(&tmp).await;
                    } else {
                        tokio::fs::rename(&tmp, &cached).await?;
                    }
                }
                cached
            }
            ModSource::Other => continue,
        };
        tokio::fs::copy(&cached, mods_dir.join(format!("{}.jar", m.name))).await?;
    }

    let natives_dir = root.join("versions").join(&profile.id).join("natives");
    tokio::fs::create_dir_all(&natives_dir).await?;
    let args = profile.arguments.clone().unwrap_or_default();
    let installed = Installed {
        build,
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
    tokio::fs::write(installed_path(&root), serde_json::to_string_pretty(&installed)?).await?;
    rep.step(6, t6, "LiquidBounce is installed", Some(1.0));
    Ok(installed)
}

async fn clear_jars(dir: &Path) -> Result<()> {
    let mut entries = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "jar") {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
    Ok(())
}

async fn extract_first_jar(archive: &Path, dest: &Path) -> Result<()> {
    let (archive, dest) = (archive.to_path_buf(), dest.to_path_buf());
    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&archive)?)?;
        let index = (0..zip.len())
            .find(|&i| zip.by_index(i).map(|f| f.name().ends_with(".jar")).unwrap_or(false))
            .context("The LiquidBounce download didn't contain a .jar file")?;
        let mut entry = zip.by_index(index)?;
        let mut out = std::fs::File::create(&dest)?;
        std::io::copy(&mut entry, &mut out)?;
        Ok(())
    })
    .await?
}

/// What the home screen shows about the local install.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checks {
    pub java: bool,
    pub minecraft: bool,
    pub client: bool,
}

pub fn checks(installed: Option<&Installed>, settings: &Settings) -> Checks {
    let Some(inst) = installed else {
        return Checks { java: false, minecraft: false, client: false };
    };
    let p = &inst.plan;
    let java = if settings.java_path.trim().is_empty() {
        p.java.is_file()
    } else {
        Path::new(settings.java_path.trim()).is_file()
    };
    let minecraft = p.client_jar.is_file() && p.classpath.iter().all(|f| f.is_file());
    let client = std::fs::read_dir(&p.mods_dir)
        .map(|entries| entries.flatten().any(|e| e.path().extension().map_or(false, |x| x == "jar")))
        .unwrap_or(false);
    Checks { java, minecraft, client }
}
