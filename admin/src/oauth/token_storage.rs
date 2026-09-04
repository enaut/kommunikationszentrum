use crate::oauth::UserInfo;
use web_sys::window;

pub fn get_stored_user_info() -> Option<UserInfo> {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok()).flatten() {
        if let Ok(Some(user_info_str)) = storage.get_item("oauth_user_info") {
            return serde_json::from_str(&user_info_str).ok();
        }
    }
    None
}

pub fn store_user_info(user_info: &UserInfo) {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok()).flatten() {
        if let Ok(user_info_str) = serde_json::to_string(user_info) {
            let _ = storage.set_item("oauth_user_info", &user_info_str);
        }
    }
}

pub fn remove_stored_user_info() {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.remove_item("oauth_user_info");
    }
}

pub fn get_stored_code_verifier() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item("oauth_code_verifier").ok().flatten())
}

pub fn store_code_verifier(code_verifier: &str) {
    if let Some(s) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = s.set_item("oauth_code_verifier", code_verifier);
    }
}

pub fn remove_stored_code_verifier() {
    if let Some(s) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = s.remove_item("oauth_code_verifier");
    }
}

pub fn store_state(state: &str) {
    if let Some(s) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = s.set_item("oauth_state", state);
    }
}

pub fn get_stored_state() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item("oauth_state").ok().flatten())
}

pub fn remove_stored_state() {
    if let Some(s) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = s.remove_item("oauth_state");
    }
}

pub fn store_nonce(nonce: &str) {
    if let Some(s) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = s.set_item("oauth_nonce", nonce);
    }
}

pub fn get_stored_nonce() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item("oauth_nonce").ok().flatten())
}

pub fn remove_stored_nonce() {
    if let Some(s) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = s.remove_item("oauth_nonce");
    }
}
