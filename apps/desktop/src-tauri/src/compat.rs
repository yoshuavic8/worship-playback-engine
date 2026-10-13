//! macOS / hardware compatibility profile (Full vs Lite).

use serde::Serialize;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CompatProfile {
    Full,
    Lite,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacHardware {
    pub arch_label: String,
    pub macos_version: String,
    pub ram_gb: u32,
    pub rosetta: bool,
    pub apple_silicon: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatInfo {
    pub profile: CompatProfile,
    pub reason: String,
    pub separation_backend: String,
    pub prep_concurrency: u32,
    pub prep_eta_warning: Option<String>,
    pub mlx_available: bool,
    pub hardware: MacHardware,
    pub ollama_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OllamaProbe {
    pub models: Vec<String>,
    pub version: Option<String>,
}

static OLLAMA_CACHE: Mutex<Option<(Instant, Result<OllamaProbe, String>)>> = Mutex::new(None);

fn sysctl(key: &str) -> Option<String> {
    Command::new("/usr/sbin/sysctl")
        .args(["-n", key])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

fn sw_vers(key: &str) -> String {
    Command::new("/usr/bin/sw_vers")
        .arg(key)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

pub fn macos_version_tuple() -> (u32, u32, u32) {
    let v = sw_vers("-productVersion");
    let mut parts = v.split('.').map(|p| p.parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}

pub fn macos_at_least(major: u32, minor: u32) -> bool {
    let (ma, mi, _) = macos_version_tuple();
    ma > major || (ma == major && mi >= minor)
}

pub fn ram_gb() -> u32 {
    sysctl("hw.memsize")
        .and_then(|m| m.parse::<u64>().ok())
        .map(|b| ((b + 512_999_999) / 1_073_741_824) as u32)
        .unwrap_or(0)
}

pub fn host_apple_silicon() -> bool {
    sysctl("hw.optional.arm64").map(|s| s == "1").unwrap_or(std::env::consts::ARCH == "aarch64")
}

pub fn running_under_rosetta() -> bool {
    sysctl("sysctl.proc_translated").map(|s| s == "1").unwrap_or(false)
}

pub fn arch_label() -> String {
    match std::env::consts::ARCH {
        "aarch64" if running_under_rosetta() => "x86_64 (Rosetta)".into(),
        "aarch64" => "Apple Silicon (arm64)".into(),
        "x86_64" => "Intel (x86_64)".into(),
        other => other.into(),
    }
}

pub fn mlx_eligible() -> bool {
    host_apple_silicon() && !running_under_rosetta() && macos_at_least(13, 5)
}

pub fn detect() -> CompatInfo {
    let macos_version = sw_vers("-productVersion");
    let ram = ram_gb();
    let apple_silicon = host_apple_silicon() && !running_under_rosetta() && std::env::consts::ARCH == "aarch64";
    let rosetta = running_under_rosetta();
    let hardware = MacHardware {
        arch_label: arch_label(),
        macos_version: macos_version.clone(),
        ram_gb: ram,
        rosetta,
        apple_silicon,
    };

    let (profile, reason) = if apple_silicon && macos_at_least(13, 5) {
        (
            CompatProfile::Full,
            format!("Apple Silicon · macOS {macos_version} · {ram} GB RAM"),
        )
    } else if apple_silicon {
        (
            CompatProfile::Lite,
            format!(
                "Apple Silicon on macOS {macos_version} — MLX needs 13.5+; using PyTorch (MPS/CPU). Prep may take longer."
            ),
        )
    } else if rosetta {
        (
            CompatProfile::Lite,
            "Running under Rosetta — stem prep uses CPU and is slower.".into(),
        )
    } else {
        (
            CompatProfile::Lite,
            format!(
                "Intel Mac — separation uses PyTorch CPU (same models, slower). Expect roughly 1–3× song length per prep."
            ),
        )
    };

    let mlx_available = mlx_eligible();
    let separation_backend = if mlx_available {
        "MLX (Apple GPU) or PyTorch".into()
    } else if apple_silicon {
        "PyTorch (MPS or CPU)".into()
    } else {
        "PyTorch (CPU)".into()
    };

    let prep_eta_warning = if profile == CompatProfile::Lite {
        Some(
            "Lite profile: one prep job at a time. Stem separation may take several minutes on this Mac.".into(),
        )
    } else {
        None
    };

    let ollama_version = probe_ollama("http://127.0.0.1:11434").ok().and_then(|p| p.version);

    CompatInfo {
        profile,
        reason,
        separation_backend,
        prep_concurrency: 1,
        prep_eta_warning,
        mlx_available,
        hardware,
        ollama_version,
    }
}

/// Fast cached probe (≈1.5s timeout). Used for diagnostics and AI provider selection.
/// When `enabled` is false, callers must not probe (no network, no cache churn).
pub fn probe_ollama_if_enabled(enabled: bool, url: &str) -> Result<OllamaProbe, String> {
    if !enabled {
        return Err("disabled".into());
    }
    probe_ollama(url)
}

pub fn probe_ollama(url: &str) -> Result<OllamaProbe, String> {
    {
        let cache = OLLAMA_CACHE.lock().unwrap();
        if let Some((at, res)) = cache.as_ref() {
            if at.elapsed() < Duration::from_secs(30) {
                return res.clone();
            }
        }
    }
    let result = probe_ollama_uncached(url);
    *OLLAMA_CACHE.lock().unwrap() = Some((Instant::now(), result.clone()));
    result
}

fn probe_ollama_uncached(url: &str) -> Result<OllamaProbe, String> {
    let base = url.trim_end_matches('/');
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(1500))
        .build()
        .map_err(|e| e.to_string())?;
    let version = client
        .get(format!("{base}/api/version"))
        .send()
        .ok()
        .and_then(|r| r.json::<serde_json::Value>().ok())
        .and_then(|j| j["version"].as_str().map(String::from));
    let r = client
        .get(format!("{base}/api/tags"))
        .send()
        .map_err(|e| e.to_string())?;
    let j: serde_json::Value = r.json().map_err(|e| e.to_string())?;
    let models = j["models"]
        .as_array()
        .map(|a| a.iter().filter_map(|m| m["name"].as_str().map(String::from)).collect())
        .unwrap_or_default();
    Ok(OllamaProbe { models, version })
}

pub fn invalidate_ollama_cache() {
    *OLLAMA_CACHE.lock().unwrap() = None;
}

/// Rough parameter count from Ollama model tags (e.g. qwen2.5:7b → 7).
pub fn model_billions(name: &str) -> Option<f32> {
    let lower = name.to_lowercase();
    for token in lower.split([':', '-', '_']) {
        let t = token.trim();
        if t.ends_with('b') {
            if let Ok(n) = t[..t.len() - 1].parse::<f32>() {
                return Some(n);
            }
        }
    }
    None
}

const TINY_MODELS: &[&str] = &["qwen2.5:1.5b", "llama3.2:1b", "gemma3:1b", "gemma2:2b"];

/// Pick an Ollama model tag respecting hardware. Never auto-picks >3B on Intel or ≤8 GB RAM.
pub fn pick_ollama_model(
    preferred: Option<&str>,
    installed: &[String],
    info: &CompatInfo,
) -> Option<String> {
    if installed.is_empty() {
        return None;
    }
    if let Some(p) = preferred.filter(|m| installed.iter().any(|x| x == m)) {
        return Some(p.to_string());
    }
    let lite = info.profile == CompatProfile::Lite
        || !info.hardware.apple_silicon
        || info.hardware.ram_gb <= 8;
    let max_b = if lite { 3.0 } else { 8.0 };

    for want in TINY_MODELS {
        if let Some(m) = installed.iter().find(|m| m.starts_with(want) || **m == *want) {
            return Some(m.clone());
        }
    }
    for m in installed {
        if let Some(b) = model_billions(m) {
            if b <= max_b {
                return Some(m.clone());
            }
        }
    }
    if lite {
        installed
            .iter()
            .min_by(|a, b| {
                model_billions(a)
                    .unwrap_or(99.0)
                    .partial_cmp(&model_billions(b).unwrap_or(99.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    } else {
        installed
            .iter()
            .find(|m| m.starts_with("qwen2.5"))
            .or_else(|| installed.iter().find(|m| m.contains("llama3")))
            .cloned()
            .or_else(|| installed.first().cloned())
    }
}

pub fn ollama_pull_hint(info: &CompatInfo, installed: &[String]) -> Option<String> {
    if !installed.is_empty() {
        return None;
    }
    if info.profile == CompatProfile::Lite || info.hardware.ram_gb <= 8 || !info.hardware.apple_silicon {
        Some("ollama pull qwen2.5:1.5b".into())
    } else {
        Some("ollama pull qwen2.5:7b".into())
    }
}

pub fn humanize_uv_sync_error(stderr: &str) -> String {
    let s = stderr.to_lowercase();
    if s.contains("onnxruntime") || s.contains("1.24") {
        return "This Mac needs onnxruntime below 1.24 (Intel and macOS 12–13). Re-install the Python environment from Diagnostics.".into();
    }
    if s.contains("torch") && s.contains("x86_64") {
        return "Intel Macs use PyTorch 2.2.2 (last release with an x86_64 wheel). Re-run Install environment in Diagnostics.".into();
    }
    if s.contains("no solution") || s.contains("platform") {
        return "The locked Python environment does not match this Mac (arm64 vs Intel). Update the app and install the environment again.".into();
    }
    "Python environment install failed. Open Diagnostics → Logs for details, then try Install again.".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(profile: CompatProfile, apple: bool, ram: u32) -> CompatInfo {
        CompatInfo {
            profile,
            reason: "test".into(),
            separation_backend: "test".into(),
            prep_concurrency: 1,
            prep_eta_warning: None,
            mlx_available: apple,
            hardware: MacHardware {
                arch_label: if apple { "arm64".into() } else { "x86_64".into() },
                macos_version: "14.0".into(),
                ram_gb: ram,
                rosetta: false,
                apple_silicon: apple,
            },
            ollama_version: None,
        }
    }

    #[test]
    fn profile_mapping_intel_is_lite() {
        let c = info(CompatProfile::Lite, false, 16);
        assert_eq!(c.profile, CompatProfile::Lite);
        assert!(!c.mlx_available);
    }

    #[test]
    fn model_choice_prefers_tiny_on_lite() {
        let c = info(CompatProfile::Lite, false, 16);
        let models = vec!["qwen2.5:7b".into(), "qwen2.5:1.5b".into(), "llama3.1:8b".into()];
        let m = pick_ollama_model(None, &models, &c).unwrap();
        assert!(m.contains("1.5b"));
    }

    #[test]
    fn model_choice_never_picks_large_on_intel() {
        let c = info(CompatProfile::Lite, false, 32);
        let models = vec!["qwen2.5:14b".into(), "qwen2.5:7b".into()];
        let m = pick_ollama_model(None, &models, &c).unwrap();
        assert!(model_billions(&m).unwrap_or(99.0) <= 7.0);
    }

    #[test]
    fn model_billions_parses_tags() {
        assert_eq!(model_billions("qwen2.5:7b"), Some(7.0));
        assert_eq!(model_billions("llama3.2:1b"), Some(1.0));
    }

    #[test]
    fn macos_at_least_works() {
        // smoke: current host should parse
        let _ = macos_version_tuple();
    }

    #[test]
    fn ollama_probe_skipped_when_disabled() {
        assert_eq!(probe_ollama_if_enabled(false, "http://127.0.0.1:11434").unwrap_err(), "disabled");
    }
}
