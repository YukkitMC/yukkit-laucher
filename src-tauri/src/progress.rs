//! Sends install progress and log lines to the window.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const STEPS: u8 = 6;

#[derive(Serialize, Clone)]
pub struct ProgressEvent {
    pub step: u8,
    pub steps: u8,
    pub title: String,
    pub detail: String,
    /// 0.0 – 1.0, or `None` while the size of the work is unknown.
    pub fraction: Option<f64>,
}

#[derive(Clone)]
pub struct Reporter {
    app: AppHandle,
    last: Arc<Mutex<Instant>>,
}

impl Reporter {
    pub fn new(app: AppHandle) -> Self {
        Self { app, last: Arc::new(Mutex::new(Instant::now())) }
    }

    /// Always delivered: use for step changes.
    pub fn step(&self, step: u8, title: &str, detail: impl Into<String>, fraction: Option<f64>) {
        let detail = detail.into();
        self.log(&format!("[{step}/{STEPS}] {title} — {detail}"));
        self.send(step, title, detail, fraction);
    }

    /// Throttled: use for frequent updates inside a step.
    pub fn update(&self, step: u8, title: &str, detail: impl Into<String>, fraction: Option<f64>) {
        {
            let mut last = self.last.lock().unwrap();
            if last.elapsed() < Duration::from_millis(80) && fraction.map_or(true, |f| f > 0.0 && f < 1.0) {
                return;
            }
            *last = Instant::now();
        }
        self.send(step, title, detail.into(), fraction);
    }

    pub fn bytes(&self, step: u8, title: &str, done: u64, total: Option<u64>) {
        let mb = |b: u64| b as f64 / 1_048_576.0;
        match total {
            Some(t) if t > 0 => self.update(
                step,
                title,
                format!("{:.1} MB of {:.1} MB", mb(done), mb(t)),
                Some(done as f64 / t as f64),
            ),
            _ => self.update(step, title, format!("{:.1} MB downloaded", mb(done)), None),
        }
    }

    fn send(&self, step: u8, title: &str, detail: String, fraction: Option<f64>) {
        let _ = self.app.emit(
            "progress",
            ProgressEvent { step, steps: STEPS, title: title.to_string(), detail, fraction },
        );
    }

    pub fn log(&self, line: &str) {
        log(&self.app, line);
    }
}

pub fn log(app: &AppHandle, line: &str) {
    let _ = app.emit("launcher-log", line.to_string());
}
