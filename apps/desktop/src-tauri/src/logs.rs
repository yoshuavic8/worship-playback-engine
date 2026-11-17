//! Structured logging: JSON lines on disk + an in-memory ring buffer for Diagnostics → Logs.

use parking_lot::Mutex;
use serde::Serialize;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub time: String,
    pub level: String,
    pub category: String,
    pub message: String,
}

struct Logger {
    file: Mutex<Option<File>>,
    ring: Mutex<VecDeque<LogEntry>>,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();
const RING: usize = 2000;

fn logger() -> &'static Logger {
    LOGGER.get_or_init(|| Logger { file: Mutex::new(None), ring: Mutex::new(VecDeque::with_capacity(RING)) })
}

pub fn init(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join("wpe.log");
    // keep the log bounded: rotate at 5 MB
    if std::fs::metadata(&path).map(|m| m.len() > 5_000_000).unwrap_or(false) {
        let _ = std::fs::rename(&path, dir.join("wpe.1.log"));
    }
    if let Ok(f) = OpenOptions::new().create(true).append(true).open(path) {
        *logger().file.lock() = Some(f);
    }
}

pub fn log(level: &str, category: &str, message: impl Into<String>) {
    let e = LogEntry {
        time: chrono::Local::now().to_rfc3339(),
        level: level.into(),
        category: category.into(),
        message: message.into(),
    };
    if cfg!(debug_assertions) {
        eprintln!("[{}] {:<11} {}", e.level, e.category, e.message);
    }
    let l = logger();
    if let Some(f) = l.file.lock().as_mut() {
        if let Ok(line) = serde_json::to_string(&e) {
            let _ = writeln!(f, "{line}");
        }
    }
    let mut r = l.ring.lock();
    if r.len() >= RING {
        r.pop_front();
    }
    r.push_back(e);
}

pub fn info(category: &str, message: impl Into<String>) {
    log("info", category, message)
}
pub fn warn(category: &str, message: impl Into<String>) {
    log("warn", category, message)
}
pub fn error(category: &str, message: impl Into<String>) {
    log("error", category, message)
}

pub fn recent(limit: usize, category: Option<&str>) -> Vec<LogEntry> {
    let r = logger().ring.lock();
    r.iter()
        .rev()
        .filter(|e| category.map(|c| c.eq_ignore_ascii_case(&e.category)).unwrap_or(true))
        .take(limit)
        .cloned()
        .collect()
}

pub mod cat {
    pub const AUDIO: &str = "Audio";
    pub const PREPARATION: &str = "Preparation";
    pub const AI: &str = "AI";
    pub const ROUTING: &str = "Routing";
    pub const PLAYBACK: &str = "Playback";
    pub const SYSTEM: &str = "System";
    pub const ERROR: &str = "Error";
}
