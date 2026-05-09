use dioxus::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{
    api,
    store::SpecListStore,
    types::SpecSummary,
};

use super::helpers::{
    compute_prefetch_targets,
    label_for,
};

pub(super) fn persist_store(store: SpecListStore) {
    use_effect(move || {
        store.persist();
    });
}

pub(super) fn use_initial_selected_tab(
    selected_id: Signal<Option<String>>,
    mut tabs: viewer_api_dioxus::TabsStore<String>,
) {
    use_hook(move || {
        if let Some(id) = selected_id.peek().clone() {
            tabs.open(id.clone(), id);
        }
    });
}

pub(super) fn use_selected_id_from_tabs(
    mut selected_id: Signal<Option<String>>,
    tabs: viewer_api_dioxus::TabsStore<String>,
) {
    use_effect(move || {
        let next = tabs.active.read().clone();
        if *selected_id.peek() != next {
            selected_id.set(next);
        }
    });
}

pub(super) fn use_tabs_from_selected_id(
    selected_id: Signal<Option<String>>,
    specs: Signal<Vec<SpecSummary>>,
    mut tabs: viewer_api_dioxus::TabsStore<String>,
) {
    use_effect(move || {
        if let Some(id) = selected_id.read().clone() {
            if tabs.active.peek().as_deref() != Some(id.as_str()) {
                let label = label_for(&specs.peek(), &id);
                tabs.open(id, label);
            }
        }
    });
}

pub(super) fn use_refresh_tab_labels(
    specs: Signal<Vec<SpecSummary>>,
    mut tabs: viewer_api_dioxus::TabsStore<String>,
) {
    use_effect(move || {
        let list = specs.read().clone();
        if list.is_empty() {
            return;
        }

        let snapshot = tabs.snapshot();
        let mut updated = false;
        let new_tabs: Vec<viewer_api_dioxus::Tab<String>> = snapshot
            .tabs
            .into_iter()
            .map(|tab| {
                let label = label_for(&list, &tab.id);
                if label != tab.payload {
                    updated = true;
                }
                viewer_api_dioxus::Tab {
                    id: tab.id,
                    payload: label,
                }
            })
            .collect();

        if updated {
            tabs.set_tabs(new_tabs, snapshot.active);
        }
    });
}

pub(super) fn use_spec_list(
    mut specs: Signal<Vec<SpecSummary>>,
    mut loading: Signal<bool>,
    mut list_error: Signal<Option<String>>,
    filter: Signal<String>,
    state_filter: Signal<String>,
) {
    use_effect(move || {
        loading.set(true);
        list_error.set(None);

        let query_value = filter.read().clone();
        let state_value = state_filter.read().clone();
        let query = if query_value.trim().is_empty() {
            None
        } else {
            Some(query_value.trim().to_string())
        };
        let state = if state_value.is_empty() {
            None
        } else {
            Some(state_value)
        };

        spawn_local(async move {
            match api::list_specs(state.as_deref(), query.as_deref(), None)
                .await
            {
                Ok(response) => {
                    specs.set(response.items);
                    loading.set(false);
                },
                Err(message) => {
                    list_error.set(Some(message));
                    loading.set(false);
                },
            }
        });
    });
}

pub(super) fn use_prefetch_neighbors(
    specs: Signal<Vec<SpecSummary>>,
    tabs: viewer_api_dioxus::TabsStore<String>,
    cache: crate::SpecCache,
) {
    use_effect(move || {
        let Some(active) = tabs.active.read().clone() else {
            return;
        };

        for id in compute_prefetch_targets(&specs.read(), &active) {
            if cache.get(&id).is_some() {
                continue;
            }

            let cache = cache.clone();
            spawn_local(async move {
                if cache.get(&id).is_some() {
                    return;
                }
                if let Ok(response) = api::get_spec_full(&id).await {
                    cache.insert(id, response);
                }
            });
        }
    });
}
