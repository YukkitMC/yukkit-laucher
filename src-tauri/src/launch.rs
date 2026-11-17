//! Starts Minecraft with LiquidBounce and streams its output to the Logs page.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::errors::{friendly, FriendlyExt};
use crate::installer::Installed;
use crate::minecraft::expand_args;
use crate::progress::log;
use crate::settings::Settings;

#[derive(Serialize, Clone)]
pub struct GameState {
    pub running: bool,
    pub code: Option<i32>,
}

/// Offline-mode UUID, computed the same way Minecraft servers do.
fn offline_uuid(name: &str) -> String {
    let mut bytes = md5::compute(format!("OfflinePlayer:{name}")).0;
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).simple().to_string()
}

pub async fn launch(app: &AppHandle, installed: &Installed, settings: &Settings, running: Arc<AtomicBool>) -> Result<()> {
    let plan = &installed.plan;
    let java: PathBuf = if settings.java_path.trim().is_empty() {
        plan.java.clone()
    } else {
        PathBuf::from(settings.java_path.trim())
    };
    if !java.is_file() {
        return Err(friendly(
            "Java is missing",
            "The Java program the game needs couldn't be found. Press Play again to reinstall it, or pick a different Java in Settings.",
        ));
    }
    let missing: Vec<_> = plan.classpath.iter().filter(|f| !f.is_file()).collect();
    if !missing.is_empty() {
        return Err(friendly(
            "Some game files are missing",
            "Part of Minecraft is missing from the install folder (maybe an antivirus removed it). Connect to the internet and press Play to repair it.",
        )
        .context(format!("Missing: {:?}", missing)));
    }

    let separator = if cfg!(windows) { ";" } else { ":" };
    let classpath = plan
        .classpath
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(separator);
    let username = settings.username.trim().to_string();

    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", username.clone()),
        ("version_name", plan.version_id.clone()),
        ("game_directory", plan.game_dir.to_string_lossy().into_owned()),
        ("assets_root", plan.assets_dir.to_string_lossy().into_owned()),
        ("game_assets", plan.assets_dir.to_string_lossy().into_owned()),
        ("assets_index_name", plan.asset_index.clone()),
        ("auth_uuid", offline_uuid(&username)),
        ("auth_access_token", "0".into()),
        ("auth_session", "0".into()),
        ("clientid", "0".into()),
        ("auth_xuid", "0".into()),
        ("user_type", "legacy".into()),
        ("user_properties", "{}".into()),
        ("version_type", plan.version_type.clone()),
        ("natives_directory", plan.natives_dir.to_string_lossy().into_owned()),
        ("launcher_name", "YukkitLauncher".into()),
        ("launcher_version", env!("CARGO_PKG_VERSION").into()),
        ("classpath", classpath),
        ("classpath_separator", separator.into()),
        ("library_directory", plan.libraries_dir.to_string_lossy().into_owned()),
    ]);
    let fill = |arg: &str| -> String {
        let mut out = arg.to_string();
        for (key, value) in &vars {
            out = out.replace(&format!("${{{key}}}"), value);
        }
        out
    };

    let mut args: Vec<String> = vec![
        format!("-Xmx{}M", settings.memory_mb),
        format!("-Xms{}M", settings.memory_mb.min(1024)),
        format!("-Dnet.ccbluex.liquidbounce.api.token={}", settings.session_token),
    ];
    args.extend(settings.jvm_args.split_whitespace().map(String::from));
    let jvm = expand_args(&plan.jvm_args);
    if jvm.is_empty() {
        args.push(fill("-Djava.library.path=${natives_directory}"));
        args.push("-cp".into());
        args.push(fill("${classpath}"));
    } else {
        args.extend(jvm.iter().map(|a| fill(a)));
    }
    args.push(plan.main_class.clone());
    let game = expand_args(&plan.game_args);
    if game.is_empty() {
        if let Some(legacy) = &plan.legacy_args {
            args.extend(legacy.split_whitespace().map(|a| fill(a)));
        }
    } else {
        args.extend(game.iter().map(|a| fill(a)));
    }

    tokio::fs::create_dir_all(&plan.game_dir).await?;
    log(app, &format!("Starting LiquidBounce {} with Java at {}", installed.build.lb_version, java.display()));

    let mut command = Command::new(&java);
    command
        .args(&args)
        .current_dir(&plan.game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

    let mut child = command.spawn().friendly(
        "Minecraft couldn't start",
        "Java refused to start the game. If you set a custom Java in Settings, switch back to Automatic and try again.",
    )?;

    running.store(true, Ordering::SeqCst);
    let _ = app.emit("game-state", GameState { running: true, code: None });
    if !settings.keep_open {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.minimize();
        }
    }

    if let Some(stdout) = child.stdout.take() {
        let app = app.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = app.emit("game-log", line);
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let app = app.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = app.emit("game-log", line);
            }
        });
    }

    let app = app.clone();
    tokio::spawn(async move {
        let code = child.wait().await.ok().and_then(|s| s.code());
        running.store(false, Ordering::SeqCst);
        log(&app, &format!("Minecraft closed (exit code {})", code.map_or("unknown".into(), |c| c.to_string())));
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
        }
        let _ = app.emit("game-state", GameState { running: false, code });
    });
    Ok(())
}
