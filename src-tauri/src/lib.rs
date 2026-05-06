mod cc;
mod commands;
mod db;
mod experiments;
mod mcp;
mod prompts;
mod resources;
mod wiki;
mod workspace;

use cc::session::SessionManager;
use commands::chat::{chat_cancel, chat_reset_session, chat_send};
use commands::experiments::{experiment_cancel, experiment_list, experiment_logs};
use commands::resources::{
    discard_resource, get_resource_confirm_prompt, index_resource, list_resources, submit_resource,
};
use commands::wiki::{read_wiki_file, save_ideas, save_notes, update_pulse_focus};
use commands::workspace::{
    close_workspace, init_workspace, open_workspace, read_workspace_state, save_workspace_state,
    setup_status,
};
use mcp::McpState;
use workspace::ActiveWorkspace;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
        .manage(ActiveWorkspace::default())
        .manage(SessionManager::default())
        .manage(McpState::default())
        .invoke_handler(tauri::generate_handler![
            setup_status,
            open_workspace,
            init_workspace,
            close_workspace,
            read_workspace_state,
            save_workspace_state,
            read_wiki_file,
            save_notes,
            save_ideas,
            update_pulse_focus,
            chat_send,
            chat_cancel,
            chat_reset_session,
            submit_resource,
            discard_resource,
            index_resource,
            list_resources,
            get_resource_confirm_prompt,
            experiment_list,
            experiment_logs,
            experiment_cancel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
