use ::dioxus::prelude::*;
use dioxus_bootstrap_css::prelude::{
    Color, Dropdown, DropdownItem, Icon, NavItem, NavLink, Navbar as BsNavbar, NavbarCollapse,
    NavbarExpand, NavbarNav, NavbarToggler, Theme,
};
use dioxus_i18n::tid;

use crate::components::language_switcher::LanguageSwitcher;
use crate::oauth::UserInfo;
use crate::router::{ActiveView, use_is_admin};

#[component]
pub fn Navbar(
    user_info: UserInfo,
    active_view: Signal<ActiveView>,
    on_logout: EventHandler<()>,
    theme: Signal<Theme>,
) -> Element {
    let is_admin = use_is_admin();
    let collapsed = use_signal(|| true);
    let user_dropdown_open = use_signal(|| false);
    let management_dropdown_open = use_signal(|| false);
    let update_account_config = crate::module_bindings::dioxus::use_reducer_update_account_config();
    let update_account_config_for_theme = update_account_config.clone();
    rsx! {
        BsNavbar {
            expand: NavbarExpand::Lg,
            color: Color::Primary,
            class: "navbar-dark",
            brand: rsx! {
                Icon { name: "envelope-fill", class: "me-2 text-white" }
                span { class: "text-white", {tid!("navbar-brand")} }
            },
            NavbarToggler { collapsed }
            NavbarCollapse { collapsed,
                NavbarNav { class: "me-auto",
                    AppNavLink {
                        label: tid!("navbar-my-topics"),
                        icon: "envelope-check",
                        view: ActiveView::MySubscriptions,
                        active_view,
                    }
                    AppNavLink {
                        label: tid!("navbar-messages"),
                        icon: "envelope",
                        view: ActiveView::Messages,
                        active_view,
                    }
                    if is_admin {
                        AppNavLink {
                            label: tid!("navbar-topics"),
                            icon: "tags-fill",
                            view: ActiveView::Topics,
                            active_view,
                        }
                        AppNavLink {
                            label: tid!("navbar-members"),
                            icon: "people-fill",
                            view: ActiveView::Members,
                            active_view,
                        }
                        NavItem {
                            Dropdown {
                                open: management_dropdown_open,
                                toggle_class: "btn-link nav-link text-white",
                                toggle: rsx! {
                                    Icon { name: "sliders", class: "me-1" }
                                    {tid!("navbar-management")}
                                },
                                menu: rsx! {
                                    DropdownItem { onclick: move |_| active_view.set(ActiveView::ManagementConfiguration), {tid!("navbar-settings")} }
                                    DropdownItem { onclick: move |_| active_view.set(ActiveView::ManagementStatus), {tid!("navbar-status")} }
                                },
                            }
                        }
                    }
                }
                NavbarNav { class: "ms-auto",
                    NavItem {
                        LanguageSwitcher {}
                    }
                    NavItem {
                        {
                            let current_theme = *theme.read();
                            let icon = match current_theme {
                                Theme::Light => "moon-stars",
                                Theme::Dark => "sun",
                            };
                            let label = match current_theme {
                                Theme::Light => "Switch to dark mode",
                                Theme::Dark => "Switch to light mode",
                            };
                            let update_fn = update_account_config_for_theme.clone();
                            let mut theme_sig = theme;
                            rsx! {
                                button {
                                    class: "btn btn-outline-light nav-link px-2 d-flex align-items-center",
                                    r#type: "button",
                                    title: "{label}",
                                    "aria-label": "{label}",
                                    onclick: move |_| {
                                        let next = theme_sig.read().toggle();
                                        theme_sig.set(next);
                                        let next_str = match next {
                                            Theme::Dark => "dark",
                                            _ => "light",
                                        };
                                        crate::i18n::save_theme(next_str);
                                        let _ = update_fn(
                                            None,
                                            None,
                                            None,
                                            false,
                                            None,
                                            None,
                                            None,
                                            false,
                                            None,
                                            false,
                                            None,
                                            Some(next_str.to_string()),
                                            None,
                                            false,
                                        );
                                    },
                                    i { class: "bi bi-{icon}" }
                                }
                            }
                        }
                    }
                    NavItem {
                        Dropdown {
                            open: user_dropdown_open,
                            align_end: true,
                            toggle_class: "btn-link nav-link text-white",
                            toggle: rsx! {
                                Icon { name: "person-circle", class: "me-2" }
                                if let Some(name) = &user_info.name {
                                    "{name}"
                                } else {
                                    "{user_info.username}"
                                }
                            },
                            menu: rsx! {
                                DropdownItem {
                                    onclick: move |_| active_view.set(ActiveView::Profile),
                                    Icon { name: "person", class: "me-2" }
                                    {tid!("navbar-profile")}
                                }
                                hr { class: "dropdown-divider my-1" }
                                DropdownItem { onclick: move |_| on_logout.call(()),
                                    Icon { name: "box-arrow-right", class: "me-2" }
                                    {tid!("navbar-logout")}
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AppNavLink(
    label: String,
    icon: &'static str,
    view: ActiveView,
    mut active_view: Signal<ActiveView>,
) -> Element {
    let is_active = *active_view.read() == view;
    let view_for_click = view.clone();
    rsx! {
        NavItem {
            NavLink {
                active: is_active,
                prevent_default: true,
                onclick: move |_| active_view.set(view_for_click.clone()),
                Icon { name: icon, class: "me-1" }
                "{label}"
            }
        }
    }
}
