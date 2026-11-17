//! The LiquidBounce (CCBlueX) API: builds, launch manifests and changelogs.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::errors::{Friendly, FriendlyExt};
use crate::net::http;

const ENDPOINTS: [&str; 2] = ["https://api.liquidbounce.net", "https://api.ccbluex.net"];
const BRANCH: &str = "nextgen";

#[derive(Clone)]
pub struct Api {
    pub base: String,
    token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Build {
    pub build_id: u32,
    #[serde(default)]
    pub commit_id: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default)]
    pub subsystem: String,
    #[serde(default)]
    pub lb_version: String,
    pub mc_version: String,
    #[serde(default)]
    pub release: bool,
    #[serde(default)]
    pub date: serde_json::Value,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub jre_version: Option<u32>,
    #[serde(default)]
    pub fabric_loader_version: String,
}

fn default_branch() -> String {
    BRANCH.to_string()
}

#[derive(Deserialize)]
struct Page<T> {
    items: Vec<T>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Subsystem {
    pub name: String,
    pub manifest: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LoaderMod {
    #[serde(default)]
    pub required: bool,
    #[serde(default, alias = "default")]
    pub enabled: bool,
    pub name: String,
    pub source: ModSource,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum ModSource {
    /// Downloaded through LiquidBounce's download page (keeps the client free).
    #[serde(rename = "skip", rename_all = "camelCase")]
    Skip {
        artifact_name: String,
        url: String,
        #[serde(default)]
        extract: bool,
    },
    #[serde(rename = "repository")]
    Repository { repository: String, artifact: String },
    #[serde(other)]
    Other,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LaunchManifest {
    pub build: Build,
    pub subsystem: Subsystem,
    #[serde(default)]
    pub mods: Vec<LoaderMod>,
    #[serde(default)]
    pub repositories: BTreeMap<String, String>,
}

impl Api {
    /// Finds the first LiquidBounce API endpoint that answers.
    pub async fn connect(token: &str) -> Result<Api> {
        let mut problems = Vec::new();
        for endpoint in ENDPOINTS {
            match http().get(endpoint).send().await {
                Ok(r) if r.status().is_success() => {
                    return Ok(Api { base: endpoint.to_string(), token: token.to_string() })
                }
                Ok(r) => problems.push(format!("{endpoint} answered {}", r.status())),
                Err(e) => problems.push(format!("{endpoint}: {e}")),
            }
        }
        Err(anyhow::anyhow!(problems.join("\n")).context(Friendly::network(
            "Can't reach LiquidBounce",
            "The launcher couldn't connect to the LiquidBounce servers. Check that you're online and that a firewall, VPN or antivirus isn't blocking the launcher, then press Retry.",
        )))
    }

    async fn get<T: DeserializeOwned>(&self, version: &str, path: &str, query: &[(&str, &str)]) -> Result<T> {
        let url = format!("{}/{}/{}", self.base, version, path);
        let text = http()
            .get(&url)
            .query(query)
            .header("X-Session-Token", &self.token)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        serde_json::from_str(&text).with_context(|| format!("Unexpected answer from {url}"))
    }

    /// The newest release (or newest build when `nightly`).
    pub async fn latest_build(&self, nightly: bool) -> Result<Build> {
        let nightly = if nightly { "true" } else { "false" };
        let page: Page<Build> = self
            .get("api/v3", &format!("version/{BRANCH}/builds"), &[("page", "1"), ("limit", "1"), ("nightly", nightly)])
            .await
            .friendly(
                "Couldn't load LiquidBounce info",
                "The LiquidBounce servers answered, but the version list couldn't be read. Try again in a moment.",
            )?;
        page.items.into_iter().next().ok_or_else(|| {
            crate::errors::friendly(
                "No LiquidBounce version available",
                "There's no LiquidBounce build to install right now. If you turned on nightly builds, try turning them off in Settings.",
            )
        })
    }

    pub async fn launch_manifest(&self, build_id: u32) -> Result<LaunchManifest> {
        self.get("api/v1", &format!("version/launch/{build_id}"), &[]).await.friendly(
            "Couldn't load the install instructions",
            "The launcher couldn't get the list of files LiquidBounce needs. Try again in a moment.",
        )
    }

    pub async fn changelog(&self, build_id: u32) -> Result<String> {
        #[derive(Deserialize)]
        struct Changelog {
            #[serde(default)]
            changelog: String,
        }
        let c: Changelog = self.get("api/v1", &format!("version/changelog/{build_id}"), &[]).await?;
        Ok(c.changelog)
    }

    pub fn file_url(&self, pid: &str) -> String {
        format!("{}/api/v3/file/{}", self.base, pid)
    }
}
