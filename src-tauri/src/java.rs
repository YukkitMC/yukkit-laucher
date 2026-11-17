//! Finds or downloads the Java runtime Minecraft needs (Eclipse Temurin via Adoptium).

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::errors::{friendly, FriendlyExt};
use crate::net::download;
use crate::progress::Reporter;

const STEP: u8 = 3;
const TITLE: &str = "Setting up Java";

fn java_binary_name() -> &'static str {
    if cfg!(windows) {
        "javaw.exe"
    } else {
        "java"
    }
}

/// Looks for `bin/java(w)` anywhere a few folders deep (archives nest it differently per OS).
pub fn find_java(dir: &Path) -> Option<PathBuf> {
    fn search(dir: &Path, depth: u32) -> Option<PathBuf> {
        let candidate = dir.join("bin").join(java_binary_name());
        if candidate.is_file() {
            return Some(candidate);
        }
        if depth == 0 {
            return None;
        }
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = search(&path, depth - 1) {
                    return Some(found);
                }
            }
        }
        None
    }
    search(dir, 4)
}

/// Returns a Java `major` runtime, downloading it into `runtimes` the first time.
pub async fn ensure(major: u32, runtimes: &Path, rep: &Reporter) -> Result<PathBuf> {
    let dir = runtimes.join(format!("temurin-{major}"));
    if let Some(bin) = find_java(&dir) {
        rep.step(STEP, TITLE, format!("Java {major} is ready"), Some(1.0));
        return Ok(bin);
    }

    let (os, ext) = match std::env::consts::OS {
        "windows" => ("windows", "zip"),
        "macos" => ("mac", "tar.gz"),
        _ => ("linux", "tar.gz"),
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        _ => "x64",
    };
    let url = format!(
        "https://api.adoptium.net/v3/binary/latest/{major}/ga/{os}/{arch}/jre/hotspot/normal/eclipse?project=jdk"
    );

    tokio::fs::create_dir_all(runtimes).await?;
    let archive = runtimes.join(format!("temurin-{major}.{ext}"));
    rep.step(STEP, TITLE, format!("Downloading Java {major} (only needed once)"), Some(0.0));
    download(&url, &archive, None, |done, total| rep.bytes(STEP, TITLE, done, total))
        .await
        .map_err(|e| {
            e.context(crate::errors::Friendly::network(
                "Java couldn't be downloaded",
                "Minecraft needs Java, and the launcher couldn't download it. Check your internet connection and try again — or, if you already have Java installed, choose it under Settings → Java.",
            ))
        })?;

    rep.step(STEP, TITLE, format!("Unpacking Java {major}"), None);
    let staging = runtimes.join(format!("temurin-{major}.unpacking"));
    let (archive_c, staging_c, zip) = (archive.clone(), staging.clone(), ext == "zip");
    tokio::task::spawn_blocking(move || -> Result<()> {
        let _ = std::fs::remove_dir_all(&staging_c);
        std::fs::create_dir_all(&staging_c)?;
        let file = std::fs::File::open(&archive_c)?;
        if zip {
            zip::ZipArchive::new(file)?.extract(&staging_c)?;
        } else {
            tar::Archive::new(flate2::read::GzDecoder::new(file)).unpack(&staging_c)?;
        }
        Ok(())
    })
    .await?
    .friendly(
        "Java couldn't be unpacked",
        "The Java download looks damaged. Press Retry to download it again. If your antivirus quarantined it, allow the launcher's folder.",
    )?;

    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::rename(&staging, &dir).await?;
    let _ = tokio::fs::remove_file(&archive).await;

    let bin = find_java(&dir).ok_or_else(|| {
        friendly(
            "Java install looks incomplete",
            "Java was downloaded but its program file is missing. Press Retry to reinstall it.",
        )
    })?;
    rep.step(STEP, TITLE, format!("Java {major} is ready"), Some(1.0));
    Ok(bin)
}
