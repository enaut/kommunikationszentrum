use dioxus::prelude::*;
use dioxus_bootstrap_css::prelude::{Dropdown, DropdownItem, Icon};

use dioxus_i18n::tid;

use crate::i18n::use_i18n;

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

    let set_theme_and_lang = crate::module_bindings::dioxus::use_reducer_set_theme_and_lang();

    let set_de = set_theme_and_lang.clone();
    let set_en = set_theme_and_lang.clone();

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
                        let _ = set_de(None, Some(crate::module_bindings::LanguagePreference::De));
                    },
                    {tid!("language-switcher-de")}
                }
                DropdownItem {
                    active: current_code == "en",
                    onclick: move |_| {
                        let _ = set_en(None, Some(crate::module_bindings::LanguagePreference::En));
                    },
                    {tid!("language-switcher-en")}
                }
            }
        }
    }
}
