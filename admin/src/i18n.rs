use dioxus_i18n::prelude::*;
use unic_langid::{langid, LanguageIdentifier};

const STORAGE_KEY: &str = "preferred_language";

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

pub fn detect_initial_language() -> LanguageIdentifier {
    if let Some(saved) = get_saved_language() {
        if saved.starts_with("en") {
            return langid!("en");
        } else if saved.starts_with("de") {
            return langid!("de");
        }
    }
    if let Some(browser_lang) = web_sys::window().and_then(|w| w.navigator().language()) {
        if browser_lang.starts_with("en") {
            return langid!("en");
        }
    }
    langid!("de")
}

pub fn use_init_app_i18n() -> I18n {
    use_init_i18n(|| {
        let initial_lang = detect_initial_language();
        I18nConfig::new(initial_lang)
            .with_locale((langid!("de"), include_str!("locales/de.ftl")))
            .with_locale((langid!("en"), include_str!("locales/en.ftl")))
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
