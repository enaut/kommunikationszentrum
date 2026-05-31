use ::dioxus::prelude::*;

use crate::oauth::UserInfo;
use crate::router::{use_is_admin, ActiveView};

#[component]
pub fn Navbar(
    user_info: UserInfo,
    active_view: Signal<ActiveView>,
    on_logout: EventHandler<()>,
) -> Element {
    let is_admin = use_is_admin();

    rsx! {
        nav { class: "navbar navbar-expand-lg navbar-dark bg-primary",
            div { class: "container-fluid",
                span { class: "navbar-brand",
                    i { class: "bi bi-envelope-fill me-2" }
                    "Kommunikationszentrum"
                }
                button {
                    class: "navbar-toggler",
                    "type": "button",
                    "data-bs-toggle": "collapse",
                    "data-bs-target": "#navbarNav",
                    span { class: "navbar-toggler-icon" }
                }
                div { class: "collapse navbar-collapse", id: "navbarNav",
                    ul { class: "navbar-nav me-auto",
                        NavLink {
                            label: "Meine Kategorien",
                            icon: "bi-envelope-check",
                            view: ActiveView::MySubscriptions,
                            active_view,
                        }
                        if is_admin {
                            NavLink {
                                label: "Kategorien",
                                icon: "bi-tags-fill",
                                view: ActiveView::Categories,
                                active_view,
                            }
                            NavLink {
                                label: "Mitglieder",
                                icon: "bi-people-fill",
                                view: ActiveView::Members,
                                active_view,
                            }
                            NavLink {
                                label: "Debug",
                                icon: "bi-bug-fill",
                                view: ActiveView::Debug,
                                active_view,
                            }
                        }
                    }
                    ul { class: "navbar-nav ms-auto",
                        li { class: "nav-item dropdown",
                            a {
                                class: "nav-link dropdown-toggle",
                                href: "#",
                                role: "button",
                                "data-bs-toggle": "dropdown",
                                i { class: "bi bi-person-circle me-2" }
                                if let Some(name) = &user_info.name {
                                    "{name}"
                                } else {
                                    "{user_info.username}"
                                }
                            }
                            ul { class: "dropdown-menu dropdown-menu-end",
                                li {
                                    button {
                                        class: "dropdown-item",
                                        onclick: move |_| on_logout.call(()),
                                        i { class: "bi bi-box-arrow-right me-2" }
                                        "Abmelden"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn NavLink(
    label: &'static str,
    icon: &'static str,
    view: ActiveView,
    active_view: Signal<ActiveView>,
) -> Element {
    let is_active = *active_view.read() == view;
    let view_for_click = view.clone();
    rsx! {
        li { class: "nav-item",
            button {
                class: if is_active {
                    "nav-link btn btn-link text-white fw-bold"
                } else {
                    "nav-link btn btn-link text-white-50"
                },
                onclick: move |_| active_view.set(view_for_click.clone()),
                i { class: "bi {icon} me-1" }
                "{label}"
            }
        }
    }
}
