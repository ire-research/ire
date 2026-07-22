use serde::Serialize;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use tauri::State;

use crate::agent_provider::{AgentProvider, ClaudeCodeProvider, CodexProvider};
use crate::binary::BinaryStatus;
use crate::tool_cards::ToolProvider;
use crate::workspace::state::ActiveWorkspace;

/// One selectable model plus the effort levels valid for it, as reported by
/// an `AgentProvider`.
#[derive(Debug, Serialize)]
pub struct AgentModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub effort_levels: &'static [&'static str],
}

/// Capability metadata for one provider: which models it offers, which is
/// the default, and which is used for lightweight background work (chat
/// titles).
#[derive(Debug, Serialize)]
pub struct ProviderCapabilities {
    pub provider: ToolProvider,
    pub default_model: &'static str,
    pub lightweight_model: &'static str,
    pub models: Vec<AgentModelInfo>,
}

fn capabilities(agent: &dyn AgentProvider) -> ProviderCapabilities {
    ProviderCapabilities {
        provider: agent.id(),
        default_model: agent.default_model(),
        lightweight_model: agent.lightweight_model(),
        models: agent
            .models()
            .iter()
            .map(|m| AgentModelInfo {
                id: m.id,
                label: m.label,
                effort_levels: agent.effort_levels_for(m.id),
            })
            .collect(),
    }
}

#[tauri::command]
pub fn list_agent_models() -> Vec<ProviderCapabilities> {
    vec![
        capabilities(&ClaudeCodeProvider),
        capabilities(&CodexProvider),
    ]
}

/// Machine-level info that doesn't change for the lifetime of the app
/// process. Computed once on first request and cached.
#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub cpu_model: String,
    pub ram_total_gb: u32,
    pub gpu_model: Option<String>,
    pub gpu_vram_gb: Option<u32>,
    pub hostname: String,
    pub username: String,
}

/// Workspace/runtime metrics that change over time and are polled.
#[derive(Debug, Serialize)]
pub struct SystemMetrics {
    pub git_branch: String,
    pub git_insertions: u32,
    pub git_deletions: u32,
    pub cpu_usage_pct: f32,
    pub gpu_usage_pct: Option<f32>,
    pub claude_binary: BinaryStatus,
    pub codex_binary: BinaryStatus,
}

#[derive(Default)]
pub struct SystemInfoCache(pub Mutex<Option<SystemInfo>>);

/// Long-lived sysinfo handle for CPU usage. Kept alive across polls so each
/// `get_system_metrics` call can compute usage from the delta since the last
/// refresh, instead of blocking on `MINIMUM_CPU_UPDATE_INTERVAL` every time.
#[derive(Clone)]
pub struct CpuMonitor(Arc<Mutex<System>>);

impl Default for CpuMonitor {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(System::new_with_specifics(
            RefreshKind::new().with_cpu(CpuRefreshKind::everything()),
        ))))
    }
}

#[tauri::command]
pub async fn get_system_info(cache: State<'_, SystemInfoCache>) -> Result<SystemInfo, String> {
    if let Some(info) = cache.0.lock().map_err(|e| e.to_string())?.clone() {
        return Ok(info);
    }

    let info = tauri::async_runtime::spawn_blocking(collect_system_info)
        .await
        .map_err(|e| e.to_string())?;

    *cache.0.lock().map_err(|e| e.to_string())? = Some(info.clone());
    Ok(info)
}

#[tauri::command]
pub async fn get_system_metrics(
    active: State<'_, ActiveWorkspace>,
    cache: State<'_, SystemInfoCache>,
    cpu_monitor: State<'_, CpuMonitor>,
) -> Result<SystemMetrics, String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };

    let has_gpu = cache
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .is_some_and(|info| info.gpu_model.is_some());

    let cpu_monitor = cpu_monitor.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        collect_system_metrics(&workspace_path, has_gpu, &cpu_monitor)
    })
    .await
    .map_err(|e| e.to_string())
}

fn collect_system_info() -> SystemInfo {
    let mut sys =
        System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));
    sys.refresh_memory();
    let cpu_model = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());
    let ram_total_gb = (sys.total_memory() / 1_073_741_824) as u32;

    let (gpu_model, _gpu_usage_pct, gpu_vram_gb) = query_nvidia_smi();

    let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".to_string());

    SystemInfo {
        cpu_model,
        ram_total_gb,
        gpu_model,
        gpu_vram_gb,
        hostname,
        username,
    }
}

fn collect_system_metrics(
    workspace_path: &Path,
    has_gpu: bool,
    cpu_monitor: &CpuMonitor,
) -> SystemMetrics {
    // Git branch: symbolic-ref works even on a fresh repo with no commits;
    // rev-parse is only reached in detached-HEAD state.
    let git_branch = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(workspace_path)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .current_dir(workspace_path)
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "HEAD".to_string())
        });

    let (git_insertions, git_deletions) = git_diff_stat(workspace_path);

    let cpu_usage_pct = {
        let mut sys = cpu_monitor.0.lock().unwrap_or_else(|e| e.into_inner());
        sys.refresh_cpu_all();
        sys.global_cpu_usage()
    };

    // Only re-query nvidia-smi for live usage if a GPU was detected at all.
    let gpu_usage_pct = if has_gpu { query_nvidia_smi().1 } else { None };

    let claude_binary = ClaudeCodeProvider.readiness();
    let codex_binary = CodexProvider.readiness();

    SystemMetrics {
        git_branch,
        git_insertions,
        git_deletions,
        cpu_usage_pct,
        gpu_usage_pct,
        claude_binary,
        codex_binary,
    }
}

fn git_diff_stat(path: &Path) -> (u32, u32) {
    let out = Command::new("git")
        .args(["diff", "--shortstat", "HEAD"])
        .current_dir(path)
        .output();
    let Ok(out) = out else { return (0, 0) };
    if !out.status.success() {
        return (0, 0);
    }
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

#[cfg(target_os = "macos")]
fn query_nvidia_smi() -> (Option<String>, Option<f32>, Option<u32>) {
    // nvidia-smi never exists on macOS; skip the spawn entirely.
    (None, None, None)
}

#[cfg(not(target_os = "macos"))]
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
