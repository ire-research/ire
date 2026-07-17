use crate::{analytics, user_config};

#[tauri::command]
pub async fn submit_feedback(message: String, email: Option<String>) -> Result<(), String> {
    let user_id = user_config::analytics_id();
    tokio::task::spawn_blocking(move || analytics::track_feedback(user_id, message, email))
        .await
        .map_err(|e| e.to_string())?
}
