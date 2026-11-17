//! LiquidBounce is distributed through its own download page. Like the official launcher,
//! we open that page in a window; when the player finishes it, the page sends back the id
//! of the file to download.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;
use tauri::{AppHandle, Listener, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::errors::friendly;

enum Message {
    Pid(String),
    Closed,
}

pub async fn obtain_file_id(app: &AppHandle, url: &str) -> Result<String> {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let label = format!("download_view-{}", COUNTER.fetch_add(1, Ordering::SeqCst));
    let parsed = url.parse().context("Invalid download page address")?;

    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(parsed))
        .title("Download LiquidBounce — finish this page to continue")
        .inner_size(1100.0, 760.0)
        .center()
        .focused(true);
    if let Some(main) = app.get_webview_window("main") {
        builder = builder.parent(&main)?;
    }
    let window = builder.build().context("Failed to open the download page")?;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    let tx_pid = tx.clone();
    window.once("download", move |event| {
        #[derive(Deserialize)]
        struct Payload {
            pid: String,
        }
        if let Ok(payload) = serde_json::from_str::<Payload>(event.payload()) {
            let _ = tx_pid.send(Message::Pid(payload.pid));
        }
    });

    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed) {
            let _ = tx.send(Message::Closed);
        }
    });

    let result = tokio::time::timeout(Duration::from_secs(15 * 60), rx.recv()).await;
    let _ = window.destroy();

    match result {
        Ok(Some(Message::Pid(pid))) => Ok(pid),
        Ok(Some(Message::Closed)) | Ok(None) => Err(friendly(
            "Download page was closed",
            "LiquidBounce is downloaded through its official download page, which keeps the client free. Press Retry and follow the page until it says the download has started.",
        )),
        Err(_) => Err(friendly(
            "Download page timed out",
            "The LiquidBounce download page was open for a long time without finishing. Press Retry to open it again.",
        )),
    }
}
