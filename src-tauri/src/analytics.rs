use std::time::Duration;

use serde_json::json;

const POSTHOG_HOST: &str = "https://eu.i.posthog.com";
const POSTHOG_API_KEY: &str = "phc_rTTEsnXUW3ZWRmJTbeKENHft6Tfc7pYAzH78zpsSzKGy";
const MAX_LAUNCH_ATTEMPTS: u32 = 4;
const LAUNCH_RETRY_DELAY: Duration = Duration::from_secs(5 * 60);
const LAUNCH_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const CLOSE_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

fn capture_body(event: &str, distinct_id: &str, extra_properties: serde_json::Value) -> String {
    let mut properties = json!({
        "$app_version": env!("CARGO_PKG_VERSION"),
        "$os": std::env::consts::OS,
    });
    if let (Some(props), Some(extra)) = (properties.as_object_mut(), extra_properties.as_object()) {
        props.extend(extra.clone());
    }
    json!({
        "api_key": POSTHOG_API_KEY,
        "event": event,
        "distinct_id": distinct_id,
        "properties": properties,
    })
    .to_string()
}

/// Fires a fire-and-forget "app_launched" event to PostHog for retention/usage tracking,
/// retrying every 5 minutes up to `MAX_LAUNCH_ATTEMPTS` on failure.
///
/// Runs on a plain OS thread rather than a Tokio task: `reqwest::blocking` builds its own
/// internal runtime and must not be driven from inside an existing one (Tauri's async
/// runtime), which can otherwise stall the app. A detached thread also can't block shutdown.
pub fn track_app_launched(distinct_id: String) {
    std::thread::spawn(move || {
        let client = match reqwest::blocking::Client::builder()
            .timeout(LAUNCH_REQUEST_TIMEOUT)
            .build()
        {
            Ok(client) => client,
            Err(err) => {
                tracing::debug!(%err, "analytics client build failed");
                return;
            }
        };

        let body = capture_body("app_launched", &distinct_id, json!({}));

        for attempt in 1..=MAX_LAUNCH_ATTEMPTS {
            let result = client
                .post(format!("{POSTHOG_HOST}/capture/"))
                .header("Content-Type", "application/json")
                .body(body.clone())
                .send();

            match result {
                Ok(resp) if resp.status().is_success() => return,
                Ok(resp) => tracing::debug!(status = %resp.status(), attempt, "analytics ping rejected"),
                Err(err) => tracing::debug!(%err, attempt, "analytics ping failed"),
            }

            if attempt < MAX_LAUNCH_ATTEMPTS {
                std::thread::sleep(LAUNCH_RETRY_DELAY);
            }
        }
    });
}

/// Sends a best-effort "app_closed" event with the session duration, synchronously, on the
/// calling thread. Called from the Tauri `RunEvent::Exit` handler, which runs on the native
/// event loop (not inside Tokio), so a direct blocking call is safe here. Deliberately a
/// single attempt with a short timeout: this runs during shutdown, so it must not noticeably
/// delay quitting, and there's no later point to retry from.
pub fn track_app_closed(distinct_id: String, session_duration: Duration) {
    let client = match reqwest::blocking::Client::builder()
        .timeout(CLOSE_REQUEST_TIMEOUT)
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            tracing::debug!(%err, "analytics client build failed");
            return;
        }
    };

    let body = capture_body(
        "app_closed",
        &distinct_id,
        json!({ "session_duration_seconds": session_duration.as_secs() }),
    );

    if let Err(err) = client
        .post(format!("{POSTHOG_HOST}/capture/"))
        .header("Content-Type", "application/json")
        .body(body)
        .send()
    {
        tracing::debug!(%err, "analytics close ping failed");
    }
}
