//! Connecting through the browser. The app starts the flow with a random
//! value, the signed-in player confirms on restedrealm.com, and the site hands
//! a one-time code back through a `restedrealm-companion://connect` link.
//! A link the app did not start is ignored, so nobody can connect a player's
//! PC to their own account by sending them a crafted link.

use crate::state::{Shared, Trigger};
use rrc_core::upload::{redeem_code, Https, BASE_URL};
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Url};
use tauri_plugin_opener::OpenerExt;

pub const SCHEME: &str = "restedrealm-companion";
/// How long the browser step may take.
const WINDOW: Duration = Duration::from_secs(15 * 60);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConnectResult {
    ok: bool,
    message: String,
}

pub fn device_name() -> String {
    std::env::var("COMPUTERNAME").ok().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Windows PC".into())
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Open the confirm page in the player's browser.
pub fn start(app: &AppHandle, shared: &Shared) -> Result<(), String> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|_| "Could not create a secure connection request.".to_string())?;
    let state = hex::encode(bytes);
    let url = format!("{BASE_URL}/companion/connect?state={state}&device={}", encode(&device_name()));
    *shared.pending_connect.lock().unwrap() = Some((state, Instant::now()));
    app.opener().open_url(url, None::<&str>).map_err(|e| format!("Could not open your browser: {e}"))
}

fn report(app: &AppHandle, ok: bool, message: &str) {
    let _ = app.emit("connect-result", ConnectResult { ok, message: message.into() });
    let _ = app.emit("state-changed", ());
}

/// Accept a returning link only for the request this app started, and only in time.
fn check_state(pending: Option<(String, Instant)>, state: &str) -> Result<(), &'static str> {
    match pending {
        Some((expected, started)) if expected == state && started.elapsed() < WINDOW => Ok(()),
        Some(_) => Err("That link does not match this app's request, or it took too long. Choose Connect again."),
        None => Err("This app did not ask to connect. Choose Connect in the app to start."),
    }
}

/// Handle a `restedrealm-companion://connect?code=…&state=…` link.
pub fn handle_link(app: &AppHandle, shared: &Arc<Shared>, url: &Url) {
    if url.scheme() != SCHEME || url.host_str() != Some("connect") {
        return;
    }
    crate::tray::show_window(app);
    let query = |key: &str| url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| v.into_owned());
    let (Some(code), Some(state)) = (query("code"), query("state")) else {
        return report(app, false, "That link was incomplete. Choose Connect again.");
    };
    // The pending request is used once, whatever happens next.
    let pending = shared.pending_connect.lock().unwrap().take();
    if let Err(reason) = check_state(pending, &state) {
        return report(app, false, reason);
    }
    let app = app.clone();
    let shared = shared.clone();
    std::thread::spawn(move || match redeem_code(&Https::default(), shared.store.as_ref(), &code, &device_name()) {
        Ok(_) => {
            shared.nudge(Trigger::SyncNow);
            report(&app, true, "Connected to your RestedRealm account.");
        }
        Err(e) => report(&app, false, &e.to_string()),
    });
}

#[cfg(test)]
mod tests {
    use super::{check_state, encode, WINDOW};
    use std::time::Instant;

    #[test]
    fn only_the_request_this_app_started_is_accepted() {
        let now = Instant::now();
        assert!(check_state(Some(("abc".into(), now)), "abc").is_ok());
        assert!(check_state(Some(("abc".into(), now)), "abd").is_err());
        assert!(check_state(None, "abc").is_err());
        if let Some(old) = now.checked_sub(WINDOW + std::time::Duration::from_secs(1)) {
            assert!(check_state(Some(("abc".into(), old)), "abc").is_err());
        }
    }

    #[test]
    fn device_names_are_encoded_for_a_url() {
        assert_eq!(encode("DESKTOP-AB12"), "DESKTOP-AB12");
        assert_eq!(encode("Anna's PC & co"), "Anna%27s%20PC%20%26%20co");
        assert_eq!(encode("Zoë"), "Zo%C3%AB");
    }
}
