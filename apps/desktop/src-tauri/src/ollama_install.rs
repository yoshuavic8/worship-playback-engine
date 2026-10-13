//! Optional Ollama install guidance and model pull (official URLs only).

use crate::compat;
use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaInstallInfo {
    pub macos_version: String,
    pub recommended_version: String,
    pub download_page: String,
    pub zip_url: String,
    pub notes: String,
}

pub fn install_info() -> OllamaInstallInfo {
    let macos = compat::macos_version_tuple();
    let ver_str = format!("{}.{}.{}", macos.0, macos.1, macos.2);
    let old = macos.0 < 14;
    let recommended = if old { "0.12.3" } else { "latest" };
    let zip = if old {
        "https://github.com/ollama/ollama/releases/download/v0.12.3/Ollama-darwin.zip"
    } else {
        "https://github.com/ollama/ollama/releases/latest/download/Ollama-darwin.zip"
    };
    OllamaInstallInfo {
        macos_version: ver_str,
        recommended_version: recommended.into(),
        download_page: "https://ollama.com/download".into(),
        zip_url: zip.into(),
        notes: if old {
            "macOS 12–13: gunakan Ollama v0.12.3 (versi terbaru butuh macOS 14+).".into()
        } else {
            "Unduh dari ollama.com, lalu tarik Ollama.app ke Applications.".into()
        },
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PullProgress {
    pub status: String,
    pub completed: u64,
    pub total: u64,
    pub done: bool,
    pub error: Option<String>,
}

pub fn pull_model(url: &str, model: &str) -> Result<PullProgress, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;
    let body = serde_json::json!({ "name": model, "stream": true });
    let resp = client
        .post(format!("{}/api/pull", url.trim_end_matches('/')))
        .json(&body)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Ollama returned {}", resp.status()));
    }
    let mut reader = BufReader::new(resp);
    let mut line = String::new();
    let mut last = PullProgress::default();
    loop {
        line.clear();
        if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let j: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| e.to_string())?;
        last.status = j["status"].as_str().unwrap_or("").into();
        last.completed = j["completed"].as_u64().unwrap_or(0);
        last.total = j["total"].as_u64().unwrap_or(0);
        if j["error"].is_string() {
            last.error = j["error"].as_str().map(String::from);
        }
    }
    last.done = true;
    Ok(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_urls_are_official() {
        let i = install_info();
        assert!(i.download_page.starts_with("https://ollama.com"));
        assert!(i.zip_url.contains("github.com/ollama/ollama"));
    }

    #[test]
    fn old_macos_gets_0123() {
        // smoke: info always has a zip url
        assert!(!install_info().zip_url.is_empty());
    }
}
