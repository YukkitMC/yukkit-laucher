//! HTTP helpers: JSON requests and verified, retried file downloads.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

pub fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("YukkitLauncher/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .build()
            .expect("failed to build HTTP client")
    })
}

pub async fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let text = http()
        .get(url)
        .send()
        .await
        .with_context(|| format!("Request to {url} failed"))?
        .error_for_status()?
        .text()
        .await?;
    serde_json::from_str(&text).with_context(|| format!("Unexpected answer from {url}"))
}

/// A file that exists, is not empty and (when known) has the expected size.
pub fn file_ok(path: &Path, size: Option<u64>) -> bool {
    match std::fs::metadata(path) {
        Ok(meta) => meta.is_file() && meta.len() > 0 && size.map_or(true, |s| s == meta.len()),
        Err(_) => false,
    }
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

/// Downloads `url` to `dest`, checking the SHA-1 when given. Retries up to three times.
pub async fn download(
    url: &str,
    dest: &Path,
    sha1: Option<&str>,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<()> {
    let mut last_error = None;
    for attempt in 0u64..3 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(800 * attempt)).await;
        }
        match download_once(url, dest, sha1, &mut on_progress).await {
            Ok(()) => return Ok(()),
            Err(err) => last_error = Some(err),
        }
    }
    Err(last_error.expect("at least one attempt")).with_context(|| format!("Failed to download {url}"))
}

async fn download_once(
    url: &str,
    dest: &Path,
    sha1: Option<&str>,
    on_progress: &mut impl FnMut(u64, Option<u64>),
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let response = http().get(url).send().await?.error_for_status()?;
    let total = response.content_length();

    let tmp = part_path(dest);
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut hasher = Sha1::new();
    let mut done = 0u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        done += chunk.len() as u64;
        on_progress(done, total);
    }
    file.flush().await?;
    drop(file);

    if let Some(expected) = sha1 {
        let actual = format!("{:x}", hasher.finalize());
        if !actual.eq_ignore_ascii_case(expected) {
            let _ = tokio::fs::remove_file(&tmp).await;
            bail!("Checksum mismatch for {url}: expected {expected}, got {actual}");
        }
    }

    if dest.exists() {
        let _ = tokio::fs::remove_file(dest).await;
    }
    tokio::fs::rename(&tmp, dest).await?;
    Ok(())
}
