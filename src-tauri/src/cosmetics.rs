//! Skin and cape chosen on the Customize page.
//!
//! The launcher only stores the pictures and hands them to the game. Drawing them in game,
//! and sharing capes between Yukkit Toolkit players, is the Toolkit mod's job.
//!
//! Layout:
//!   <config>/YukkitLauncher/cosmetics/skin.png          active skin (absent = the game's default)
//!   <config>/YukkitLauncher/cosmetics/cape.png          active cape (absent = no cape)
//!   <config>/YukkitLauncher/cosmetics/custom-skin.png   last skin the player uploaded
//!   <config>/YukkitLauncher/cosmetics/custom-cape.png   last cape the player uploaded
//!
//! On Play, the active files are copied to <instance>/yukkit/cosmetics/ together with
//! cosmetics.json, which is what the Toolkit reads.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::errors::friendly;
use crate::settings::Settings;

/// Largest picture accepted, in bytes. HD skins at 512x512 stay well under this.
const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Skin,
    Cape,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Kind> {
        match s {
            "skin" => Ok(Kind::Skin),
            "cape" => Ok(Kind::Cape),
            _ => Err(friendly("Unknown picture type", "Only skins and capes can be changed here.")),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Kind::Skin => "skin",
            Kind::Cape => "cape",
        }
    }
}

pub fn dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("YukkitLauncher")
        .join("cosmetics")
}

fn active_path(kind: Kind) -> PathBuf {
    dir().join(format!("{}.png", kind.name()))
}

fn custom_path(kind: Kind) -> PathBuf {
    dir().join(format!("custom-{}.png", kind.name()))
}

/// Width and height from a PNG header, or None if the bytes are not a PNG.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < 24 || bytes[..8] != SIG || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

/// Checks that the bytes are a PNG with a size Minecraft understands for this kind.
pub fn validate(kind: Kind, bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_BYTES {
        return Err(friendly("That picture is too big", "Pick a PNG file smaller than 2 MB."));
    }
    let Some((w, h)) = png_size(bytes) else {
        return Err(friendly("That isn't a PNG picture", "Skins and capes must be PNG files. Export the picture as PNG and try again."));
    };
    let ok = match kind {
        // 64x64 (modern), 64x32 (legacy) and HD multiples up to 512.
        Kind::Skin => w % 64 == 0 && w <= 512 && (h == w || h * 2 == w),
        // 64x32 and HD multiples up to 512x256.
        Kind::Cape => w % 64 == 0 && w <= 512 && h * 2 == w,
    };
    if !ok {
        let (title, msg) = match kind {
            Kind::Skin => ("This skin has the wrong size", format!("Skins are 64×64 or 64×32 pixels (or a larger multiple, up to 512). This one is {w}×{h}.")),
            Kind::Cape => ("This cape has the wrong size", format!("Capes are 64×32 pixels (or a larger multiple, up to 512×256). This one is {w}×{h}.")),
        };
        return Err(friendly(title, &msg));
    }
    Ok(())
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create the cosmetics folder")?;
    }
    std::fs::write(path, bytes).with_context(|| format!("Failed to write {}", path.display()))
}

/// Sets (Some) or clears (None) the active picture.
pub fn set_active(kind: Kind, bytes: Option<&[u8]>) -> Result<()> {
    let path = active_path(kind);
    match bytes {
        Some(b) => {
            validate(kind, b)?;
            write(&path, b)
        }
        None => {
            if path.exists() {
                std::fs::remove_file(&path).with_context(|| format!("Failed to remove {}", path.display()))?;
            }
            Ok(())
        }
    }
}

/// Stores an uploaded picture as the player's custom one.
pub fn save_custom(kind: Kind, bytes: &[u8]) -> Result<()> {
    validate(kind, bytes)?;
    write(&custom_path(kind), bytes)
}

pub fn read_custom(kind: Kind) -> Option<Vec<u8>> {
    std::fs::read(custom_path(kind)).ok().filter(|b| validate(kind, b).is_ok())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportSkin {
    file: &'static str,
    model: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportCape {
    file: &'static str,
}

/// What the Toolkit reads from <instance>/yukkit/cosmetics/cosmetics.json.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Export {
    format: u32,
    username: String,
    /// None = keep the game's default skin.
    skin: Option<ExportSkin>,
    /// None = no cape. Capes are only meant to be shown to other Yukkit Toolkit players.
    cape: Option<ExportCape>,
}

/// Copies the active skin and cape into the game folder for the Toolkit to pick up.
pub fn export(settings: &Settings, game_dir: &Path) -> Result<()> {
    let out = game_dir.join("yukkit").join("cosmetics");
    std::fs::create_dir_all(&out).with_context(|| format!("Failed to create {}", out.display()))?;

    let copy = |kind: Kind| -> Result<bool> {
        let target = out.join(format!("{}.png", kind.name()));
        match std::fs::read(active_path(kind)).ok().filter(|b| validate(kind, b).is_ok()) {
            Some(bytes) => {
                write(&target, &bytes)?;
                Ok(true)
            }
            None => {
                let _ = std::fs::remove_file(&target);
                Ok(false)
            }
        }
    };
    let has_skin = copy(Kind::Skin)? && settings.skin != "default";
    let has_cape = copy(Kind::Cape)? && settings.cape != "none";

    let export = Export {
        format: 1,
        username: settings.username.clone(),
        skin: has_skin.then(|| ExportSkin { file: "skin.png", model: settings.skin_model.clone() }),
        cape: has_cape.then_some(ExportCape { file: "cape.png" }),
    };
    std::fs::write(out.join("cosmetics.json"), serde_json::to_string_pretty(&export)?)
        .context("Failed to write cosmetics.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b
    }

    #[test]
    fn skin_sizes() {
        assert!(validate(Kind::Skin, &png(64, 64)).is_ok());
        assert!(validate(Kind::Skin, &png(64, 32)).is_ok());
        assert!(validate(Kind::Skin, &png(128, 128)).is_ok());
        assert!(validate(Kind::Skin, &png(64, 48)).is_err());
        assert!(validate(Kind::Skin, &png(1024, 1024)).is_err());
    }

    #[test]
    fn cape_sizes() {
        assert!(validate(Kind::Cape, &png(64, 32)).is_ok());
        assert!(validate(Kind::Cape, &png(128, 64)).is_ok());
        assert!(validate(Kind::Cape, &png(64, 64)).is_err());
    }

    #[test]
    fn rejects_non_png() {
        assert!(validate(Kind::Skin, b"GIF89a not a png at all......").is_err());
    }
}
