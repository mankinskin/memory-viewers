use dioxus::prelude::*;
use viewer_api_dioxus::{Layout, Overlay, TabsStore, ThemeSettings};

use crate::sse::use_sse;
use crate::store::SpecListStore;
use crate::types::SpecSummary;

use super::effects::{
    persist_store, use_initial_selected_tab, use_prefetch_neighbors, use_refresh_tab_labels,
    use_selected_id_from_tabs, use_spec_list, use_tabs_from_selected_id,
};
use super::helpers::sidebar_button_state;
use super::render::{
    render_spec_list_content, render_spec_list_header, render_spec_list_sidebar,
};
use super::super::Route;

#[component]
pub fn SpecListPage() -> Element {
    let store = SpecListStore::use_store();
    let sidebar_collapsed = use_signal(|| false);
    let mobile_sidebar_open = use_signal(|| false);
    let mut show_theme_settings = use_signal(|| false);
    let filter_panel_open = use_signal(|| false);
    let specs: Signal<Vec<SpecSummary>> = use_signal(Vec::new);
    let loading: Signal<bool> = use_signal(|| true);
    let list_error: Signal<Option<String>> = use_signal(|| None);
    let selected_id = store.open_spec_id;
    let filter = store.filter;
    let state_filter = store.state_filter;
    let active_tab = store.active_tab;
    let mut tabs: TabsStore<String> = use_hook(TabsStore::new);
    let cache = use_context::<crate::SpecCache>();
    let nav = use_navigator();

    persist_store(store);
    use_initial_selected_tab(selected_id, tabs);
    use_selected_id_from_tabs(selected_id, tabs);
    use_tabs_from_selected_id(selected_id, specs, tabs);
    use_refresh_tab_labels(specs, tabs);
    use_spec_list(specs, loading, list_error, filter, state_filter);
    use_sse(specs);
    use_prefetch_neighbors(specs, tabs, cache);

    let (sidebar_button_active, sidebar_button_label) =
        sidebar_button_state(sidebar_collapsed, mobile_sidebar_open);
    let nav_home = nav.clone();
    let on_home = EventHandler::new(move |_| {
        nav_home.push(Route::SpecListPage {});
    });

    rsx! {
        Layout {
            header: rsx! {
                {render_spec_list_header(
                    sidebar_button_active,
                    sidebar_button_label,
                    sidebar_collapsed,
                    mobile_sidebar_open,
                    filter_panel_open,
                    show_theme_settings,
                    filter,
                    state_filter,
                    on_home,
                )}
            },
            {render_spec_list_sidebar(
                sidebar_collapsed,
                mobile_sidebar_open,
                specs,
                loading,
                list_error,
                filter,
                state_filter,
                active_tab,
                tabs,
            )}
            {render_spec_list_content(specs, filter, state_filter, active_tab, tabs)}
        }
        Overlay {
            open: *show_theme_settings.read(),
            on_close: move |_| show_theme_settings.set(false),
            panel_class: "theme-settings-modal".to_string(),
            aria_label: "Theme settings".to_string(),
            ThemeSettings {
                on_close: move |_| show_theme_settings.set(false),
            }
        }
    }
}