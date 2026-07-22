mod agent_provider;
mod analytics;
mod binary;
#[path = "claude-code/mod.rs"]
mod claude_code;
mod codex;
mod commands;
mod db;
mod events;
mod experiments;
mod mcp;
mod prompts;
mod resources;
mod tool_cards;
mod user_config;
mod ire;
mod workspace;

use claude_code::session::SessionManager;
use commands::chat::{
    chat_cancel, chat_reset_session, chat_send, generate_chat_title, submit_ask_answer,
};
use commands::experiments::{
    experiment_cancel, experiment_delete, experiment_list, experiment_logs, experiment_rename,
};
use commands::feedback::submit_feedback;
use commands::history::{
    chat_history_delete, chat_history_get, chat_history_list, chat_history_save,
};
use commands::resources::{
    confirm_resource, discard_resource, read_resource_draft, save_resource_draft,
    submit_local_resource, submit_resource, submit_resources, InflightResources,
};
use commands::system::{
    get_system_info, get_system_metrics, list_agent_models, CpuMonitor, SystemInfoCache,
};
use commands::ire::{
    read_resource, save_focus_field, save_ideas, save_notes, save_resource,
};
use commands::workspace::{
    close_workspace, open_in_vscode, open_workspace, read_user_config,
    save_user_config, setup_status,
};
use mcp::McpState;
use workspace::ActiveWorkspace;

/// Run as the stdio MCP server (`ire --mcp-stdio`), spawned by Claude Code / Codex.
pub fn run_mcp_stdio() {
    mcp::stdio_server::run_stdio();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let launch_at = std::time::Instant::now();

    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .compact()
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|_app| {
            if !cfg!(debug_assertions) && user_config::analytics_enabled() {
                analytics::track_app_launched(user_config::analytics_id());
            }
            Ok(())
        })
        .manage(ActiveWorkspace::default())
        .manage(SessionManager::default())
        .manage(McpState::default())
        .manage(InflightResources::default())
        .manage(SystemInfoCache::default())
        .manage(CpuMonitor::default())
        .invoke_handler(tauri::generate_handler![
            setup_status,
            open_workspace,
            close_workspace,
            open_in_vscode,
            read_user_config,
            save_user_config,
            read_resource,
            save_resource,
            save_notes,
            save_focus_field,
            save_ideas,
            get_system_info,
            get_system_metrics,
            list_agent_models,
            chat_send,
            chat_cancel,
            chat_reset_session,
            submit_ask_answer,
            generate_chat_title,
            submit_resource,
            submit_local_resource,
            submit_resources,
            discard_resource,
            read_resource_draft,
            save_resource_draft,
            confirm_resource,
            experiment_list,
            experiment_logs,
            experiment_cancel,
            experiment_delete,
            experiment_rename,
            chat_history_save,
            chat_history_list,
            chat_history_get,
            chat_history_delete,
            submit_feedback,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                if !cfg!(debug_assertions) && user_config::analytics_enabled() {
                    analytics::track_app_closed(user_config::analytics_id(), launch_at.elapsed());
                }
            }
        });
}
