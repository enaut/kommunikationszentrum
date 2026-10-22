use dioxus_i18n::prelude::*;
use unic_langid::{LanguageIdentifier, langid};

const STORAGE_KEY: &str = "preferred_language";
const THEME_STORAGE_KEY: &str = "preferred_theme";

pub fn get_saved_language() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten())
}

pub fn save_language(lang: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(STORAGE_KEY, lang);
    }
}

pub fn get_saved_theme() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(THEME_STORAGE_KEY).ok().flatten())
}

pub fn save_theme(theme_str: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(THEME_STORAGE_KEY, theme_str);
    }
}

pub fn detect_initial_theme() -> dioxus_bootstrap_css::prelude::Theme {
    if let Some(saved) = get_saved_theme() {
        if saved.eq_ignore_ascii_case("dark") {
            return dioxus_bootstrap_css::prelude::Theme::Dark;
        } else if saved.eq_ignore_ascii_case("light") {
            return dioxus_bootstrap_css::prelude::Theme::Light;
        }
    }
    dioxus_bootstrap_css::prelude::Theme::Light
}

pub fn detect_initial_language() -> LanguageIdentifier {
    if let Some(saved) = get_saved_language() {
        if saved.starts_with("en") {
            return langid!("en-US");
        } else if saved.starts_with("de") {
            return langid!("de-DE");
        }
    }
    if let Some(browser_lang) = web_sys::window().and_then(|w| w.navigator().language()) {
        if browser_lang.starts_with("en") {
            return langid!("en-US");
        }
    }
    langid!("de-DE")
}

const BASE_DE_FTL: &str = include_str!("locales/de.ftl");
const PROFILE_DE_FTL: &str = include_str!(concat!(env!("OUT_DIR"), "/profile_de.ftl"));

const BASE_EN_FTL: &str = include_str!("locales/en.ftl");
const PROFILE_EN_FTL: &str = include_str!(concat!(env!("OUT_DIR"), "/profile_en.ftl"));

pub fn use_init_app_i18n() -> I18n {
    use_init_i18n(|| {
        let initial_lang = detect_initial_language();
        I18nConfig::new(initial_lang)
            .with_locale((langid!("de"), BASE_DE_FTL))
            .with_locale((langid!("de-DE"), PROFILE_DE_FTL))
            .with_locale((langid!("en"), BASE_EN_FTL))
            .with_locale((langid!("en-US"), PROFILE_EN_FTL))
            .with_fallback(langid!("de"))
    })
}

pub fn use_i18n() -> I18n {
    i18n()
}

pub fn switch_language(mut i18n: I18n, lang: LanguageIdentifier) {
    save_language(lang.language.as_str());
    i18n.set_language(lang);
}

pub fn theme_to_preference(
    theme: dioxus_bootstrap_css::prelude::Theme,
) -> crate::module_bindings::ThemePreference {
    match theme {
        dioxus_bootstrap_css::prelude::Theme::Dark => crate::module_bindings::ThemePreference::Dark,
        _ => crate::module_bindings::ThemePreference::Light,
    }
}

pub fn preference_to_theme(
    pref: crate::module_bindings::ThemePreference,
) -> dioxus_bootstrap_css::prelude::Theme {
    match pref {
        crate::module_bindings::ThemePreference::Dark => dioxus_bootstrap_css::prelude::Theme::Dark,
        crate::module_bindings::ThemePreference::Light => {
            dioxus_bootstrap_css::prelude::Theme::Light
        }
    }
}

pub fn langid_to_preference(
    lang: &LanguageIdentifier,
) -> crate::module_bindings::LanguagePreference {
    if lang.language.as_str() == "en" {
        crate::module_bindings::LanguagePreference::En
    } else {
        crate::module_bindings::LanguagePreference::De
    }
}

pub fn preference_to_langid(
    pref: crate::module_bindings::LanguagePreference,
) -> LanguageIdentifier {
    match pref {
        crate::module_bindings::LanguagePreference::En => langid!("en-US"),
        crate::module_bindings::LanguagePreference::De => langid!("de-DE"),
    }
}

pub fn format_timestamp(ts: spacetimedb_sdk::Timestamp) -> String {
    let fmt = dioxus_i18n::tid!("datetime-format");
    match ts.to_chrono_date_time() {
        Ok(utc_dt) => utc_dt
            .with_timezone(&chrono::Local)
            .format(&fmt)
            .to_string(),
        Err(_) => ts.to_string(),
    }
}
