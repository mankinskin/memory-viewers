use std::collections::BTreeMap;

use dioxus::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
use viewer_api_dioxus::{
    BreadcrumbItem,
    Breadcrumbs,
    HamburgerIcon,
    Layout,
    NodeIcon,
    Overlay,
    PageHeader,
    Sidebar,
    TabBar,
    TabItem,
    TabsStore,
    ThemeSettings,
    ThemeProvider,
    TreeNode,
    TreeView,
    ViewerShell,
    WgpuOverlay,
};

use crate::{
    api,
    types::{
        CargoDocArtifact,
        DocWorkspaceResponse,
    },
};

const RUSTDOC_IFRAME_ID: &str = "doc-browser-frame";
const RUSTDOC_STYLE_ID: &str = "doc-viewer-rustdoc-theme";
const RUSTDOC_STYLE_TEXT: &str =
    include_str!("../public/doc-viewer-overrides.css");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtifactView {
    Html,
    Json,
}

#[derive(Clone, Debug, PartialEq)]
struct OpenArtifactTab {
    id: String,
    label: String,
    package_name: String,
    target_name: String,
    artifact: CargoDocArtifact,
    active_view: ArtifactView,
    json_content: Option<String>,
    json_loading: bool,
    json_error: Option<String>,
}

impl OpenArtifactTab {
    fn from_artifact(artifact: CargoDocArtifact) -> Self {
        let default_view = if artifact.html_exists {
            ArtifactView::Html
        } else {
            ArtifactView::Json
        };
        Self {
            id: artifact_id(&artifact),
            label: format!("{}::{}", artifact.package_name, artifact.target_name),
            package_name: artifact.package_name.clone(),
            target_name: artifact.target_name.clone(),
            artifact,
            active_view: default_view,
            json_content: None,
            json_loading: false,
            json_error: None,
        }
    }
}

fn artifact_id(artifact: &CargoDocArtifact) -> String {
    format!("{}::{}", artifact.package_name, artifact.target_name)
}

#[cfg(target_arch = "wasm32")]
fn apply_rustdoc_iframe_theme() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(frame) = document
        .get_element_by_id(RUSTDOC_IFRAME_ID)
        .and_then(|element| element.dyn_into::<web_sys::HtmlIFrameElement>().ok())
    else {
        return;
    };
    let Some(frame_document) = frame.content_document() else {
        return;
    };
    let Some(head) = frame_document.head() else {
        return;
    };

    let element = if let Some(existing) =
        frame_document.get_element_by_id(RUSTDOC_STYLE_ID)
    {
        existing
    } else {
        let Ok(new_element) = frame_document.create_element("style") else {
            return;
        };
        new_element.set_id(RUSTDOC_STYLE_ID);
        let _ = head.append_child(&new_element);
        new_element
    };

    element.set_text_content(Some(RUSTDOC_STYLE_TEXT));
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_rustdoc_iframe_theme() {}

fn availability_label(exists: bool) -> &'static str {
    if exists {
        "available"
    } else {
        "missing"
    }
}

fn artifact_tree_node(artifact: CargoDocArtifact) -> TreeNode {
    let target_name = artifact.target_name.clone();
    let package_name = artifact.package_name.clone();
    let target_kind = artifact.target_kind.join(", ");
    let html_exists = artifact.html_exists;
    let html_path = artifact.html_index_path.clone();
    let json_exists = artifact.rustdoc_json_exists;
    let json_path = artifact.rustdoc_json_path.clone();
    let tooltip_label = format!("{package_name}::{target_name}");
    let tooltip_kind = target_kind.clone();
    let tooltip_html_status = availability_label(html_exists).to_string();
    let tooltip_json_status = availability_label(json_exists).to_string();

    TreeNode {
        id: artifact_id(&artifact),
        label: target_name,
        badge: Some(target_kind),
        tooltip: Some(format!(
            "html: {} | json: {}",
            artifact.html_exists,
            artifact.rustdoc_json_exists,
        )),
        tooltip_render: None,
        badge_color: None,
        is_dir: false,
        icon: NodeIcon::Doc,
        children: vec![],
    }
    .with_tooltip_render(move || {
        rsx! {
            div {
                style: "display: flex; flex-direction: column; gap: 4px; max-width: 360px;",
                div {
                    style: "font-weight: 600; color: var(--text-primary);",
                    "{tooltip_label}"
                }
                div { "kinds: {tooltip_kind}" }
                div { "html: {tooltip_html_status}" }
                if html_exists {
                    div {
                        style: "color: var(--text-muted); font-size: 11px; overflow-wrap: anywhere;",
                        "{html_path}"
                    }
                }
                div { "rustdoc json: {tooltip_json_status}" }
                if json_exists {
                    div {
                        style: "color: var(--text-muted); font-size: 11px; overflow-wrap: anywhere;",
                        "{json_path}"
                    }
                }
            }
        }
    })
}

fn package_tree_node(
    package_id: String,
    package_name: String,
    children: Vec<TreeNode>,
    artifact_count: usize,
    html_count: usize,
    json_count: usize,
) -> TreeNode {
    let tooltip_package_name = package_name.clone();

    TreeNode {
        id: package_id,
        label: package_name,
        badge: Some(artifact_count.to_string()),
        tooltip: Some(format!("{artifact_count} targets")),
        tooltip_render: None,
        badge_color: None,
        is_dir: true,
        icon: NodeIcon::Crate,
        children,
    }
    .with_tooltip_render(move || {
        rsx! {
            div {
                style: "display: flex; flex-direction: column; gap: 4px;",
                div {
                    style: "font-weight: 600; color: var(--text-primary);",
                    "{tooltip_package_name}"
                }
                div { "targets: {artifact_count}" }
                div { "html artifacts: {html_count}" }
                div { "rustdoc json artifacts: {json_count}" }
            }
        }
    })
}

fn build_tree(artifacts: &[CargoDocArtifact]) -> (Vec<TreeNode>, Vec<String>) {
    let mut grouped: BTreeMap<String, Vec<CargoDocArtifact>> = BTreeMap::new();
    for artifact in artifacts {
        grouped
            .entry(artifact.package_name.clone())
            .or_default()
            .push(artifact.clone());
    }

    let mut expanded = Vec::new();
    let mut nodes = Vec::new();

    for (package_name, mut package_artifacts) in grouped {
        package_artifacts.sort_by(|left, right| left.target_name.cmp(&right.target_name));
        let artifact_count = package_artifacts.len();
        let html_count = package_artifacts.iter().filter(|artifact| artifact.html_exists).count();
        let json_count = package_artifacts
            .iter()
            .filter(|artifact| artifact.rustdoc_json_exists)
            .count();
        let package_id = format!("package::{package_name}");
        expanded.push(package_id.clone());
        let children = package_artifacts
            .into_iter()
            .map(artifact_tree_node)
            .collect::<Vec<_>>();

        nodes.push(package_tree_node(
            package_id,
            package_name,
            children,
            artifact_count,
            html_count,
            json_count,
        ));
    }

    (nodes, expanded)
}

fn spawn_fetch_json(
    mut tabs_store: TabsStore<OpenArtifactTab>,
    tab_id: String,
    package_name: String,
    target_name: String,
) {
    let already_loaded = tabs_store
        .tabs
        .read()
        .iter()
        .find(|tab| tab.id == tab_id)
        .map(|tab| {
            tab.payload.json_loading || tab.payload.json_content.is_some()
        })
        .unwrap_or(false);
    if already_loaded {
        return;
    }

    tabs_store.tabs.with_mut(|tabs| {
        if let Some(tab) = tabs.iter_mut().find(|tab| tab.id == tab_id) {
            tab.payload.json_loading = true;
            tab.payload.json_error = None;
        }
    });

    spawn(async move {
        match api::fetch_rustdoc_json(&package_name, &target_name).await {
            Ok(json) => tabs_store.tabs.with_mut(|tabs| {
                if let Some(tab) = tabs.iter_mut().find(|tab| tab.id == tab_id) {
                    tab.payload.json_loading = false;
                    tab.payload.json_content = Some(json);
                }
            }),
            Err(err) => tabs_store.tabs.with_mut(|tabs| {
                if let Some(tab) = tabs.iter_mut().find(|tab| tab.id == tab_id) {
                    tab.payload.json_loading = false;
                    tab.payload.json_error = Some(err);
                }
            }),
        }
    });
}

fn open_artifact_tab(
    artifact: CargoDocArtifact,
    mut tabs_store: TabsStore<OpenArtifactTab>,
    mut mobile_sidebar_open: Signal<bool>,
) {
    let tab_id = artifact_id(&artifact);
    let exists = tabs_store.tabs.read().iter().any(|tab| tab.id == tab_id);
    if !exists {
        tabs_store.open(tab_id.clone(), OpenArtifactTab::from_artifact(artifact.clone()));
    } else {
        tabs_store.activate(&tab_id);
    }

    if artifact.rustdoc_json_exists {
        spawn_fetch_json(
            tabs_store,
            tab_id.clone(),
            artifact.package_name.clone(),
            artifact.target_name.clone(),
        );
    }

    mobile_sidebar_open.set(false);
}

fn close_tab(
    tab_id: &str,
    mut tabs_store: TabsStore<OpenArtifactTab>,
) {
    if !tabs_store.tabs.read().iter().any(|tab| tab.id == tab_id) {
        return;
    }

    let remaining = tabs_store
        .tabs
        .read()
        .iter()
        .filter(|tab| tab.id != tab_id)
        .cloned()
        .collect::<Vec<_>>();
    let next_active = if tabs_store.active.read().as_deref() == Some(tab_id) {
        remaining.last().map(|tab| tab.id.clone())
    } else {
        tabs_store.active.read().clone()
    };

    tabs_store.set_tabs(remaining, next_active);
}

fn set_active_view(
    tab_id: &str,
    view: ArtifactView,
    mut tabs_store: TabsStore<OpenArtifactTab>,
) {
    tabs_store.tabs.with_mut(|tabs| {
        if let Some(tab) = tabs.iter_mut().find(|tab| tab.id == tab_id) {
            tab.payload.active_view = view;
        }
    });
}

fn spawn_load_index(
    mut workspace: Signal<Option<DocWorkspaceResponse>>,
    mut artifacts: Signal<Vec<CargoDocArtifact>>,
    mut loading: Signal<bool>,
    mut error: Signal<Option<String>>,
    mut tabs_store: TabsStore<OpenArtifactTab>,
) {
    spawn(async move {
        loading.set(true);
        error.set(None);

        match api::load_index().await {
            Ok(index) => {
                let first = index
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.html_exists || artifact.rustdoc_json_exists)
                    .cloned();

                workspace.set(Some(index.workspace));
                artifacts.set(index.artifacts.clone());
                tabs_store.set_tabs(Vec::new(), None);

                if let Some(artifact) = first {
                    let tab = OpenArtifactTab::from_artifact(artifact.clone());
                    let tab_id = tab.id.clone();
                    tabs_store.open(tab_id.clone(), tab);

                    if artifact.rustdoc_json_exists {
                        spawn_fetch_json(
                            tabs_store,
                            tab_id,
                            artifact.package_name.clone(),
                            artifact.target_name.clone(),
                        );
                    }
                }
            }
            Err(err) => {
                error.set(Some(format!(
                    "Failed to load doc data: {err}. Start managed doc-viewer or doc-http, or override ?doc_http_base=http://HOST:PORT."
                )));
            }
        }

        loading.set(false);
    });
}

#[component]
pub fn App() -> Element {
    let workspace = use_signal(|| Option::<DocWorkspaceResponse>::None);
    let artifacts = use_signal(Vec::<CargoDocArtifact>::new);
    let loading = use_signal(|| true);
    let error = use_signal(|| Option::<String>::None);
    let tabs_store: TabsStore<OpenArtifactTab> =
        use_hook(|| TabsStore::new());
    let mut sidebar_collapsed = use_signal(|| false);
    let mut mobile_sidebar_open = use_signal(|| false);
    let mut show_theme_settings = use_signal(|| false);

    use_effect(move || {
        spawn_load_index(workspace, artifacts, loading, error, tabs_store);
    });

    let artifact_nodes = build_tree(&artifacts.read().clone());
    let tree_nodes = artifact_nodes.0;
    let initially_expanded = artifact_nodes.1;
    let active_tab = tabs_store.active_tab().map(|tab| tab.payload);
    let active_tab_kind_label = active_tab
        .as_ref()
        .map(|tab| tab.artifact.target_kind.join(", "))
        .unwrap_or_default();
    let active_package_name = active_tab
        .as_ref()
        .map(|tab| tab.package_name.clone())
        .unwrap_or_default();
    let active_target_name = active_tab
        .as_ref()
        .map(|tab| tab.target_name.clone())
        .unwrap_or_default();
    let active_tab_label = active_tab
        .as_ref()
        .map(|tab| tab.label.clone())
        .unwrap_or_default();
    let active_tab_id_value = active_tab
        .as_ref()
        .map(|tab| tab.id.clone())
        .unwrap_or_default();
    let active_html_path = active_tab
        .as_ref()
        .map(|tab| tab.artifact.html_index_path.clone())
        .unwrap_or_default();
    let active_json_path = active_tab
        .as_ref()
        .map(|tab| tab.artifact.rustdoc_json_path.clone())
        .unwrap_or_default();
    let active_html_src = active_tab
        .as_ref()
        .map(|tab| api::html_url(&tab.package_name, &tab.target_name))
        .unwrap_or_default();
    let active_view = active_tab
        .as_ref()
        .map(|tab| tab.active_view)
        .unwrap_or(ArtifactView::Html);
    let active_html_exists = active_tab
        .as_ref()
        .map(|tab| tab.artifact.html_exists)
        .unwrap_or(false);
    let active_json_exists = active_tab
        .as_ref()
        .map(|tab| tab.artifact.rustdoc_json_exists)
        .unwrap_or(false);
    let active_json_loading = active_tab
        .as_ref()
        .map(|tab| tab.json_loading)
        .unwrap_or(false);
    let active_json_content = active_tab
        .as_ref()
        .and_then(|tab| tab.json_content.clone());
    let active_json_error = active_tab
        .as_ref()
        .and_then(|tab| tab.json_error.clone());
    let html_tab_id = active_tab_id_value.clone();
    let json_tab_id = active_tab_id_value.clone();
    let json_package_name = active_package_name.clone();
    let json_target_name = active_target_name.clone();

    let artifacts_for_select = artifacts.read().clone();
    let tab_items = tabs_store
        .tabs
        .read()
        .iter()
        .map(|tab| {
            let mut item = TabItem::new(tab.id.clone(), tab.payload.label.clone());
            item.closeable = true;
            item
        })
        .collect::<Vec<_>>();

    rsx! {
        style { "html, body, #main {{ overflow: hidden; margin: 0; padding: 0; width: 100%; height: 100%; }}" }
        ThemeProvider {
            ViewerShell {
                WgpuOverlay {}
                Layout {
                    header: rsx! {
                        PageHeader {
                            lead: Some(rsx! {
                                button {
                                    class: if *sidebar_collapsed.read() { "btn btn-icon btn-active" } else { "btn btn-icon" },
                                    aria_label: "Toggle sidebar",
                                    title: "Toggle sidebar",
                                    onclick: move |_| {
                                        if viewer_api_dioxus::is_mobile_sidebar_viewport() {
                                            mobile_sidebar_open.set(!mobile_sidebar_open());
                                        } else {
                                            sidebar_collapsed.set(!sidebar_collapsed());
                                        }
                                    },
                                    HamburgerIcon {}
                                }
                            }),
                            icon: Some(rsx! { "Docs" }),
                            title: Some("Doc Viewer".to_string()),
                            subtitle: workspace
                                .read()
                                .as_ref()
                                .map(|workspace| format!("{} packages", workspace.package_count)),
                            on_refresh: Some(EventHandler::new(move |_| {
                                spawn_load_index(workspace, artifacts, loading, error, tabs_store);
                            })),
                            on_theme_toggle: Some(EventHandler::new(move |_| {
                                let next = !*show_theme_settings.read();
                                show_theme_settings.set(next);
                            })),
                        }
                    },
                    Sidebar {
                        title: Some("Generated Docs".to_string()),
                        badge: Some(artifacts.read().len().to_string()),
                        collapsed: *sidebar_collapsed.read(),
                        on_toggle: move |_| sidebar_collapsed.set(!sidebar_collapsed()),
                        mobile_open: Some(*mobile_sidebar_open.read()),
                        on_mobile_open_change: move |open| mobile_sidebar_open.set(open),
                        div {
                            class: "doc-browser__sidebar-help",
                            "Workspace packages and targets discovered from doc-http."
                        }
                        if *loading.read() {
                            div { class: "doc-browser__loading", "Loading generated-doc index..." }
                        } else if let Some(err) = error.read().as_ref() {
                            div { class: "doc-browser__error", "{err}" }
                        } else if tree_nodes.is_empty() {
                            div { class: "doc-browser__empty", "No generated-doc artifacts were found." }
                        } else {
                            TreeView {
                                nodes: tree_nodes,
                                selected_id: tabs_store.active.read().clone(),
                                initially_expanded,
                                on_select: move |id: String| {
                                    if let Some(artifact) = artifacts_for_select.iter().find(|artifact| artifact_id(artifact) == id).cloned() {
                                        open_artifact_tab(artifact, tabs_store, mobile_sidebar_open);
                                    }
                                },
                            }
                        }
                    }
                    div {
                        class: "doc-browser__content",
                        div { class: "doc-browser__body",
                            if let Some(workspace) = workspace.read().as_ref() {
                                div { class: "doc-browser__meta",
                                    span { class: "doc-browser__chip", "workspace: {workspace.workspace_root}" }
                                    span { class: "doc-browser__chip", "manifest: {workspace.workspace_manifest_path}" }
                                    span { class: "doc-browser__chip", "target: {workspace.target_directory}" }
                                }
                            }

                            if !tab_items.is_empty() {
                                TabBar {
                                    tabs: tab_items,
                                    active_id: tabs_store.active.read().clone().unwrap_or_default(),
                                    on_select: move |tab_id: String| {
                                        let mut tabs_store = tabs_store;
                                        tabs_store.activate(&tab_id);
                                    },
                                    on_close: move |tab_id: String| close_tab(&tab_id, tabs_store),
                                }
                            }

                            if *loading.read() && active_tab.is_none() {
                                div { class: "doc-browser__loading", "Loading generated-doc browser..." }
                            } else if let Some(err) = error.read().as_ref() {
                                div { class: "doc-browser__error", "{err}" }
                            } else if active_tab.is_some() {
                                Breadcrumbs {
                                    items: vec![
                                        BreadcrumbItem::link(active_package_name.clone(), EventHandler::new(move |_| {})),
                                        BreadcrumbItem::current(active_target_name.clone()),
                                    ]
                                }

                                div { class: "doc-browser__meta",
                                    span { class: "doc-browser__chip", "kinds: {active_tab_kind_label}" }
                                    span { class: "doc-browser__chip", "html: {active_html_path}" }
                                    span { class: "doc-browser__chip", "json: {active_json_path}" }
                                }

                                div { class: "doc-browser__mode-row",
                                    if active_html_exists {
                                        button {
                                            class: if active_view == ArtifactView::Html { "doc-browser__mode-btn doc-browser__mode-btn--active" } else { "doc-browser__mode-btn" },
                                            onclick: move |_| set_active_view(&html_tab_id, ArtifactView::Html, tabs_store),
                                            "HTML"
                                        }
                                    }
                                    if active_json_exists {
                                        button {
                                            class: if active_view == ArtifactView::Json { "doc-browser__mode-btn doc-browser__mode-btn--active" } else { "doc-browser__mode-btn" },
                                            onclick: move |_| {
                                                set_active_view(&json_tab_id, ArtifactView::Json, tabs_store);
                                                spawn_fetch_json(tabs_store, json_tab_id.clone(), json_package_name.clone(), json_target_name.clone());
                                            },
                                            "Rustdoc JSON"
                                        }
                                    }
                                }

                                if active_view == ArtifactView::Html && active_html_exists {
                                    iframe {
                                        id: RUSTDOC_IFRAME_ID,
                                        class: "doc-browser__frame",
                                        src: "{active_html_src}",
                                        title: "{active_tab_label}",
                                        onload: move |_| apply_rustdoc_iframe_theme(),
                                    }
                                } else if active_json_loading {
                                    div { class: "doc-browser__loading", "Loading rustdoc JSON..." }
                                } else if let Some(json) = active_json_content.as_ref() {
                                    pre { class: "doc-browser__json", "{json}" }
                                } else if let Some(err) = active_json_error.as_ref() {
                                    div { class: "doc-browser__error", "Failed to load rustdoc JSON: {err}" }
                                } else {
                                    div { class: "doc-browser__empty", "No renderable artifact is available for this target." }
                                }
                            } else {
                                div { class: "doc-browser__empty", "Select a generated doc target from the sidebar to browse its HTML or rustdoc JSON output." }
                            }
                        }
                    }
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
    }
}