//! Where the mods come from: Yukkit Toolkit from GitHub Releases, Fabric API from Modrinth.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::catalog::GameVersion;
use crate::errors::{friendly, Friendly};
use crate::net::http;

const CACHE_FOR: Duration = Duration::from_secs(5 * 60);

#[derive(Deserialize, Clone, Debug)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
    html_url: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize, Clone, Debug)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// A Yukkit Toolkit release, as the UI and installer use it.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToolkitRelease {
    pub tag: String,
    pub title: String,
    pub notes: String,
    pub date: String,
    pub url: String,
    pub prerelease: bool,
    pub jar_name: String,
    pub jar_url: String,
    pub jar_size: u64,
}

static RELEASES: Mutex<Option<(Instant, String, Vec<ToolkitRelease>)>> = Mutex::new(None);

fn github_error() -> Friendly {
    Friendly::network(
        "Can't reach GitHub",
        "Yukkit Toolkit is downloaded from GitHub, and GitHub couldn't be reached. Check your connection and try again in a minute.",
    )
}

/// All published Yukkit Toolkit releases that contain a mod jar, newest first.
pub async fn toolkit_releases(repo: &str) -> Result<Vec<ToolkitRelease>> {
    let cached = RELEASES.lock().unwrap().clone();
    if let Some((at, cached_repo, list)) = cached {
        if cached_repo == repo && at.elapsed() < CACHE_FOR {
            return Ok(list);
        }
    }

    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=30");
    let response = http()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| anyhow::Error::new(e).context(github_error()))?;
    let status = response.status();
    if status.as_u16() == 403 || status.as_u16() == 429 {
        return Err(anyhow::anyhow!("GitHub answered {status}").context(Friendly::network(
            "GitHub needs a short break",
            "GitHub limits how often it can be asked for downloads. Wait a few minutes and press Try again.",
        )));
    }
    let releases: Vec<GhRelease> = response
        .error_for_status()
        .map_err(|e| anyhow::Error::new(e).context(github_error()))?
        .json()
        .await?;

    let list: Vec<ToolkitRelease> = releases
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let jar = r.assets.iter().find(|a| {
                a.name.ends_with(".jar") && !a.name.contains("-sources") && !a.name.contains("-dev")
            })?;
            Some(ToolkitRelease {
                title: r.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| r.tag_name.clone()),
                tag: r.tag_name,
                notes: r.body.unwrap_or_default(),
                date: r.published_at.unwrap_or_default().chars().take(10).collect(),
                url: r.html_url,
                prerelease: r.prerelease,
                jar_name: jar.name.clone(),
                jar_url: jar.browser_download_url.clone(),
                jar_size: jar.size,
            })
        })
        .collect();

    *RELEASES.lock().unwrap() = Some((Instant::now(), repo.to_string(), list.clone()));
    Ok(list)
}

/// The newest Yukkit Toolkit build for a game version.
pub async fn latest_toolkit(version: &GameVersion, include_previews: bool) -> Result<ToolkitRelease> {
    let releases = toolkit_releases(&version.toolkit_repo).await?;
    let wanted = |r: &&ToolkitRelease| include_previews || !r.prerelease;
    let matches = |r: &&ToolkitRelease| {
        version.toolkit_match.is_empty()
            || r.title.contains(&version.toolkit_match)
            || r.notes.contains(&version.toolkit_match)
    };
    releases
        .iter()
        .filter(wanted)
        .find(matches)
        .or_else(|| releases.iter().find(matches))
        .cloned()
        .ok_or_else(|| {
            friendly(
                "No Yukkit Toolkit build for this version yet",
                "There isn't a Yukkit Toolkit download for this Minecraft version yet. Pick another version on the Versions page, or check back soon.",
            )
        })
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModFile {
    pub version: String,
    pub file_name: String,
    pub url: String,
    pub sha1: Option<String>,
}

#[derive(Deserialize)]
struct ModrinthVersion {
    version_number: String,
    files: Vec<ModrinthFile>,
}

#[derive(Deserialize)]
struct ModrinthFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: HashMap<String, String>,
}

/// The newest Fabric API for a Minecraft version (Yukkit Toolkit depends on it).
pub async fn fabric_api(minecraft: &str) -> Result<ModFile> {
    let game_versions = format!("[\"{minecraft}\"]");
    let versions: Vec<ModrinthVersion> = http()
        .get("https://api.modrinth.com/v2/project/fabric-api/version")
        .query(&[("loaders", "[\"fabric\"]"), ("game_versions", game_versions.as_str())])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .map_err(|e| {
            anyhow::Error::new(e).context(Friendly::network(
                "Couldn't load Fabric API",
                "Yukkit Toolkit needs Fabric API, and its download info couldn't be loaded from Modrinth. Try again in a moment.",
            ))
        })?;
    let newest = versions.into_iter().next().ok_or_else(|| {
        friendly(
            "Fabric API isn't out for this version yet",
            "Yukkit Toolkit needs Fabric API, which hasn't been released for this Minecraft version yet.",
        )
    })?;
    let file = newest
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| newest.files.first())
        .ok_or_else(|| friendly("Fabric API download is missing", "Modrinth lists Fabric API without a file. Try again later."))?;
    Ok(ModFile {
        version: newest.version_number.clone(),
        file_name: file.filename.clone(),
        url: file.url.clone(),
        sha1: file.hashes.get("sha1").cloned(),
    })
}
