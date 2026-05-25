use serde::Serialize;
use std::process::Command;
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use tauri::State;

use crate::cc::discovery::find_claude_binary;
use crate::codex::discovery::find_codex_binary;
use crate::workspace::state::ActiveWorkspace;

#[derive(Debug, Serialize)]
pub struct SystemStatus {
    pub workspace_path: String,
    pub git_branch: String,
    pub git_insertions: u32,
    pub git_deletions: u32,
    pub cpu_model: String,
    pub cpu_usage_pct: f32,
    pub gpu_model: Option<String>,
    pub gpu_usage_pct: Option<f32>,
    pub gpu_vram_gb: Option<u32>,
    pub ram_total_gb: u32,
    pub hostname: String,
    pub username: String,
    pub cc_connected: bool,
    pub codex_connected: bool,
}

#[tauri::command]
pub fn get_system_status(active: State<'_, ActiveWorkspace>) -> Result<SystemStatus, String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };

    // Git branch
    let git_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&workspace_path)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "HEAD".to_string());

    // Git diff stat
    let (git_insertions, git_deletions) = git_diff_stat(&workspace_path);

    // CPU via sysinfo
    let mut sys =
        System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_all();
    let cpu_model = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());
    let cpu_usage_pct = sys.global_cpu_usage();

    // RAM via sysinfo
    sys.refresh_memory();
    let ram_total_gb = (sys.total_memory() / 1_073_741_824) as u32;

    // GPU via nvidia-smi (best-effort)
    let (gpu_model, gpu_usage_pct, gpu_vram_gb) = query_nvidia_smi();

    // Hostname + username
    let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".to_string());

    Ok(SystemStatus {
        workspace_path: workspace_path.to_string_lossy().into_owned(),
        git_branch,
        git_insertions,
        git_deletions,
        cpu_model,
        cpu_usage_pct,
        gpu_model,
        gpu_usage_pct,
        gpu_vram_gb,
        ram_total_gb,
        hostname,
        username,
        cc_connected: find_claude_binary().is_ok(),
        codex_connected: find_codex_binary().is_ok(),
    })
}

fn git_diff_stat(path: &std::path::Path) -> (u32, u32) {
    let out = Command::new("git")
        .args(["diff", "--shortstat", "HEAD"])
        .current_dir(path)
        .output();
    let Ok(out) = out else { return (0, 0) };
    let s = String::from_utf8_lossy(&out.stdout);
    let ins = parse_stat(&s, "insertion");
    let del = parse_stat(&s, "deletion");
    (ins, del)
}

fn parse_stat(s: &str, keyword: &str) -> u32 {
    s.split(',')
        .find(|part| part.contains(keyword))
        .and_then(|part| part.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

fn query_nvidia_smi() -> (Option<String>, Option<f32>, Option<u32>) {
    let out = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output();
    let Ok(out) = out else {
        return (None, None, None);
    };
    if !out.status.success() {
        return (None, None, None);
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let line = s.lines().next().unwrap_or("");
    let parts: Vec<&str> = line.splitn(3, ',').map(str::trim).collect();
    if parts.len() < 3 {
        return (None, None, None);
    }
    let model = Some(parts[0].to_string());
    let usage = parts[1].parse::<f32>().ok();
    let vram_mb = parts[2].parse::<u32>().ok();
    let vram_gb = vram_mb.map(|m| m / 1024);
    (model, usage, vram_gb)
}
