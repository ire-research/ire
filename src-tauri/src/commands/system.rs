use serde::Serialize;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use tauri::State;

use crate::agent_provider::{self, AgentProvider, ModelCatalog, ModelCatalogStatus};
use crate::commands::workspace::ProviderReadiness;
use crate::tool_cards::ToolProvider;
use crate::workspace::state::ActiveWorkspace;

/// Capability metadata for one provider: its model catalog status (see
/// `ModelCatalogStatus` — distinguishes "nothing configured" from "catalog
/// discovery failed") and the default model (its catalog's first entry, if
/// the catalog resolved with at least one model). This is independent of
/// the provider being unavailable (see `AgentProvider::readiness`).
#[derive(Debug, Serialize)]
pub struct ProviderCapabilities {
    pub provider: ToolProvider,
    pub default_model: Option<String>,
    pub catalog: ModelCatalogStatus,
}

fn capabilities(agent: &dyn AgentProvider, catalog: Option<&dyn ModelCatalog>) -> ProviderCapabilities {
    let status = match catalog.map(ModelCatalog::discover_models) {
        Some(Ok(models)) => ModelCatalogStatus::Available { models },
        Some(Err(e)) => {
            tracing::warn!(provider = agent.name(), error = %e, "model discovery failed");
            ModelCatalogStatus::Error {
                message: e.to_string(),
            }
        }
        None => ModelCatalogStatus::Available { models: Vec::new() },
    };
    let default_model = match &status {
        ModelCatalogStatus::Available { models } => models.first().map(|m| m.id.clone()),
        ModelCatalogStatus::Error { .. } => None,
    };
    ProviderCapabilities {
        provider: agent.id(),
        default_model,
        catalog: status,
    }
}

#[tauri::command]
pub fn list_agent_models() -> Vec<ProviderCapabilities> {
    crate::agent_provider::all()
        .map(|(agent, catalog)| capabilities(agent, catalog))
        .collect()
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
    pub providers: Vec<ProviderReadiness>,
}

#[derive(Default)]
pub struct SystemInfoCache(pub Mutex<Option<SystemInfo>>);

/// `get_system_metrics` is polled every 5s for CPU/GPU/git stats
/// (`useSystemStatus.ts`'s `useSystemMetrics`), but provider readiness
/// (`AgentProvider::readiness`) spawns a real subprocess per provider
/// (a login/auth-status check, and `which <bin>` in discovery) — running
/// that on every 5s tick is constant background subprocess churn for data
/// that changes rarely (a user logging into/out of a provider is a
/// deliberate, occasional action). Cached with a TTL well above the poll
/// interval so it's rechecked roughly every 30s instead of every 5s, while
/// CPU/GPU/git stay on the fast cadence. `setup_status` (user-triggered,
/// not polled) deliberately does *not* use this cache — its "Refresh"
/// button should reflect a just-completed `opencode auth login` immediately.
const PROVIDER_READINESS_TTL: Duration = Duration::from_secs(30);

type ReadinessCacheEntry = (Instant, Vec<ProviderReadiness>);

#[derive(Clone, Default)]
pub struct ProviderReadinessCache(Arc<Mutex<Option<ReadinessCacheEntry>>>);

fn cached_provider_readiness(cache: &ProviderReadinessCache) -> Vec<ProviderReadiness> {
    let mut guard = cache.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((checked_at, providers)) = guard.as_ref() {
        if checked_at.elapsed() < PROVIDER_READINESS_TTL {
            return providers.clone();
        }
    }
    let providers: Vec<ProviderReadiness> = agent_provider::all()
        .map(|(agent, _catalog)| ProviderReadiness {
            provider: agent.id(),
            binary: agent.readiness(),
        })
        .collect();
    *guard = Some((Instant::now(), providers.clone()));
    providers
}

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
    readiness_cache: State<'_, ProviderReadinessCache>,
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
    let readiness_cache = readiness_cache.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        collect_system_metrics(&workspace_path, has_gpu, &cpu_monitor, &readiness_cache)
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
    readiness_cache: &ProviderReadinessCache,
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

    let providers = cached_provider_readiness(readiness_cache);

    SystemMetrics {
        git_branch,
        git_insertions,
        git_deletions,
        cpu_usage_pct,
        gpu_usage_pct,
        providers,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_provider::{AgentError, ModelInfo, TurnRequest};
    use crate::binary::{DiscoveredBinary, DiscoveryError};
    use crate::stream_event::{StreamEvent, StreamState};

    struct StubProvider;

    impl AgentProvider for StubProvider {
        fn id(&self) -> ToolProvider {
            ToolProvider::Claude
        }
        fn name(&self) -> &'static str {
            "stub"
        }
        fn discover(&self) -> Result<DiscoveredBinary, DiscoveryError> {
            Err(DiscoveryError::NotFound)
        }
        fn is_logged_in(&self, _bin: &Path) -> bool {
            false
        }
        fn build_command(&self, _bin: &Path, _req: &TurnRequest<'_>) -> Command {
            Command::new("true")
        }
        fn dispatch(
            &self,
            _json: &serde_json::Value,
            _state: &mut StreamState,
            _emit: &mut dyn FnMut(StreamEvent),
        ) {
        }
    }

    struct FailingCatalog;

    impl ModelCatalog for FailingCatalog {
        fn discover_models(&self) -> Result<Vec<ModelInfo>, AgentError> {
            Err(AgentError {
                message: "endpoint unreachable".to_string(),
            })
        }
    }

    struct EmptyCatalog;

    impl ModelCatalog for EmptyCatalog {
        fn discover_models(&self) -> Result<Vec<ModelInfo>, AgentError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn catalog_error_is_surfaced_not_dropped() {
        let caps = capabilities(&StubProvider, Some(&FailingCatalog));
        match caps.catalog {
            ModelCatalogStatus::Error { message } => assert_eq!(message, "endpoint unreachable"),
            ModelCatalogStatus::Available { .. } => panic!("expected Error status"),
        }
        assert!(caps.default_model.is_none());
    }

    #[test]
    fn empty_catalog_is_available_not_error() {
        let caps = capabilities(&StubProvider, Some(&EmptyCatalog));
        match caps.catalog {
            ModelCatalogStatus::Available { models } => assert!(models.is_empty()),
            ModelCatalogStatus::Error { .. } => panic!("expected Available status"),
        }
        assert!(caps.default_model.is_none());
    }

    #[test]
    fn no_catalog_impl_is_available_empty_not_error() {
        let caps = capabilities(&StubProvider, None);
        match caps.catalog {
            ModelCatalogStatus::Available { models } => assert!(models.is_empty()),
            ModelCatalogStatus::Error { .. } => panic!("expected Available status"),
        }
    }

    #[test]
    fn cached_provider_readiness_returns_cached_value_within_ttl() {
        let cache = ProviderReadinessCache::default();
        let fake = vec![ProviderReadiness {
            provider: ToolProvider::Claude,
            binary: crate::binary::BinaryStatus::Missing,
        }];
        *cache.0.lock().unwrap() = Some((Instant::now(), fake));

        let result = cached_provider_readiness(&cache);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].provider, ToolProvider::Claude);
        match result[0].binary {
            crate::binary::BinaryStatus::Missing => {}
            _ => panic!("expected the cached entry, cache was not honored"),
        }
    }

    #[test]
    fn cached_provider_readiness_recomputes_after_ttl_expires() {
        let cache = ProviderReadinessCache::default();
        let fake = vec![ProviderReadiness {
            provider: ToolProvider::Claude,
            binary: crate::binary::BinaryStatus::Missing,
        }];
        let stale_time = Instant::now() - PROVIDER_READINESS_TTL - Duration::from_secs(1);
        *cache.0.lock().unwrap() = Some((stale_time, fake));

        // Recomputed from the real registry (claude, codex, opencode), not
        // the single stale fake entry.
        let result = cached_provider_readiness(&cache);
        assert_eq!(result.len(), 3);
    }
}
