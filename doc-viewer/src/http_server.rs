//! HTTP server for doc-viewer frontend.
//!
//! Provides REST API endpoints for browsing and reading documentation.

mod git;
mod parser;
mod schema;
mod templates;
mod tools;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, patch, post},
    Router,
};
use schema::{DocType, PlanStatus};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tools::{
    agents::{CreateDocParams, UpdateMetaParams},
    CrateDocsManager, DetailLevel, DocsManager, ListFilter,
};
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};

/// Application state shared across handlers.
#[derive(Clone)]
struct AppState {
    docs_manager: Arc<DocsManager>,
    crate_manager: Arc<CrateDocsManager>,
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
}

#[derive(Serialize)]
struct CrateTreeResponse {
    name: String,
    description: String,
    children: Vec<ModuleNodeResponse>,
}

#[derive(Serialize)]
struct ModuleNodeResponse {
    name: String,
    path: String,
    description: String,
    has_readme: bool,
    children: Vec<ModuleNodeResponse>,
}

#[derive(Serialize)]
struct CrateDocResponse {
    crate_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    module_path: Option<String>,
    index_yaml: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    readme: Option<String>,
}

// === New API Response Types ===

#[derive(Serialize)]
struct CreateDocResponse {
    filename: String,
    path: String,
}

#[derive(Serialize)]
struct SearchResultsResponse {
    query: String,
    total_matches: usize,
    files_searched: usize,
    results: Vec<SearchMatchResponse>,
}

#[derive(Serialize)]
struct SearchMatchResponse {
    filename: String,
    doc_type: String,
    match_count: usize,
    excerpts: Vec<ExcerptResponse>,
}

#[derive(Serialize)]
struct ExcerptResponse {
    line_number: usize,
    line: String,
    context_before: Vec<String>,
    context_after: Vec<String>,
}

#[derive(Serialize)]
struct ValidationResponse {
    documents_checked: usize,
    errors: usize,
    warnings: usize,
    issues: Vec<ValidationIssueResponse>,
}

#[derive(Serialize)]
struct ValidationIssueResponse {
    file: String,
    category: String,
    issue: String,
    severity: String,
}

#[derive(Serialize)]
struct HealthDashboardResponse {
    total_documents: usize,
    frontmatter_coverage: f32,
    index_sync_rate: f32,
    naming_compliance: f32,
    categories: Vec<CategoryHealthResponse>,
    recommendations: Vec<String>,
}

#[derive(Serialize)]
struct CategoryHealthResponse {
    name: String,
    doc_count: usize,
    has_frontmatter: usize,
    has_valid_name: usize,
    in_index: usize,
}

// === Query Parameters ===

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
struct SearchQuery {
    query: Option<String>,
    tag: Option<String>,
    doc_type: Option<String>,
    search_content: Option<bool>,
    lines_before: Option<usize>,
    lines_after: Option<usize>,
}

// === Request Bodies ===

#[derive(Deserialize)]
struct CreateDocRequest {
    doc_type: String,
    name: String,
    title: String,
    summary: String,
    tags: Vec<String>,
    status: Option<String>,
}

#[derive(Deserialize)]
struct UpdateMetaRequest {
    tags: Option<Vec<String>>,
    summary: Option<String>,
    status: Option<String>,
}

// === Handlers ===

/// GET /api/docs - List all documentation
async fn list_docs(
    State(state): State<AppState>,
    Query(params): Query<ListDocsQuery>,
) -> Result<Json<DocListResponse>, (StatusCode, Json<ApiError>)> {
    let doc_types = match params.doc_type.as_deref() {
        Some(dt) => match parse_doc_type(dt) {
            Some(t) => vec![t],
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ApiError {
                        error: format!("Invalid doc_type: {}", dt),
                    }),
                ))
            }
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
            }
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError {
                        error: e.to_string(),
                    }),
                ))
            }
        }
    }

    Ok(Json(DocListResponse { total, categories }))
}

/// GET /api/docs/:filename - Read a specific document
async fn read_doc(
    State(state): State<AppState>,
    Path(filename): Path<String>,
    Query(params): Query<ReadDocQuery>,
) -> Result<Json<DocContentResponse>, (StatusCode, Json<ApiError>)> {
    let detail = match params.detail.as_deref() {
        Some("outline") => DetailLevel::Outline,
        Some("full") => DetailLevel::Full,
        _ => DetailLevel::Full, // Default to full for viewing
    };

    match state.docs_manager.read_document(&filename, detail) {
        Ok(result) => Ok(Json(DocContentResponse {
            filename: result.filename,
            doc_type: result.doc_type,
            title: result.title,
            date: result.date,
            summary: result.summary,
            tags: result.tags,
            status: result.status.map(|s| s.to_string()),
            body: result.body,
        })),
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
        }
    }
}

/// GET /api/crates - List all documented crates
async fn list_crates(
    State(state): State<AppState>,
) -> Result<Json<CrateListResponse>, (StatusCode, Json<ApiError>)> {
    match state.crate_manager.discover_crates_with_diagnostics() {
        Ok(result) => Ok(Json(CrateListResponse {
            crates: result
                .crates
                .into_iter()
                .map(|c| CrateSummaryResponse {
                    name: c.name,
                    version: c.version,
                    description: c.description,
                    module_count: c.module_count,
                    has_readme: c.has_readme,
                })
                .collect(),
        })),
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
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<CrateTreeResponse>, (StatusCode, Json<ApiError>)> {
    match state.crate_manager.browse_crate(&name) {
        Ok(tree) => Ok(Json(CrateTreeResponse {
            name: tree.name.clone(),
            description: tree.description.clone(),
            children: tree.children.iter().map(convert_module_node).collect(),
        })),
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
        }
    }
}

#[derive(Deserialize)]
struct ReadCrateDocQuery {
    module: Option<String>,
    include_readme: Option<bool>,
}

/// GET /api/crates/:name/doc - Read crate or module documentation
async fn read_crate_doc(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(params): Query<ReadCrateDocQuery>,
) -> Result<Json<CrateDocResponse>, (StatusCode, Json<ApiError>)> {
    let include_readme = params.include_readme.unwrap_or(true);
    
    match state.crate_manager.read_crate_doc(
        &name,
        params.module.as_deref(),
        include_readme,
    ) {
        Ok(result) => Ok(Json(CrateDocResponse {
            crate_name: result.crate_name,
            module_path: result.module_path,
            index_yaml: result.index_yaml,
            readme: result.readme,
        })),
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
        }
    }
}

// === New Handlers ===

/// POST /api/docs - Create a new document
async fn create_doc(
    State(state): State<AppState>,
    Json(body): Json<CreateDocRequest>,
) -> Result<Json<CreateDocResponse>, (StatusCode, Json<ApiError>)> {
    let doc_type = parse_doc_type(&body.doc_type).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: format!("Invalid doc_type: {}", body.doc_type),
            }),
        )
    })?;

    let status = body.status.as_ref().and_then(|s| parse_status(s));

    let params = CreateDocParams {
        doc_type,
        name: body.name,
        title: body.title,
        summary: body.summary,
        tags: Some(body.tags),
        status,
    };

    match state.docs_manager.create_document(params) {
        Ok(result) => Ok(Json(CreateDocResponse {
            filename: result.filename,
            path: result.path,
        })),
        Err(e) => {
            let status_code = if e.to_string().contains("already exists") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            Err((
                status_code,
                Json(ApiError {
                    error: e.to_string(),
                }),
            ))
        }
    }
}

/// PATCH /api/docs/:filename - Update document metadata
async fn update_doc_meta(
    State(state): State<AppState>,
    Path(filename): Path<String>,
    Json(body): Json<UpdateMetaRequest>,
) -> Result<Json<DocContentResponse>, (StatusCode, Json<ApiError>)> {
    let status = body.status.as_ref().and_then(|s| parse_status(s));

    let params = UpdateMetaParams {
        filename: filename.clone(),
        tags: body.tags,
        summary: body.summary,
        status,
    };

    match state.docs_manager.update_document_metadata(params) {
        Ok(()) => {
            // Return the updated document
            match state.docs_manager.read_document(&filename, DetailLevel::Full) {
                Ok(result) => Ok(Json(DocContentResponse {
                    filename: result.filename,
                    doc_type: result.doc_type,
                    title: result.title,
                    date: result.date,
                    summary: result.summary,
                    tags: result.tags,
                    status: result.status.map(|s| s.to_string()),
                    body: result.body,
                })),
                Err(e) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError {
                        error: e.to_string(),
                    }),
                )),
            }
        }
        Err(e) => {
            let status_code = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            Err((
                status_code,
                Json(ApiError {
                    error: e.to_string(),
                }),
            ))
        }
    }
}

/// GET /api/docs/search - Search documents
async fn search_docs(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResultsResponse>, (StatusCode, Json<ApiError>)> {
    // At least one of query or tag must be provided
    if params.query.is_none() && params.tag.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "At least one of 'query' or 'tag' must be provided".to_string(),
            }),
        ));
    }

    let doc_type = params.doc_type.as_ref().and_then(|s| parse_doc_type(s));
    let search_content = params.search_content.unwrap_or(false);
    let lines_before = params.lines_before.unwrap_or(2);
    let lines_after = params.lines_after.unwrap_or(2);

    // If searching content with context, use search_content method
    if search_content && params.query.is_some() {
        let filter = ListFilter {
            tag: params.tag.clone(),
            status: None,
        };

        match state.docs_manager.search_content(
            params.query.as_deref().unwrap_or(""),
            doc_type,
            &filter,
            lines_before,
            lines_after,
        ) {
            Ok(result) => Ok(Json(SearchResultsResponse {
                query: result.query,
                total_matches: result.total_matches,
                files_searched: result.files_searched,
                results: result
                    .matches
                    .into_iter()
                    .map(|m| SearchMatchResponse {
                        filename: m.filename,
                        doc_type: m.doc_type,
                        match_count: m.match_count,
                        excerpts: m
                            .excerpts
                            .into_iter()
                            .map(|e| ExcerptResponse {
                                line_number: e.line_number,
                                line: e.line,
                                context_before: e.context_before,
                                context_after: e.context_after,
                            })
                            .collect(),
                    })
                    .collect(),
            })),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: e.to_string(),
                }),
            )),
        }
    } else {
        // Simple search: return matching documents
        match state.docs_manager.search_docs(
            params.query.as_deref(),
            params.tag.as_deref(),
            search_content,
            doc_type,
        ) {
            Ok(docs) => Ok(Json(SearchResultsResponse {
                query: params.query.unwrap_or_default(),
                total_matches: docs.len(),
                files_searched: 0, // Not tracked in simple search
                results: docs
                    .into_iter()
                    .map(|d| SearchMatchResponse {
                        filename: d.filename.clone(),
                        doc_type: String::new(),
                        match_count: 1,
                        excerpts: vec![ExcerptResponse {
                            line_number: 0,
                            line: d.summary,
                            context_before: vec![],
                            context_after: vec![],
                        }],
                    })
                    .collect(),
            })),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: e.to_string(),
                }),
            )),
        }
    }
}

/// GET /api/docs/validate - Validate all documents
async fn validate_docs(
    State(state): State<AppState>,
) -> Result<Json<ValidationResponse>, (StatusCode, Json<ApiError>)> {
    match state.docs_manager.validate() {
        Ok(report) => {
            let errors = report
                .issues
                .iter()
                .filter(|i| matches!(i.severity, tools::agents::IssueSeverity::Error))
                .count();
            let warnings = report.issues.len() - errors;

            Ok(Json(ValidationResponse {
                documents_checked: report.documents_checked,
                errors,
                warnings,
                issues: report
                    .issues
                    .into_iter()
                    .map(|i| ValidationIssueResponse {
                        file: i.file,
                        category: i.category,
                        issue: i.issue,
                        severity: format!("{:?}", i.severity).to_lowercase(),
                    })
                    .collect(),
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: e.to_string(),
            }),
        )),
    }
}

/// GET /api/docs/health - Health dashboard
async fn health_dashboard(
    State(state): State<AppState>,
) -> Result<Json<HealthDashboardResponse>, (StatusCode, Json<ApiError>)> {
    match state.docs_manager.health_dashboard(true) {
        Ok(dashboard) => Ok(Json(HealthDashboardResponse {
            total_documents: dashboard.total_documents,
            frontmatter_coverage: dashboard.frontmatter_coverage,
            index_sync_rate: dashboard.index_sync_rate,
            naming_compliance: dashboard.naming_compliance,
            categories: dashboard
                .categories
                .into_iter()
                .map(|c| CategoryHealthResponse {
                    name: c.name,
                    doc_count: c.doc_count,
                    has_frontmatter: c.has_frontmatter,
                    has_valid_name: c.has_valid_name,
                    in_index: c.in_index,
                })
                .collect(),
            recommendations: dashboard.recommendations,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: e.to_string(),
            }),
        )),
    }
}

fn convert_module_node(node: &schema::ModuleTreeNode) -> ModuleNodeResponse {
    ModuleNodeResponse {
        name: node.name.clone(),
        path: node.path.clone(),
        description: node.description.clone(),
        has_readme: node.has_readme,
        children: node.children.iter().map(convert_module_node).collect(),
    }
}

// === Helpers ===

fn parse_doc_type(s: &str) -> Option<DocType> {
    match s.to_lowercase().as_str() {
        "guide" | "guides" => Some(DocType::Guide),
        "plan" | "plans" => Some(DocType::Plan),
        "implemented" => Some(DocType::Implemented),
        "bug-report" | "bug-reports" | "bug_report" | "bugreport" => Some(DocType::BugReport),
        "analysis" => Some(DocType::Analysis),
        _ => None,
    }
}

fn parse_status(s: &str) -> Option<PlanStatus> {
    match s.to_lowercase().as_str() {
        "active" => Some(PlanStatus::Active),
        "completed" => Some(PlanStatus::Completed),
        "paused" => Some(PlanStatus::Paused),
        "abandoned" => Some(PlanStatus::Abandoned),
        _ => None,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get directories from environment or use defaults
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent() // memory-viewers/
        .and_then(|p| p.parent()) // context-engine/
        .unwrap_or(&manifest_dir);

    let agents_dir = std::env::var("AGENTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root.join("agents"));

    let crates_dirs: Vec<PathBuf> = std::env::var("CRATES_DIRS")
        .or_else(|_| std::env::var("CRATES_DIR"))
        .map(|val| std::env::split_paths(&val).collect())
        .unwrap_or_else(|_| {
            vec![workspace_root.join("crates"), workspace_root.join("tools")]
        });

    // Static files directory
    let static_dir = std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest_dir.join("static"));

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001);

    println!("Doc Viewer HTTP Server starting...");
    println!("Agents directory: {}", agents_dir.display());
    println!("Crates directories:");
    for dir in &crates_dirs {
        println!("  - {}", dir.display());
    }
    println!("Static directory: {}", static_dir.display());
    println!("Port: {}", port);

    let state = AppState {
        docs_manager: Arc::new(DocsManager::new(agents_dir)),
        crate_manager: Arc::new(CrateDocsManager::new(crates_dirs)),
    };

    // CORS for development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // API routes
    let api_routes = Router::new()
        // Docs endpoints
        .route("/docs", get(list_docs).post(create_doc))
        .route("/docs/search", get(search_docs))
        .route("/docs/validate", get(validate_docs))
        .route("/docs/health", get(health_dashboard))
        .route("/docs/{filename}", get(read_doc).patch(update_doc_meta))
        // Crates endpoints
        .route("/crates", get(list_crates))
        .route("/crates/{name}", get(browse_crate))
        .route("/crates/{name}/doc", get(read_crate_doc));

    // Main app with static file serving
    let app = Router::new()
        .nest("/api", api_routes)
        .fallback_service(ServeDir::new(static_dir))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("Listening on http://localhost:{}", port);
    axum::serve(listener, app)
        .with_graceful_shutdown(viewer_api::shutdown_signal())
        .await?;

    Ok(())
}
