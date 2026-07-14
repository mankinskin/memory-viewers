//! HTTP server module for doc-viewer frontend.
//!
//! Provides REST API endpoints for browsing and reading documentation.

use serde::{
    Deserialize,
    Serialize,
};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::Arc,
};
use viewer_api::{
    axum::{
        extract::{
            Path,
            Query,
            State,
        },
        http::{
            HeaderMap,
            StatusCode,
        },
        response::Json,
        routing::{
            get,
            post,
        },
        Router,
    },
    session::{
        get_session_id,
        SessionConfig,
        SessionConfigUpdate,
        SessionStore,
        SESSION_HEADER,
    },
    tower_http::{
        cors::{
            Any,
            CorsLayer,
        },
        services::ServeDir,
    },
    tracing::info,
};

use crate::{
    helpers::{
        normalize_path_str,
        to_vscode_file_uri,
        unix_path,
    },
    markdown_ast,
    query,
    schema::{
        DocType,
        ModuleTreeNode,
    },
    tools::{
        CrateDocsManager,
        DetailLevel,
        DocsManager,
        ListFilter,
    },
};

/// Application state shared across HTTP handlers.
#[derive(Clone)]
pub struct HttpState {
    pub docs_manager: Arc<DocsManager>,
    pub crate_manager: Arc<CrateDocsManager>,
    pub sessions: SessionStore,
}

// === API Response Types ===

#[derive(Serialize)]
struct ApiError {
    error: String,
}

#[derive(Serialize)]
struct DocListResponse {
    total: usize,
    categories: Vec<CategoryResponse>,
}

#[derive(Serialize)]
struct CategoryResponse {
    category: String,
    count: usize,
    docs: Vec<DocSummaryResponse>,
}

#[derive(Serialize)]
struct DocSummaryResponse {
    filename: String,
    title: String,
    date: String,
    summary: String,
    tags: Vec<String>,
    status: Option<String>,
}

#[derive(Serialize)]
struct DocContentResponse {
    filename: String,
    doc_type: String,
    title: String,
    date: String,
    summary: String,
    tags: Vec<String>,
    status: Option<String>,
    body: Option<String>,
}

#[derive(Serialize)]
struct CrateListResponse {
    crates: Vec<CrateSummaryResponse>,
}

#[derive(Serialize)]
struct CrateSummaryResponse {
    name: String,
    version: Option<String>,
    description: String,
    module_count: usize,
    has_readme: bool,
    /// Absolute path to the crate root directory
    crate_path: String,
}

#[derive(Serialize)]
struct CrateTreeResponse {
    name: String,
    description: String,
    children: Vec<ModuleNodeResponse>,
    /// Source files at the crate root level
    source_files: Vec<SourceFileLinkResponse>,
    /// Crate root path for file resolution
    crate_path: String,
}

#[derive(Serialize)]
struct ModuleNodeResponse {
    name: String,
    path: String,
    description: String,
    has_readme: bool,
    children: Vec<ModuleNodeResponse>,
    /// Source files within this module
    source_files: Vec<SourceFileLinkResponse>,
}

#[derive(Serialize)]
struct CrateDocResponse {
    crate_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    module_path: Option<String>,
    index_yaml: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    readme: Option<String>,
    /// Absolute path to the crate root directory
    crate_path: String,
    /// Source files with absolute paths and editor URIs
    source_files: Vec<SourceFileLinkResponse>,
}

#[derive(Serialize)]
struct SourceFileLinkResponse {
    /// Relative path from crate root (e.g., "src/lib.rs")
    rel_path: String,
    /// Absolute filesystem path
    abs_path: String,
    /// VS Code URI to open the file (vscode://file/...)
    vscode_uri: String,
}

#[derive(Serialize)]
struct JqQueryResponse {
    query: String,
    total: usize,
    results: Vec<Value>,
}

// === Query Parameters ===

#[derive(Deserialize)]
struct JqQueryParams {
    jq: String,
    #[serde(default)]
    doc_type: Option<String>,
    /// If true, transform values (can produce multiple outputs). Default is filter (select matching).
    #[serde(default)]
    transform: bool,
    /// If true, include parsed markdown AST in "content" field for each document.
    #[serde(default)]
    include_content: bool,
}

#[derive(Deserialize)]
struct ListDocsQuery {
    doc_type: Option<String>,
    tag: Option<String>,
}

#[derive(Deserialize)]
struct ReadDocQuery {
    detail: Option<String>,
}

#[derive(Deserialize)]
struct ReadCrateDocQuery {
    module: Option<String>,
    include_readme: Option<bool>,
}

// === Router Creation ===

/// Create the HTTP router with all API endpoints.
pub fn create_router(
    state: HttpState,
    static_dir: Option<PathBuf>,
) -> Router {
    // CORS for development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // API routes
    let api_routes = Router::new()
        .route("/docs", get(list_docs))
        .route("/docs/{filename}", get(read_doc))
        .route("/docs/{filename}/ast", get(get_doc_ast))
        .route("/crates", get(list_crates))
        .route("/crates/{name}", get(browse_crate))
        .route("/crates/{name}/doc", get(read_crate_doc))
        .route("/source/{*path}", get(read_source_file))
        .route("/query", post(query_docs))
        .route("/session", get(get_session).post(update_session));

    // Main app
    let app = Router::new()
        .nest("/api", api_routes)
        .layer(cors)
        .with_state(state);

    // Add static file serving if directory provided
    if let Some(dir) = static_dir {
        app.fallback_service(ServeDir::new(dir))
    } else {
        app
    }
}

// === Handlers ===

/// GET /api/docs - List all documentation
async fn list_docs(
    State(state): State<HttpState>,
    Query(params): Query<ListDocsQuery>,
) -> Result<Json<DocListResponse>, (StatusCode, Json<ApiError>)> {
    let doc_types = match params.doc_type.as_deref() {
        Some(dt) => match parse_doc_type(dt) {
            Some(t) => vec![t],
            None =>
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ApiError {
                        error: format!("Invalid doc_type: {}", dt),
                    }),
                )),
        },
        None => vec![
            DocType::Guide,
            DocType::Plan,
            DocType::Implemented,
            DocType::BugReport,
            DocType::Analysis,
        ],
    };

    let filter = ListFilter {
        tag: params.tag,
        status: None,
    };

    let mut categories = Vec::new();
    let mut total = 0;

    for dt in doc_types {
        match state.docs_manager.list_documents_filtered(dt, &filter) {
            Ok(docs) => {
                let count = docs.len();
                total += count;

                let category = CategoryResponse {
                    category: dt.directory().to_string(),
                    count,
                    docs: docs
                        .into_iter()
                        .map(|d| DocSummaryResponse {
                            filename: d.filename,
                            title: d.title,
                            date: d.date,
                            summary: d.summary,
                            tags: d.tags,
                            status: d.status.map(|s| s.to_string()),
                        })
                        .collect(),
                };
                categories.push(category);
            },
            Err(e) =>
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError {
                        error: e.to_string(),
                    }),
                )),
        }
    }

    info!(total, categories = categories.len(), doc_type = ?params.doc_type, tag = ?filter.tag, "Listed docs");
    Ok(Json(DocListResponse { total, categories }))
}

/// GET /api/docs/:filename - Read a specific document
async fn read_doc(
    State(state): State<HttpState>,
    Path(filename): Path<String>,
    Query(params): Query<ReadDocQuery>,
) -> Result<Json<DocContentResponse>, (StatusCode, Json<ApiError>)> {
    let detail = match params.detail.as_deref() {
        Some("outline") => DetailLevel::Outline,
        Some("full") => DetailLevel::Full,
        _ => DetailLevel::Full, // Default to full for viewing
    };

    match state.docs_manager.read_document(&filename, detail) {
        Ok(result) => {
            info!(filename = %filename, doc_type = %result.doc_type, "Read doc");
            Ok(Json(DocContentResponse {
                filename: result.filename,
                doc_type: result.doc_type,
                title: result.title,
                date: result.date,
                summary: result.summary,
                tags: result.tags,
                status: result.status.map(|s| s.to_string()),
                body: result.body,
            }))
        },
        Err(e) => {
            let status = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            Err((
                status,
                Json(ApiError {
                    error: e.to_string(),
                }),
            ))
        },
    }
}

/// GET /api/crates - List all documented crates
async fn list_crates(
    State(state): State<HttpState>
) -> Result<Json<CrateListResponse>, (StatusCode, Json<ApiError>)> {
    match state.crate_manager.discover_crates_with_diagnostics() {
        Ok(result) => {
            info!(count = result.crates.len(), "Listed crates");
            Ok(Json(CrateListResponse {
                crates: result
                    .crates
                    .into_iter()
                    .map(|c| CrateSummaryResponse {
                        name: c.name,
                        version: c.version,
                        description: c.description,
                        module_count: c.module_count,
                        has_readme: c.has_readme,
                        crate_path: c.crate_path,
                    })
                    .collect(),
            }))
        },
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: e.to_string(),
            }),
        )),
    }
}

/// GET /api/crates/:name - Browse a specific crate's module tree
async fn browse_crate(
    State(state): State<HttpState>,
    Path(name): Path<String>,
) -> Result<Json<CrateTreeResponse>, (StatusCode, Json<ApiError>)> {
    match state.crate_manager.browse_crate(&name) {
        Ok(tree) => {
            let crate_path = state
                .crate_manager
                .get_crate_path(&name)
                .map(|p| unix_path(&p))
                .unwrap_or_default();

            // Scan for source files at the crate root (src/ and agents/docs/)
            let root_source_files = scan_crate_source_files(&crate_path, None);

            info!(crate_name = %name, modules = tree.children.len(), files = root_source_files.len(), "Browsed crate");
            Ok(Json(CrateTreeResponse {
                name: tree.name.clone(),
                description: tree.description.clone(),
                children: tree
                    .children
                    .iter()
                    .map(|n| convert_module_node(n, &crate_path))
                    .collect(),
                source_files: root_source_files,
                crate_path,
            }))
        },
        Err(e) => {
            let status = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            Err((
                status,
                Json(ApiError {
                    error: e.to_string(),
                }),
            ))
        },
    }
}

/// GET /api/crates/:name/doc - Read crate or module documentation
async fn read_crate_doc(
    State(state): State<HttpState>,
    Path(name): Path<String>,
    Query(params): Query<ReadCrateDocQuery>,
) -> Result<Json<CrateDocResponse>, (StatusCode, Json<ApiError>)> {
    let include_readme = params.include_readme.unwrap_or(true);

    match state.crate_manager.read_crate_doc(
        &name,
        params.module.as_deref(),
        include_readme,
    ) {
        Ok(result) => {
            info!(crate_name = %name, module = ?params.module, "Read crate doc");
            Ok(Json(CrateDocResponse {
                crate_name: result.crate_name,
                module_path: result.module_path,
                index_yaml: result.index_yaml,
                readme: result.readme,
                crate_path: result.crate_path,
                source_files: result
                    .source_files
                    .into_iter()
                    .map(|f| SourceFileLinkResponse {
                        rel_path: f.rel_path,
                        abs_path: f.abs_path,
                        vscode_uri: f.vscode_uri,
                    })
                    .collect(),
            }))
        },
        Err(e) => {
            let status = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            Err((
                status,
                Json(ApiError {
                    error: e.to_string(),
                }),
            ))
        },
    }
}

fn convert_module_node(
    node: &ModuleTreeNode,
    crate_path: &str,
) -> ModuleNodeResponse {
    // Scan source files for this module (src/module_name/ and agents/docs/module_path/)
    let source_files = scan_crate_source_files(crate_path, Some(&node.path));

    ModuleNodeResponse {
        name: node.name.clone(),
        path: node.path.clone(),
        description: node.description.clone(),
        has_readme: node.has_readme,
        children: node
            .children
            .iter()
            .map(|n| convert_module_node(n, crate_path))
            .collect(),
        source_files,
    }
}

/// Scan for source files in a crate or module directory
fn scan_crate_source_files(
    crate_path: &str,
    module_path: Option<&str>,
) -> Vec<SourceFileLinkResponse> {
    use std::path::Path;

    let crate_dir = Path::new(crate_path);
    let mut files = Vec::new();

    // Determine which directories to scan based on module_path
    let (src_dir, docs_dir) = match module_path {
        None => {
            // Root level: scan src/ root files and agents/docs/ index.yaml
            (crate_dir.join("src"), crate_dir.join("agents").join("docs"))
        },
        Some(mod_path) => {
            // Module level: scan src/module_path/ and agents/docs/module_path/
            (
                crate_dir.join("src").join(mod_path),
                crate_dir.join("agents").join("docs").join(mod_path),
            )
        },
    };

    // Scan src/ directory for .rs files (only direct children, not recursive)
    scan_dir_by_extension(&src_dir, crate_dir, &["rs"], &mut files);

    // Scan agents/docs/ directory for .yaml and .md files
    scan_dir_by_extension(
        &docs_dir,
        crate_dir,
        &["yaml", "yml", "md"],
        &mut files,
    );

    // Sort files by name
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    files
}

/// Push `SourceFileLinkResponse` entries for direct-child files in `dir`
/// whose extension matches one of `exts`. Paths are relativized to `crate_dir`.
fn scan_dir_by_extension(
    dir: &std::path::Path,
    crate_dir: &std::path::Path,
    exts: &[&str],
    files: &mut Vec<SourceFileLinkResponse>,
) {
    if !(dir.exists() && dir.is_dir()) {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension() else {
            continue;
        };
        if !exts.iter().any(|e| ext == *e) {
            continue;
        }
        let rel_path = path
            .strip_prefix(crate_dir)
            .map(|p| normalize_path_str(&p.to_string_lossy()))
            .unwrap_or_default();
        files.push(SourceFileLinkResponse {
            rel_path,
            abs_path: unix_path(&path),
            vscode_uri: to_vscode_file_uri(&path),
        });
    }
}

// === Helpers ===

fn parse_doc_type(s: &str) -> Option<DocType> {
    match s.to_lowercase().as_str() {
        "guide" | "guides" => Some(DocType::Guide),
        "plan" | "plans" => Some(DocType::Plan),
        "implemented" => Some(DocType::Implemented),
        "bug-report" | "bug-reports" | "bug_report" | "bugreport" =>
            Some(DocType::BugReport),
        "analysis" => Some(DocType::Analysis),
        _ => None,
    }
}

/// POST /api/query - Query documents using JQ expressions
///
/// Example queries:
/// - `select(.doc_type == "guide")` - Filter guides
/// - `select(.tags | any(. == "testing"))` - Filter by tag
/// - `select(.title | test("search"; "i"))` - Regex in title
/// - `{title, date, tags}` - Extract specific fields
async fn query_docs(
    State(state): State<HttpState>,
    Json(params): Json<JqQueryParams>,
) -> Result<Json<JqQueryResponse>, (StatusCode, Json<ApiError>)> {
    // Collect all docs into JSON values
    let doc_types = match params.doc_type.as_deref() {
        Some(dt) => match parse_doc_type(dt) {
            Some(t) => vec![t],
            None =>
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ApiError {
                        error: format!("Invalid doc_type: {}", dt),
                    }),
                )),
        },
        None => vec![
            DocType::Guide,
            DocType::Plan,
            DocType::Implemented,
            DocType::BugReport,
            DocType::Analysis,
        ],
    };

    let filter = ListFilter::default();
    let mut all_docs: Vec<Value> = Vec::new();

    for dt in doc_types {
        match state.docs_manager.list_documents_filtered(dt, &filter) {
            Ok(docs) => collect_query_docs(
                &state,
                dt,
                docs,
                params.include_content,
                &mut all_docs,
            ),
            Err(e) =>
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError {
                        error: e.to_string(),
                    }),
                )),
        }
    }

    // Apply JQ query
    let results = if params.transform {
        query::transform_values(all_docs.iter(), &params.jq)
    } else {
        query::filter_values(all_docs.iter(), &params.jq)
    };

    match results {
        Ok(values) => {
            info!(query = %params.jq, results = values.len(), transform = params.transform, "Query docs");
            Ok(Json(JqQueryResponse {
                query: params.jq,
                total: values.len(),
                results: values,
            }))
        },
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: format!("JQ query error: {}", e.message),
            }),
        )),
    }
}

/// Convert listed documents of one `DocType` into JSON values for JQ querying,
/// optionally including parsed markdown content, appending to `all_docs`.
fn collect_query_docs(
    state: &HttpState,
    dt: DocType,
    docs: Vec<crate::tools::agents::DocSummary>,
    include_content: bool,
    all_docs: &mut Vec<Value>,
) {
    for d in docs {
        let mut doc_json = serde_json::json!({
            "doc_type": dt.directory(),
            "filename": d.filename,
            "title": d.title,
            "date": d.date,
            "summary": d.summary,
            "tags": d.tags,
            "status": d.status.map(|s| s.to_string()),
        });

        if include_content {
            if let Some(ast) = read_doc_content_ast(state, &d.filename) {
                doc_json["content"] = ast;
            }
        }

        all_docs.push(doc_json);
    }
}

/// Read a document body and parse it into a markdown-AST JSON value.
fn read_doc_content_ast(
    state: &HttpState,
    filename: &str,
) -> Option<Value> {
    let result = state
        .docs_manager
        .read_document(filename, DetailLevel::Full)
        .ok()?;
    let body = result.body.as_ref()?;
    markdown_ast::parse_markdown_to_json(body).ok()
}

/// GET /api/docs/:filename/ast - Get markdown AST for a document
///
/// Returns the document with its content parsed into a structured AST.
/// Useful for querying document structure with JQ.
async fn get_doc_ast(
    State(state): State<HttpState>,
    Path(filename): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<ApiError>)> {
    match state
        .docs_manager
        .read_document(&filename, DetailLevel::Full)
    {
        Ok(result) => {
            let content_ast = result.body.as_ref().and_then(|body| {
                markdown_ast::parse_markdown_to_json(body).ok()
            });

            info!(filename = %filename, "Get doc AST");
            Ok(Json(serde_json::json!({
                "filename": result.filename,
                "doc_type": result.doc_type,
                "title": result.title,
                "date": result.date,
                "summary": result.summary,
                "tags": result.tags,
                "status": result.status.map(|s| s.to_string()),
                "content": content_ast,
            })))
        },
        Err(e) => {
            let status = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            Err((
                status,
                Json(ApiError {
                    error: e.to_string(),
                }),
            ))
        },
    }
}

// === Session Handlers ===

/// GET /api/session - Get current session configuration
async fn get_session(
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Json<SessionConfig>, (StatusCode, Json<ApiError>)> {
    let session_id = get_session_id(&headers).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: format!("Missing {} header", SESSION_HEADER),
            }),
        )
    })?;

    let config = state.sessions.get_or_create(session_id);
    info!(session_id = %session_id, "Get session");
    Ok(Json(config))
}

/// POST /api/session - Update session configuration
async fn update_session(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(update): Json<SessionConfigUpdate>,
) -> Result<Json<SessionConfig>, (StatusCode, Json<ApiError>)> {
    let session_id = get_session_id(&headers).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: format!("Missing {} header", SESSION_HEADER),
            }),
        )
    })?;

    // Ensure session exists
    state.sessions.get_or_create(session_id);

    // Update configuration
    let config = state
        .sessions
        .update(session_id, |config| {
            if let Some(verbose) = update.verbose {
                config.verbose = verbose;
            }
            if let Some(data) = &update.data {
                for (key, value) in data {
                    config.data.insert(key.clone(), value.clone());
                }
            }
        })
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: "Session not found after creation".to_string(),
                }),
            )
        })?;

    info!(session_id = %session_id, verbose = config.verbose, "Session updated");
    Ok(Json(config))
}

// === Source File Handler ===

#[derive(Serialize)]
struct SourceFileResponse {
    path: String,
    content: String,
    language: String,
    total_lines: usize,
}

/// Detect language from file extension
fn detect_language(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "html" => "html",
        "css" => "css",
        _ => "plaintext",
    }
}

/// GET /api/source/*path - Read a source file
/// The path should be an absolute filesystem path
async fn read_source_file(
    State(state): State<HttpState>,
    Path(path): Path<String>,
) -> Result<Json<SourceFileResponse>, (StatusCode, Json<ApiError>)> {
    // Normalize path separators for cross-platform support
    let normalized_path = path.replace('/', std::path::MAIN_SEPARATOR_STR);
    let file_path = PathBuf::from(&normalized_path);

    // Security check: path must be within one of the crates directories
    let crates_dirs = state.crate_manager.crates_dirs();
    let is_allowed = crates_dirs.iter().any(|dir| file_path.starts_with(dir));

    if !is_allowed {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiError {
                error: "Access denied: path outside allowed directories"
                    .to_string(),
            }),
        ));
    }

    // Additional check: no path traversal
    if normalized_path.contains("..") {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiError {
                error: "Path traversal not allowed".to_string(),
            }),
        ));
    }

    // Read file content
    let content = std::fs::read_to_string(&file_path).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: format!("Failed to read file: {}", e),
            }),
        )
    })?;

    let total_lines = content.lines().count();
    let language = detect_language(&normalized_path).to_string();

    info!(path = %normalized_path, lines = total_lines, language = %language, "Read source file");

    Ok(Json(SourceFileResponse {
        path: normalized_path,
        content,
        language,
        total_lines,
    }))
}
