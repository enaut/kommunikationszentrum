use ::dioxus::prelude::*;
use dioxus_bootstrap_css::prelude::{
    Color, Dropdown, DropdownItem, Icon, NavItem, NavLink, Navbar as BsNavbar, NavbarCollapse,
    NavbarExpand, NavbarNav, NavbarToggler, Theme, ThemeToggle,
};
use dioxus_i18n::tid;

use crate::components::language_switcher::LanguageSwitcher;
use crate::oauth::UserInfo;
use crate::router::{use_is_admin, ActiveView};

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
    rsx! {
        BsNavbar {
            expand: NavbarExpand::Lg,
            color: Color::Primary,
            class: "navbar-dark",
            brand: rsx! {
                Icon { name: "envelope-fill", class: "me-2 text-white" }
                span { class: "text-white", "{tid!(\"navbar-brand\")}" }
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
                            view: ActiveView::Categories,
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
                                    "{tid!(\"navbar-management\")}"
                                },
                                menu: rsx! {
                                    DropdownItem { onclick: move |_| active_view.set(ActiveView::ManagementConfiguration), "{tid!(\"navbar-settings\")}" }
                                    DropdownItem { onclick: move |_| active_view.set(ActiveView::ManagementStatus), "{tid!(\"navbar-status\")}" }
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
                        ThemeToggle { theme }
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
                                DropdownItem { onclick: move |_| on_logout.call(()),
                                    Icon { name: "box-arrow-right", class: "me-2" }
                                    "{tid!(\"navbar-logout\")}"
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
