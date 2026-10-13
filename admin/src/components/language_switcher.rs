use dioxus::prelude::*;
use dioxus_bootstrap_css::prelude::{Dropdown, DropdownItem, Icon};

use dioxus_i18n::tid;
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

    let update_config = crate::module_bindings::dioxus::use_reducer_update_account_config();

    let update_config_de = update_config.clone();
    let update_config_en = update_config.clone();

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
                        let _ = update_config_de(None, None, None, false, None, None, None, false, None, false, Some("de".to_string()), None, None, false);
                    },
                    {tid!("language-switcher-de")}
                }
                DropdownItem {
                    active: current_code == "en",
                    onclick: move |_| {
                        switch_language(i18n, langid!("en"));
                        let _ = update_config_en(None, None, None, false, None, None, None, false, None, false, Some("en".to_string()), None, None, false);
                    },
                    {tid!("language-switcher-en")}
                }
            }
        }
    }
}
