//! System readiness / dependency detection for Diagnostics.

use crate::core::Ctx;
use crate::paths::find_tool;
use serde::Serialize;
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: String,
    pub label: String,
    /// ok | warning | missing | optional
    pub status: String,
    pub detail: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Readiness {
    pub checks: Vec<Check>,
    pub playback: String,
    pub preparation: String,
    pub ai: String,
    pub ollama_models: Vec<String>,
    pub app_version: String,
    pub data_dir: String,
    pub library_dir: String,
    pub schema_version: i64,
    pub disk_usage: serde_json::Value,
}

fn check(id: &str, label: &str, status: &str, detail: impl Into<String>, hint: Option<&str>) -> Check {
    Check { id: id.into(), label: label.into(), status: status.into(), detail: detail.into(), hint: hint.map(String::from) }
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

pub fn ollama_models(url: &str) -> Result<Vec<String>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(1500))
        .build()
        .map_err(|e| e.to_string())?;
    let r = client.get(format!("{}/api/tags", url.trim_end_matches('/'))).send().map_err(|e| e.to_string())?;
    let j: serde_json::Value = r.json().map_err(|e| e.to_string())?;
    Ok(j["models"]
        .as_array()
        .map(|a| a.iter().filter_map(|m| m["name"].as_str().map(String::from)).collect())
        .unwrap_or_default())
}

pub fn readiness(ctx: &Ctx) -> Readiness {
    let paths = ctx.paths.read().clone();
    let settings = ctx.settings.read().clone();
    let mut checks = vec![];

    let arm = std::env::consts::ARCH == "aarch64";
    let chip = run("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"]).unwrap_or_default();
    checks.push(check(
        "silicon",
        "Apple Silicon",
        if arm { "ok" } else { "warning" },
        if arm { format!("{chip} detected") } else { "Intel Mac — separation will be slow".into() },
        None,
    ));
    let mem = run("/usr/sbin/sysctl", &["-n", "hw.memsize"])
        .and_then(|m| m.parse::<u64>().ok())
        .map(|b| format!("{} GB unified memory", b / 1_073_741_824))
        .unwrap_or_default();
    let os = run("/usr/bin/sw_vers", &["-productVersion"]).unwrap_or_default();
    checks.push(check("macos", "macOS", "ok", format!("macOS {os} · {mem}"), None));

    let audio_ok = ctx.engine.running();
    checks.push(match &audio_ok {
        Some(r) => check(
            "audio",
            "Audio engine",
            "ok",
            format!("{} · {} Hz · {} outputs", r.device_name, r.sample_rate, r.channels),
            None,
        ),
        None => check(
            "audio",
            "Audio engine",
            "missing",
            ctx.engine.last_error().unwrap_or_else(|| "Not running".into()),
            Some("Open Devices and select an output."),
        ),
    });

    let ffmpeg = find_tool("ffmpeg");
    checks.push(match &ffmpeg {
        Some(p) => {
            let v = run(&p.to_string_lossy(), &["-version"])
                .and_then(|o| o.lines().next().map(|l| l.split_whitespace().take(3).collect::<Vec<_>>().join(" ")))
                .unwrap_or_default();
            check("ffmpeg", "FFmpeg", "ok", v, None)
        }
        None => check("ffmpeg", "FFmpeg", "missing", "Not found", Some("brew install ffmpeg")),
    });

    let uv = find_tool("uv");
    checks.push(match &uv {
        Some(p) => check("uv", "uv (Python manager)", "ok", run(&p.to_string_lossy(), &["--version"]).unwrap_or_default(), None),
        None => check("uv", "uv (Python manager)", "missing", "Not found", Some("brew install uv")),
    });

    let py = paths.python_bin();
    let env_ok = py.exists();
    let probe = if env_ok {
        run(
            &py.to_string_lossy(),
            &[
                "-c",
                "import json,importlib.util as u,sys;r={'python':sys.version.split()[0]};\nfor m in ['demucs','torch','librosa','demucs_mlx','mlx']: r[m]=u.find_spec(m) is not None\ntry:\n import torch;r['mps']=bool(torch.backends.mps.is_available());r['torchVersion']=torch.__version__\nexcept Exception: r['mps']=False\nprint(json.dumps(r))",
            ],
        )
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    } else {
        None
    };
    checks.push(match &probe {
        Some(p) => check(
            "python",
            "Python environment",
            if p["librosa"] == true { "ok" } else { "warning" },
            format!("Python {} · {}", p["python"].as_str().unwrap_or("?"), paths.python_env().display()),
            None,
        ),
        None => check(
            "python",
            "Python environment",
            "missing",
            if env_ok { "Environment is broken".into() } else { "Not installed".to_string() },
            Some("Click Install environment (≈1 GB download, one time)."),
        ),
    });
    let has = |k: &str| probe.as_ref().map(|p| p[k] == true).unwrap_or(false);
    checks.push(check(
        "demucs",
        "Demucs (PyTorch)",
        if has("demucs") { "ok" } else { "missing" },
        if has("demucs") {
            format!(
                "torch {} · GPU (MPS) {}",
                probe.as_ref().and_then(|p| p["torchVersion"].as_str()).unwrap_or("?"),
                if has("mps") { "available" } else { "unavailable" }
            )
        } else {
            "Not installed".into()
        },
        None,
    ));
    checks.push(check(
        "mlx",
        "MLX (Apple Silicon)",
        if has("demucs_mlx") && has("mlx") { "ok" } else { "optional" },
        if has("demucs_mlx") { "demucs-mlx available".to_string() } else { "Not installed".into() },
        None,
    ));
    let model = &settings.separation_model;
    let engines = ctx.resources.installed_engines(&paths, model);
    checks.push(check(
        "model",
        "Separation model",
        if engines.is_empty() { "missing" } else { "ok" },
        if engines.is_empty() { format!("{model} not downloaded") } else { format!("{model} · {}", engines.join(", ")) },
        Some("Download a model in AI Models below."),
    ));
    if settings.separation_profile == "worship" {
        let mdx = ctx.resources.model_installed(&paths, "inst-hq-3");
        checks.push(check(
            "mdx",
            "Worship vocal model",
            if mdx { "ok" } else { "missing" },
            if mdx {
                "UVR-MDX-NET Inst HQ 3".to_string()
            } else {
                "Not downloaded — live vocal removal will use Demucs only".to_string()
            },
            Some("Download UVR-MDX-NET Inst HQ 3 below."),
        ));
    }

    let ollama = if settings.ollama_enabled { ollama_models(&settings.ollama_url) } else { Err("disabled".into()) };
    let ollama_list = ollama.clone().unwrap_or_default();
    checks.push(match &ollama {
        Ok(m) if !m.is_empty() => {
            let sel = settings.ollama_model.clone().filter(|s| m.contains(s)).unwrap_or_else(|| m[0].clone());
            check("ollama", "Ollama (optional)", "ok", format!("{} models · using {sel}", m.len()), None)
        }
        Ok(_) => check("ollama", "Ollama (optional)", "optional", "Running, but no models pulled", Some("ollama pull qwen2.5:7b")),
        Err(e) if e == "disabled" => check("ollama", "Ollama (optional)", "optional", "Disabled in Settings", None),
        Err(_) => check("ollama", "Ollama (optional)", "optional", "Unavailable — the app works fine without it", None),
    });

    let playback = if audio_ok.is_some() { "READY" } else { "NOT READY" };
    let preparation = if ffmpeg.is_some() && has("librosa") && !engines.is_empty() {
        "READY"
    } else if ffmpeg.is_some() && has("librosa") {
        "ANALYSIS ONLY"
    } else {
        "SETUP NEEDED"
    };
    let ai = if ollama.map(|m| !m.is_empty()).unwrap_or(false) { "READY" } else { "OPTIONAL" };

    Readiness {
        checks,
        playback: playback.into(),
        preparation: preparation.into(),
        ai: ai.into(),
        ollama_models: ollama_list,
        app_version: env!("CARGO_PKG_VERSION").into(),
        data_dir: paths.root.to_string_lossy().into(),
        library_dir: paths.songs().to_string_lossy().into(),
        schema_version: crate::db::schema_version(&ctx.db),
        disk_usage: serde_json::json!({
            "songs": crate::resources::dir_size(&paths.songs()),
            "models": crate::resources::dir_size(&paths.models()),
            "pythonEnv": crate::resources::dir_size(&paths.python_env()),
            "cues": crate::resources::dir_size(&paths.cues()),
        }),
    }
}
