use gloo_net::http::Request;
use percent_encoding::{
    utf8_percent_encode,
    NON_ALPHANUMERIC,
};
use serde::de::DeserializeOwned;
use viewer_api_dioxus::with_session;

use crate::types::{
    ArtifactListResponse,
    DocIndexResponse,
    DocWorkspaceResponse,
};

fn session_headers() -> Vec<(String, String)> {
    with_session(Vec::new())
}

fn api_base() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(search) = window.location().search() {
                for pair in search.trim_start_matches('?').split('&') {
                    let mut parts = pair.splitn(2, '=');
                    if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                        if key == "doc_http_base" && !value.is_empty() {
                            return value.to_string();
                        }
                    }
                }
            }
        }
    }

    String::new()
}

fn api_url(path: &str) -> String {
    format!("{}{path}", api_base())
}

async fn get_json<T>(path: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let mut request = Request::get(&api_url(path));
    for (name, value) in session_headers() {
        request = request.header(&name, &value);
    }

    let response = request
        .send()
        .await
        .map_err(|err| format!("fetch error: {err}"))?;

    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }

    response
        .json::<T>()
        .await
        .map_err(|err| format!("json decode error: {err}"))
}

async fn get_text(path: &str) -> Result<String, String> {
    let mut request = Request::get(&api_url(path));
    for (name, value) in session_headers() {
        request = request.header(&name, &value);
    }

    let response = request
        .send()
        .await
        .map_err(|err| format!("fetch error: {err}"))?;

    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }

    response
        .text()
        .await
        .map_err(|err| format!("text decode error: {err}"))
}

pub async fn load_index() -> Result<DocIndexResponse, String> {
    let workspace = get_json::<DocWorkspaceResponse>("/api/docs/workspace").await?;
    let artifacts = get_json::<ArtifactListResponse>("/api/docs/artifacts").await?;
    Ok(DocIndexResponse {
        workspace,
        artifacts: artifacts.artifacts,
    })
}

pub fn html_url(
    package_name: &str,
    target_name: &str,
) -> String {
    let package = utf8_percent_encode(package_name, NON_ALPHANUMERIC);
    let target = utf8_percent_encode(target_name, NON_ALPHANUMERIC);
    api_url(&format!("/api/docs/artifacts/{package}/{target}/html"))
}

pub async fn fetch_rustdoc_json(
    package_name: &str,
    target_name: &str,
) -> Result<String, String> {
    let package = utf8_percent_encode(package_name, NON_ALPHANUMERIC);
    let target = utf8_percent_encode(target_name, NON_ALPHANUMERIC);
    get_text(&format!(
        "/api/docs/artifacts/{package}/{target}/rustdoc-json"
    ))
    .await
}