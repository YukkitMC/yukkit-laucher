//! Launcher settings, stored as JSON in the user's config folder.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::errors::friendly;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Empty means "use the default location".
    pub install_dir: String,
    /// Offline player name shown in game.
    pub username: String,
    /// Memory given to Minecraft, in MB.
    pub memory_mb: u32,
    /// Empty means "download and manage Java automatically".
    pub java_path: String,
    /// Extra JVM arguments, separated by spaces.
    pub jvm_args: String,
    /// Use nightly (development) builds instead of releases.
    pub nightly: bool,
    /// Keep the launcher window open while playing (otherwise it's minimized).
    pub keep_open: bool,
    /// Random id sent to the LiquidBounce API, like the official launcher does.
    pub session_token: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            install_dir: String::new(),
            username: "Player".into(),
            memory_mb: 4096,
            java_path: String::new(),
            jvm_args: String::new(),
            nightly: false,
            keep_open: true,
            session_token: uuid::Uuid::new_v4().simple().to_string(),
        }
    }
}

pub fn default_install_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("YukkitLauncher")
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("YukkitLauncher")
        .join("settings.json")
}

impl Settings {
    pub fn install_dir(&self) -> PathBuf {
        let dir = self.install_dir.trim();
        if dir.is_empty() {
            default_install_dir()
        } else {
            PathBuf::from(dir)
        }
    }

    pub fn load() -> Settings {
        let mut settings = std::fs::read_to_string(config_path())
            .ok()
            .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
            .unwrap_or_default();
        if settings.session_token.is_empty() {
            settings.session_token = uuid::Uuid::new_v4().simple().to_string();
        }
        let _ = settings.save();
        settings
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create config folder")?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("Failed to write {}", path.display()))
    }

    pub fn validate(&self) -> Result<()> {
        let name = self.username.trim();
        let valid_chars = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if name.len() < 3 || name.len() > 16 || !valid_chars {
            return Err(friendly(
                "Check your player name",
                "Player names must be 3 to 16 characters long and use only letters, numbers and underscores (_).",
            ));
        }
        if !(1024..=65536).contains(&self.memory_mb) {
            return Err(friendly(
                "Check the memory setting",
                "Memory must be between 1 GB and 64 GB. 4 GB is a good choice for most computers.",
            ));
        }
        let dir = self.install_dir.trim();
        if !dir.is_empty() && !Path::new(dir).is_absolute() {
            return Err(friendly(
                "Check the install location",
                "The install location must be a full folder path, like C:\\Games\\LiquidBounce. Use Browse to pick one.",
            ));
        }
        let java = self.java_path.trim();
        if !java.is_empty() && !Path::new(java).is_file() {
            return Err(friendly(
                "Java wasn't found",
                "The custom Java path in Settings doesn't point to a Java program. Pick java.exe / javaw.exe again, or switch Java back to Automatic.",
            ));
        }
        Ok(())
    }
}
