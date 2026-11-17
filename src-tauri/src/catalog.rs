//! The list of game versions the launcher offers.
//!
//! The newest list is read from `versions.json` on the launcher's GitHub repository, so new
//! versions can be added without a launcher update. A copy is compiled in for offline use.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::net::http;

const CATALOG_URL: &str = "https://raw.githubusercontent.com/YukkitMC/yukkit-laucher/main/versions.json";
const BUILTIN: &str = include_str!("../../versions.json");
const CACHE_FOR: Duration = Duration::from_secs(10 * 60);

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GameVersion {
    /// Stable id, also the instance folder name: `26.3`.
    pub id: String,
    pub name: String,
    /// Minecraft version to install.
    pub minecraft: String,
    /// Fabric Loader version. Empty means "newest stable loader".
    #[serde(default)]
    pub fabric_loader: String,
    /// Java major version the game needs.
    #[serde(default = "default_java")]
    pub java: u32,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub description: String,
    /// GitHub repository (`owner/name`) whose releases carry the Yukkit Toolkit jar.
    #[serde(default = "default_repo")]
    pub toolkit_repo: String,
    /// Text a release title or notes must contain to belong to this version.
    #[serde(default)]
    pub toolkit_match: String,
}

fn default_java() -> u32 {
    25
}

fn default_repo() -> String {
    "YukkitMC/yukkit-toolkit".into()
}

#[derive(Deserialize)]
struct CatalogFile {
    versions: Vec<GameVersion>,
}

pub fn builtin() -> Vec<GameVersion> {
    serde_json::from_str::<CatalogFile>(BUILTIN)
        .map(|c| c.versions)
        .unwrap_or_default()
}

static CACHE: Mutex<Option<(Instant, Vec<GameVersion>)>> = Mutex::new(None);

async fn fetch_remote() -> Option<Vec<GameVersion>> {
    let response = http()
        .get(CATALOG_URL)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let catalog: CatalogFile = serde_json::from_str(&response.text().await.ok()?).ok()?;
    (!catalog.versions.is_empty()).then_some(catalog.versions)
}

/// The version list: from GitHub when reachable, otherwise the built-in copy.
pub async fn load() -> Vec<GameVersion> {
    let cached = CACHE.lock().unwrap().clone();
    if let Some((at, list)) = cached {
        if at.elapsed() < CACHE_FOR {
            return list;
        }
    }
    match fetch_remote().await {
        Some(list) => {
            *CACHE.lock().unwrap() = Some((Instant::now(), list.clone()));
            list
        }
        None => builtin(),
    }
}

pub async fn find(id: &str) -> Option<GameVersion> {
    let list = load().await;
    list.iter()
        .find(|v| v.id == id)
        .cloned()
        .or_else(|| builtin().into_iter().find(|v| v.id == id))
}
