#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod catalog;
mod cosmetics;
mod errors;
mod installer;
mod java;
mod launch;
mod minecraft;
mod net;
mod progress;
mod settings;
mod sources;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use errors::{is_network, UiError};
use settings::Settings;

struct AppState {
    settings: Mutex<Settings>,
    busy: AtomicBool,
    running: Arc<AtomicBool>,
}

/// Makes sure only one install/launch runs at a time.
struct BusyGuard<'a>(&'a AtomicBool);

impl<'a> BusyGuard<'a> {
    fn acquire(flag: &'a AtomicBool) -> Result<Self, UiError> {
        if flag.swap(true, Ordering::SeqCst) {
            return Err(UiError::new("Already working", "The launcher is already installing or starting the game. Please wait for it to finish."));
        }
        Ok(Self(flag))
    }
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstalledView {
    toolkit_tag: String,
    fabric_api: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionView {
    id: String,
    name: String,
    minecraft: String,
    channel: String,
    description: String,
    installed: Option<InstalledView>,
    checks: installer::Checks,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Overview {
    versions: Vec<VersionView>,
    selected: String,
    latest: Option<sources::ToolkitRelease>,
    update_available: bool,
    error: Option<UiError>,
    game_running: bool,
    username: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    settings: Settings,
    install_dir: String,
    default_install_dir: String,
    version: String,
}

/// The selected version, or the newest one when nothing (or something unknown) is selected.
fn selected_id(settings: &Settings, versions: &[catalog::GameVersion]) -> String {
    versions
        .iter()
        .find(|v| v.id == settings.selected_version)
        .or_else(|| versions.first())
        .map(|v| v.id.clone())
        .unwrap_or_default()
}

async fn resolve_version(id: &str) -> Result<catalog::GameVersion, UiError> {
    catalog::find(id).await.ok_or_else(|| {
        UiError::new(
            "That version isn't available",
            "The version you picked is no longer offered. Choose another one on the Versions page.",
        )
    })
}

#[tauri::command]
async fn get_overview(state: State<'_, AppState>) -> Result<Overview, UiError> {
    let settings = state.settings.lock().unwrap().clone();
    let root = settings.install_dir();
    let list = catalog::load().await;
    let selected = selected_id(&settings, &list);

    let views = list
        .iter()
        .map(|v| {
            let installed = installer::load_installed(&root, &v.id);
            VersionView {
                id: v.id.clone(),
                name: v.name.clone(),
                minecraft: v.minecraft.clone(),
                channel: v.channel.clone(),
                description: v.description.clone(),
                checks: installer::checks(installed.as_ref(), &settings),
                installed: installed.map(|i| InstalledView { toolkit_tag: i.toolkit.tag, fabric_api: i.fabric_api }),
            }
        })
        .collect::<Vec<_>>();

    let (latest, error) = match list.iter().find(|v| v.id == selected) {
        Some(version) => match sources::latest_toolkit(version, settings.previews).await {
            Ok(r) => (Some(r), None),
            Err(e) => (None, Some(UiError::from(e))),
        },
        None => (None, None),
    };
    let update_available = match (&latest, installer::load_installed(&root, &selected)) {
        (Some(l), Some(i)) => l.tag != i.toolkit.tag,
        _ => false,
    };

    Ok(Overview {
        versions: views,
        selected,
        latest,
        update_available,
        error,
        game_running: state.running.load(Ordering::SeqCst),
        username: settings.username.clone(),
    })
}

#[tauri::command]
async fn get_patch_notes(state: State<'_, AppState>) -> Result<Vec<sources::ToolkitRelease>, UiError> {
    let settings = state.settings.lock().unwrap().clone();
    let list = catalog::load().await;
    let selected = selected_id(&settings, &list);
    let repo = list
        .iter()
        .find(|v| v.id == selected)
        .map(|v| v.toolkit_repo.clone())
        .unwrap_or_else(|| "YukkitMC/yukkit-toolkit".into());
    Ok(sources::toolkit_releases(&repo).await?)
}

#[tauri::command]
fn select_version(state: State<'_, AppState>, version_id: String) -> Result<(), UiError> {
    let mut settings = state.settings.lock().unwrap();
    settings.selected_version = version_id;
    settings.save()?;
    Ok(())
}

#[tauri::command]
async fn install(app: AppHandle, state: State<'_, AppState>, version_id: String) -> Result<(), UiError> {
    let _guard = BusyGuard::acquire(&state.busy)?;
    let settings = state.settings.lock().unwrap().clone();
    settings.validate()?;
    let version = resolve_version(&version_id).await?;
    installer::install(&app, &settings, &version).await?;
    Ok(())
}

#[tauri::command]
async fn play(app: AppHandle, state: State<'_, AppState>, version_id: String) -> Result<(), UiError> {
    if state.running.load(Ordering::SeqCst) {
        return Err(UiError::new(
            "Minecraft is already running",
            "Close the game window first if you want to start it again.",
        ));
    }
    let _guard = BusyGuard::acquire(&state.busy)?;
    let settings = state.settings.lock().unwrap().clone();
    settings.validate()?;
    let root = settings.install_dir();

    let installed = match resolve_version(&version_id).await {
        Ok(version) => match installer::install(&app, &settings, &version).await {
            Ok(installed) => installed,
            Err(err) if is_network(&err) => match installer::load_installed(&root, &version_id) {
                Some(installed) => {
                    progress::log(&app, &format!("Offline — starting the installed version instead ({:#})", err));
                    installed
                }
                None => return Err(err.into()),
            },
            Err(err) => return Err(err.into()),
        },
        Err(err) => match installer::load_installed(&root, &version_id) {
            Some(installed) => installed,
            None => return Err(err),
        },
    };
    // Hand the chosen skin and cape to the Toolkit. A failure here must not stop the game.
    if let Err(err) = cosmetics::export(&settings, &installer::instance_dir(&root, &version_id)) {
        progress::log(&app, &format!("Couldn't copy your skin and cape to the game folder: {:#}", err));
    }
    launch::launch(&app, &installed, &settings, state.running.clone()).await?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CosmeticsView {
    /// PNG bytes of the last uploaded skin / cape, if any.
    custom_skin: Option<Vec<u8>>,
    custom_cape: Option<Vec<u8>>,
}

#[tauri::command]
fn get_cosmetics() -> CosmeticsView {
    CosmeticsView {
        custom_skin: cosmetics::read_custom(cosmetics::Kind::Skin),
        custom_cape: cosmetics::read_custom(cosmetics::Kind::Cape),
    }
}

/// Opens a file picker for a skin or cape PNG, checks it and keeps it as the custom one.
/// Returns the picture so the page can preview it, or None if the picker was closed.
#[tauri::command]
async fn pick_cosmetic(app: AppHandle, kind: String) -> Result<Option<Vec<u8>>, UiError> {
    let kind = cosmetics::Kind::parse(&kind)?;
    let title = if kind == cosmetics::Kind::Skin { "Choose a skin (PNG)" } else { "Choose a cape (PNG)" };
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().set_title(title).add_filter("PNG picture", &["png"]).blocking_pick_file()
    })
    .await
    .map_err(|e| UiError::from(anyhow::Error::new(e)))?;
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(None) };
    let bytes = std::fs::read(&path).map_err(|e| UiError::from(anyhow::Error::new(e)))?;
    cosmetics::save_custom(kind, &bytes)?;
    Ok(Some(bytes))
}

/// Sets the active skin or cape picture (None clears it: default skin / no cape).
#[tauri::command]
fn set_cosmetic(kind: String, png: Option<Vec<u8>>) -> Result<(), UiError> {
    let kind = cosmetics::Kind::parse(&kind)?;
    cosmetics::set_active(kind, png.as_deref())?;
    Ok(())
}

#[tauri::command]
fn remove_version(state: State<'_, AppState>, version_id: String) -> Result<(), UiError> {
    if state.busy.load(Ordering::SeqCst) || state.running.load(Ordering::SeqCst) {
        return Err(UiError::new("Not right now", "Wait until the game is closed and nothing is installing, then try again."));
    }
    let root = state.settings.lock().unwrap().install_dir();
    installer::uninstall(&root, &version_id)?;
    Ok(())
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> SettingsView {
    let settings = state.settings.lock().unwrap().clone();
    SettingsView {
        install_dir: settings.install_dir().to_string_lossy().into_owned(),
        default_install_dir: settings::default_install_dir().to_string_lossy().into_owned(),
        version: env!("CARGO_PKG_VERSION").into(),
        settings,
    }
}

#[tauri::command]
fn save_settings(state: State<'_, AppState>, mut settings: Settings) -> Result<(), UiError> {
    let mut current = state.settings.lock().unwrap();
    settings.username = settings.username.trim().to_string();
    settings.validate()?;
    settings.save()?;
    *current = settings;
    Ok(())
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Result<Option<String>, UiError> {
    let picked = tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
        .await
        .map_err(|e| UiError::from(anyhow::Error::new(e)))?;
    Ok(picked.and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
async fn pick_java(app: AppHandle) -> Result<Option<String>, UiError> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().set_title("Choose java.exe or javaw.exe").blocking_pick_file()
    })
    .await
    .map_err(|e| UiError::from(anyhow::Error::new(e)))?;
    Ok(picked.and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
fn open_install_folder(app: AppHandle, state: State<'_, AppState>) -> Result<(), UiError> {
    let dir = state.settings.lock().unwrap().install_dir();
    std::fs::create_dir_all(&dir).map_err(|e| UiError::from(anyhow::Error::new(e)))?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| UiError::from(anyhow::Error::new(e)))
}

#[tauri::command]
fn open_link(app: AppHandle, url: String) -> Result<(), UiError> {
    if !(url.starts_with("https://")) {
        return Err(UiError::new("Link blocked", "Only secure web links can be opened."));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| UiError::from(anyhow::Error::new(e)))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            settings: Mutex::new(Settings::load()),
            busy: AtomicBool::new(false),
            running: Arc::new(AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            get_overview,
            get_patch_notes,
            select_version,
            install,
            play,
            remove_version,
            get_settings,
            save_settings,
            pick_folder,
            pick_java,
            open_install_folder,
            open_link,
            get_cosmetics,
            pick_cosmetic,
            set_cosmetic
        ])
        .run(tauri::generate_context!())
        .expect("error while running the launcher");
}
