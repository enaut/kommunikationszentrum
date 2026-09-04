use dioxus::prelude::*;
use dioxus_bootstrap_css::prelude::{Dropdown, DropdownItem, Icon};

use unic_langid::langid;

use crate::i18n::{switch_language, use_i18n};

#[component]
pub fn LanguageSwitcher() -> Element {
    let i18n = use_i18n();
    let open = use_signal(|| false);
    let current_lang = i18n.language();
    let current_code = current_lang.language.as_str();

    let display_label = match current_code {
        "de" => "DE",
        "en" => "EN",
        other => other,
    };

    rsx! {
        Dropdown {
            open,
            align_end: true,
            toggle_class: "btn-link nav-link text-white d-flex align-items-center",
            toggle: rsx! {
                Icon { name: "globe", class: "me-1" }
                span { "{display_label}" }
            },
            menu: rsx! {
                DropdownItem {
                    active: current_code == "de",
                    onclick: move |_| {
                        switch_language(i18n, langid!("de"));
                    },
                    "Deutsch (DE)"
                }
                DropdownItem {
                    active: current_code == "en",
                    onclick: move |_| {
                        switch_language(i18n, langid!("en"));
                    },
                    "English (EN)"
                }
            }
        }
    }
}
