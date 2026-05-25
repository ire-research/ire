mod binary;
mod cc;
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
mod wiki;
mod workspace;

use cc::session::SessionManager;
use commands::chat::{chat_cancel, chat_reset_session, chat_send};
use commands::experiments::{
    experiment_cancel, experiment_delete, experiment_list, experiment_logs, experiment_rename,
};
use commands::resources::{
    discard_resource, get_resource_confirm_prompt, submit_local_resource, submit_resource,
    submit_resources,
};
use commands::system::get_system_status;
use commands::wiki::{
    read_wiki_file, save_ideas_json, save_notes, save_pulse_field, save_wiki_file,
};
use commands::workspace::{
    close_workspace, init_workspace, open_in_vscode, open_workspace, read_user_config,
    read_workspace_state, save_user_config, save_workspace_state, setup_status,
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
            open_in_vscode,
            read_workspace_state,
            save_workspace_state,
            read_user_config,
            save_user_config,
            read_wiki_file,
            save_wiki_file,
            save_notes,
            save_pulse_field,
            save_ideas_json,
            get_system_status,
            chat_send,
            chat_cancel,
            chat_reset_session,
            submit_resource,
            submit_local_resource,
            submit_resources,
            discard_resource,
            get_resource_confirm_prompt,
            experiment_list,
            experiment_logs,
            experiment_cancel,
            experiment_delete,
            experiment_rename,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
