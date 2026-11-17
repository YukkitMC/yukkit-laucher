#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod download_page;
mod errors;
mod installer;
mod java;
mod launch;
mod minecraft;
mod net;
mod progress;
mod settings;

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
struct BuildView {
    build_id: u32,
    lb_version: String,
    mc_version: String,
    release: bool,
    date: String,
    message: String,
    commit: String,
}

impl From<&api::Build> for BuildView {
    fn from(b: &api::Build) -> Self {
        let date = b.date.as_str().map(|d| d.chars().take(10).collect()).unwrap_or_default();
        BuildView {
            build_id: b.build_id,
            lb_version: b.lb_version.clone(),
            mc_version: b.mc_version.clone(),
            release: b.release,
            date,
            message: b.message.lines().next().unwrap_or_default().to_string(),
            commit: b.commit_id.chars().take(7).collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Overview {
    latest: Option<BuildView>,
    api_error: Option<UiError>,
    installed: Option<BuildView>,
    checks: installer::Checks,
    update_available: bool,
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

#[tauri::command]
async fn get_overview(state: State<'_, AppState>) -> Result<Overview, UiError> {
    let settings = state.settings.lock().unwrap().clone();
    let installed = installer::load_installed(&settings.install_dir());

    let latest = async {
        let api = api::Api::connect(&settings.session_token).await?;
        api.latest_build(settings.nightly).await
    }
    .await;

    let (latest, api_error) = match latest {
        Ok(b) => (Some(b), None),
        Err(e) => (None, Some(UiError::from(e))),
    };
    let update_available = match (&latest, &installed) {
        (Some(l), Some(i)) => l.build_id != i.build.build_id,
        _ => false,
    };
    Ok(Overview {
        latest: latest.as_ref().map(BuildView::from),
        api_error,
        installed: installed.as_ref().map(|i| BuildView::from(&i.build)),
        checks: installer::checks(installed.as_ref(), &settings),
        update_available,
        game_running: state.running.load(Ordering::SeqCst),
        username: settings.username.clone(),
    })
}

#[tauri::command]
async fn get_changelog(state: State<'_, AppState>, build_id: u32) -> Result<String, UiError> {
    let token = state.settings.lock().unwrap().session_token.clone();
    let api = api::Api::connect(&token).await?;
    Ok(api.changelog(build_id).await?)
}

#[tauri::command]
async fn install(app: AppHandle, state: State<'_, AppState>) -> Result<(), UiError> {
    let _guard = BusyGuard::acquire(&state.busy)?;
    let settings = state.settings.lock().unwrap().clone();
    settings.validate()?;
    installer::install(&app, &settings).await?;
    Ok(())
}

#[tauri::command]
async fn play(app: AppHandle, state: State<'_, AppState>) -> Result<(), UiError> {
    if state.running.load(Ordering::SeqCst) {
        return Err(UiError::new("LiquidBounce is already running", "Close the game window first if you want to start it again."));
    }
    let _guard = BusyGuard::acquire(&state.busy)?;
    let settings = state.settings.lock().unwrap().clone();
    settings.validate()?;

    let installed = match installer::install(&app, &settings).await {
        Ok(installed) => installed,
        Err(err) if is_network(&err) => match installer::load_installed(&settings.install_dir()) {
            Some(installed) => {
                progress::log(&app, &format!("Offline — starting the installed version instead ({:#})", err));
                installed
            }
            None => return Err(err.into()),
        },
        Err(err) => return Err(err.into()),
    };
    launch::launch(&app, &installed, &settings, state.running.clone()).await?;
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
    settings.session_token = current.session_token.clone();
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
            get_changelog,
            install,
            play,
            get_settings,
            save_settings,
            pick_folder,
            pick_java,
            open_install_folder,
            open_link
        ])
        .run(tauri::generate_context!())
        .expect("error while running the launcher");
}
