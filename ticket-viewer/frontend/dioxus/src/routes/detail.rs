use dioxus::prelude::*;

use crate::routes::Route;

#[component]
pub fn TicketDetailPage(workspace: String, id: String) -> Element {
    let nav = use_navigator();
    let workspace = workspace.clone();
    let ticket_id = id.clone();

    use_effect(move || {
        if let Some(window) = web_sys::window() {
            let _ = window.location().set_hash(&format!("id={ticket_id}"));
        }
        nav.replace(Route::TicketListPage {
            workspace: workspace.clone(),
        });
    });

    rsx! {}
}