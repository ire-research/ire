mod cc;
mod commands;
mod db;
mod wiki;
mod workspace;

use cc::session::ChatSession;
use commands::chat::{chat_cancel, chat_reset_session, chat_send};
use commands::wiki::{read_wiki_file, save_ideas, save_notes};
use commands::workspace::{
    close_workspace, init_workspace, open_workspace, setup_status,
};
use workspace::ActiveWorkspace;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(ActiveWorkspace::default())
        .manage(ChatSession::default())
        .invoke_handler(tauri::generate_handler![
            setup_status,
            open_workspace,
            init_workspace,
            close_workspace,
            read_wiki_file,
            save_notes,
            save_ideas,
            chat_send,
            chat_cancel,
            chat_reset_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
