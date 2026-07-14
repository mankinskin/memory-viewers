//! MCP server module for documentation management.
//!
//! This module contains the DocsServer struct and all MCP tool handlers
//! for managing documentation files.
//!
//! Consolidated into 6 CRUD-based tools:
//! - list: List/browse/read resources
//! - search: Search documentation content
//! - validate: Maintenance & validation operations
//! - create: Create new documentation
//! - update: Update existing documentation
//! - delete: Delete documentation

mod inputs;

pub use inputs::*;

use rmcp::{
    handler::server::{
        tool::ToolRouter,
        wrapper::Parameters,
    },
    model::*,
    tool,
    tool_handler,
    tool_router,
    ErrorData as McpError,
    ServerHandler,
};
use std::{
    path::PathBuf,
    sync::Arc,
};

use crate::{
    helpers::{
        format_module_tree,
        parse_detail_level,
        parse_doc_type,
        parse_status,
    },
    tools::{
        self,
        agents::CreateDocParams,
        CrateDocsManager,
        DocsManager,
    },
};

/// Convert a markdown-producing tool result into a `CallToolResult`,
/// rendering errors as an error content payload.
fn markdown_or_error(result: tools::ToolResult<String>) -> CallToolResult {
    match result {
        Ok(md) => CallToolResult::success(vec![Content::text(md)]),
        Err(e) =>
            CallToolResult::error(vec![Content::text(format!("Error: {}", e))]),
    }
}

/// MCP Server for documentation management.
#[derive(Clone)]
pub struct DocsServer {
    manager: Arc<DocsManager>,
    crate_manager: Arc<CrateDocsManager>,
    tool_router: ToolRouter<Self>,
}

impl DocsServer {
    pub fn new(
        agents_dir: PathBuf,
        crates_dirs: Vec<PathBuf>,
    ) -> Self {
        Self {
            manager: Arc::new(DocsManager::new(agents_dir)),
            crate_manager: Arc::new(CrateDocsManager::new(crates_dirs)),
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl DocsServer {
    // ============================================================
    // LIST - List, browse, and read documentation
    // ============================================================

    /// List, browse, or read documentation
    #[tool(description = "List, browse, or read documentation.

Targets:
- agent_docs: List agent documentation (guides, plans, bug-reports, etc.)
- crate_docs: Browse crate API documentation modules
- crates: List all available crates

For agent_docs:
- Set doc_type to filter by type (guide, plan, etc.)
- Set filename to read a specific document
- Set tag/status to filter results

For crate_docs:
- Set crate_name to browse a specific crate's modules
- Set module_path to read specific module docs

For crates:
- No additional parameters needed")]
    async fn list(
        &self,
        Parameters(input): Parameters<ListInput>,
    ) -> Result<CallToolResult, McpError> {
        match input.target {
            ListTarget::AgentDocs => self.list_agent_docs(input).await,
            ListTarget::CrateDocs => self.list_crate_docs(input).await,
            ListTarget::Crates => self.list_crates().await,
        }
    }

    async fn list_agent_docs(
        &self,
        input: ListInput,
    ) -> Result<CallToolResult, McpError> {
        // Read specific document
        if let Some(filename) = input.filename {
            let detail = parse_detail_level(&input.detail);
            match self.manager.read_document(&filename, detail) {
                Ok(content) =>
                    Ok(CallToolResult::success(vec![Content::text(
                        content.to_markdown(),
                    )])),
                Err(e) => Ok(CallToolResult::error(vec![Content::text(
                    format!("Error: {}", e),
                )])),
            }
        }
        // Browse with optional filters (when no doc_type filter is set, shows all categories)
        else if input.doc_type.is_none() {
            let filter = tools::ListFilter {
                tag: input.tag,
                status: input.status.as_ref().and_then(|s| parse_status(s)),
            };
            match self.manager.browse_docs(None, &filter) {
                Ok(result) => Ok(CallToolResult::success(vec![Content::text(
                    result.to_markdown(),
                )])),
                Err(e) => Ok(CallToolResult::error(vec![Content::text(
                    format!("Error: {}", e),
                )])),
            }
        }
        // List filtered documents of a specific type
        else {
            let doc_type = parse_doc_type(input.doc_type.as_ref().unwrap())
                .ok_or_else(|| {
                    McpError::invalid_params(
                        format!(
                            "Invalid doc_type: {}",
                            input.doc_type.as_ref().unwrap()
                        ),
                        None,
                    )
                })?;
            let filter = tools::ListFilter {
                tag: input.tag,
                status: input.status.as_ref().and_then(|s| parse_status(s)),
            };

            match self.manager.list_documents_filtered(doc_type, &filter) {
                Ok(docs) => {
                    let mut md = format!(
                        "# {} Documents\n\n",
                        input.doc_type.as_ref().unwrap()
                    );

                    if docs.is_empty() {
                        md.push_str("No documents found.\n");
                    } else {
                        md.push_str(&format!(
                            "**{} documents found**\n\n",
                            docs.len()
                        ));
                        md.push_str("| Filename | Summary | Tags |\n");
                        md.push_str("|----------|---------|------|\n");
                        for doc in &docs {
                            let tags = doc.tags.join(", ");
                            md.push_str(&format!(
                                "| {} | {} | {} |\n",
                                doc.filename, doc.summary, tags
                            ));
                        }
                    }

                    Ok(CallToolResult::success(vec![Content::text(md)]))
                },
                Err(e) => Ok(CallToolResult::error(vec![Content::text(
                    format!("Error: {}", e),
                )])),
            }
        }
    }

    async fn list_crate_docs(
        &self,
        input: ListInput,
    ) -> Result<CallToolResult, McpError> {
        let crate_name = input.crate_name.ok_or_else(|| {
            McpError::invalid_params(
                "crate_name required for crate_docs target",
                None,
            )
        })?;

        // Read specific module or crate root
        if input.module_path.is_some() || input.detail == "full" {
            match self.crate_manager.read_crate_doc(
                &crate_name,
                input.module_path.as_deref(),
                input.include_readme,
            ) {
                Ok(doc) => Ok(CallToolResult::success(vec![Content::text(
                    doc.to_markdown(),
                )])),
                Err(e) => Ok(CallToolResult::error(vec![Content::text(
                    format!("Error: {}", e),
                )])),
            }
        }
        // Browse crate module tree
        else {
            match self.crate_manager.browse_crate(&crate_name) {
                Ok(tree) => {
                    let md = format_module_tree(&tree, 0);
                    Ok(CallToolResult::success(vec![Content::text(md)]))
                },
                Err(e) => Ok(CallToolResult::error(vec![Content::text(
                    format!("Error: {}", e),
                )])),
            }
        }
    }

    async fn list_crates(&self) -> Result<CallToolResult, McpError> {
        match self.crate_manager.discover_crates_with_diagnostics() {
            Ok(result) => {
                let mut md = String::from("# Available Crates\n\n");

                // Show directory info
                md.push_str("**Crates Directories:**\n");
                for (dir, exists) in
                    result.crates_dirs.iter().zip(result.dirs_exist.iter())
                {
                    let status = if *exists { "✅" } else { "❌" };
                    md.push_str(&format!("- `{}` {}\n", dir, status));
                }
                md.push('\n');

                if result.crates.is_empty() {
                    md.push_str("*No crates found with `agents/docs/` directories.*\n\n");
                } else {
                    md.push_str(&format!(
                        "**{} crates with documentation:**\n\n",
                        result.crates.len()
                    ));
                    md.push_str("| Crate | Version | Modules | README | Description |\n");
                    md.push_str("|-------|---------|---------|--------|-------------|\n");
                    for c in &result.crates {
                        let version = c.version.as_deref().unwrap_or("-");
                        let readme = if c.has_readme { "✅" } else { "❌" };
                        md.push_str(&format!(
                            "| {} | {} | {} | {} | {} |\n",
                            c.name,
                            version,
                            c.module_count,
                            readme,
                            c.description
                        ));
                    }
                }

                if !result.diagnostics.is_empty() {
                    md.push_str("\n## Diagnostics\n\n");
                    for msg in &result.diagnostics {
                        md.push_str(&format!("- {}\n", msg));
                    }
                }

                Ok(CallToolResult::success(vec![Content::text(md)]))
            },
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Error: {}",
                e
            ))])),
        }
    }

    // ============================================================
    // SEARCH - Search documentation content
    // ============================================================

    /// Search documentation
    #[tool(description = "Search documentation content.

Targets:
- agent_docs: Search agent documentation
- crate_docs: Search crate API documentation
- all: Search all documentation

Search options:
- query: Regex patterns (graph|path), quoted literals (\"hello\"), case-insensitive
- include_content: Search within file content (not just metadata)
- lines_before/after: Context lines for content matches")]
    async fn search(
        &self,
        Parameters(input): Parameters<SearchInput>,
    ) -> Result<CallToolResult, McpError> {
        match input.target {
            SearchTarget::AgentDocs => self.search_agent_docs(input).await,
            SearchTarget::CrateDocs => self.search_crate_docs(input).await,
            SearchTarget::All => self.search_all(input).await,
        }
    }

    async fn search_agent_docs(
        &self,
        input: SearchInput,
    ) -> Result<CallToolResult, McpError> {
        let doc_type = input.doc_type.as_ref().and_then(|s| parse_doc_type(s));
        let filter = tools::ListFilter {
            tag: input.tag.clone(),
            status: None,
        };

        if input.include_content {
            // Search content
            match self.manager.search_content(
                &input.query,
                doc_type,
                &filter,
                input.lines_before,
                input.lines_after,
            ) {
                Ok(results) =>
                    Ok(CallToolResult::success(vec![Content::text(
                        results.to_markdown(),
                    )])),
                Err(e) => Ok(CallToolResult::error(vec![Content::text(
                    format!("Error: {}", e),
                )])),
            }
        } else {
            // Search metadata
            match self.manager.search_docs(
                Some(&input.query),
                input.tag.as_deref(),
                false,
                doc_type,
            ) {
                Ok(results) => {
                    let mut md =
                        format!("# Search Results: \"{}\"\n\n", input.query);
                    md.push_str(&format!(
                        "**{} matches found**\n\n",
                        results.len()
                    ));

                    if results.is_empty() {
                        md.push_str("No matches found.\n");
                    } else {
                        md.push_str("| File | Summary | Tags |\n");
                        md.push_str("|------|---------|------|\n");
                        for r in &results {
                            let tags = r.tags.join(", ");
                            md.push_str(&format!(
                                "| {} | {} | {} |\n",
                                r.filename, r.summary, tags
                            ));
                        }
                    }

                    Ok(CallToolResult::success(vec![Content::text(md)]))
                },
                Err(e) => Ok(CallToolResult::error(vec![Content::text(
                    format!("Error: {}", e),
                )])),
            }
        }
    }

    async fn search_crate_docs(
        &self,
        input: SearchInput,
    ) -> Result<CallToolResult, McpError> {
        match self.crate_manager.search_crate_docs(
            &input.query,
            input.crate_filter.as_deref(),
            input.search_types,
            input.search_readme,
        ) {
            Ok(results) => {
                let mut md =
                    format!("# Search Results: \"{}\"\n\n", input.query);
                md.push_str(&format!(
                    "**{} matches found**\n\n",
                    results.len()
                ));

                if results.is_empty() {
                    md.push_str("No matches found.\n");
                } else {
                    md.push_str(
                        "| Crate | Module | Type | Name | Description |\n",
                    );
                    md.push_str(
                        "|-------|--------|------|------|-------------|\n",
                    );
                    for r in &results {
                        let module = if r.module_path.is_empty() {
                            "-"
                        } else {
                            &r.module_path
                        };
                        let desc = r
                            .description
                            .as_deref()
                            .or(r.context.as_deref())
                            .unwrap_or("-");
                        md.push_str(&format!(
                            "| {} | {} | {} | {} | {} |\n",
                            r.crate_name, module, r.match_type, r.name, desc
                        ));
                    }
                }

                Ok(CallToolResult::success(vec![Content::text(md)]))
            },
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Error: {}",
                e
            ))])),
        }
    }

    async fn search_all(
        &self,
        input: SearchInput,
    ) -> Result<CallToolResult, McpError> {
        let mut md = format!("# Search Results: \"{}\"\n\n", input.query);

        // Search agent docs
        md.push_str("## Agent Documentation\n\n");
        self.append_agent_doc_search(&input, &mut md);

        // Search crate docs
        md.push_str("\n\n## Crate Documentation\n\n");
        self.append_crate_doc_search(&input, &mut md);

        Ok(CallToolResult::success(vec![Content::text(md)]))
    }

    /// Append agent-documentation search results to `md`.
    fn append_agent_doc_search(
        &self,
        input: &SearchInput,
        md: &mut String,
    ) {
        let doc_type = input.doc_type.as_ref().and_then(|s| parse_doc_type(s));
        let filter = tools::ListFilter {
            tag: input.tag.clone(),
            status: None,
        };

        if input.include_content {
            match self.manager.search_content(
                &input.query,
                doc_type,
                &filter,
                input.lines_before,
                input.lines_after,
            ) {
                Ok(results) => {
                    // Skip the header (already have one)
                    let content = results.to_markdown();
                    let lines: Vec<&str> = content.lines().skip(2).collect();
                    md.push_str(&lines.join("\n"));
                },
                Err(e) =>
                    md.push_str(&format!("Error searching agent docs: {}\n", e)),
            }
            return;
        }

        match self.manager.search_docs(
            Some(&input.query),
            input.tag.as_deref(),
            false,
            doc_type,
        ) {
            Ok(results) =>
                if results.is_empty() {
                    md.push_str("No matches found.\n");
                } else {
                    md.push_str(&format!(
                        "**{} matches found**\n\n",
                        results.len()
                    ));
                    md.push_str("| File | Summary | Tags |\n");
                    md.push_str("|------|---------|------|\n");
                    for r in &results {
                        let tags = r.tags.join(", ");
                        md.push_str(&format!(
                            "| {} | {} | {} |\n",
                            r.filename, r.summary, tags
                        ));
                    }
                },
            Err(e) =>
                md.push_str(&format!("Error searching agent docs: {}\n", e)),
        }
    }

    /// Append crate-documentation search results to `md`.
    fn append_crate_doc_search(
        &self,
        input: &SearchInput,
        md: &mut String,
    ) {
        match self.crate_manager.search_crate_docs(
            &input.query,
            input.crate_filter.as_deref(),
            input.search_types,
            input.search_readme,
        ) {
            Ok(results) =>
                if results.is_empty() {
                    md.push_str("No matches found.\n");
                } else {
                    md.push_str(&format!(
                        "**{} matches found**\n\n",
                        results.len()
                    ));
                    md.push_str("| Crate | Module | Type | Name |\n");
                    md.push_str("|-------|--------|------|------|\n");
                    for r in &results {
                        let module = if r.module_path.is_empty() {
                            "-"
                        } else {
                            &r.module_path
                        };
                        md.push_str(&format!(
                            "| {} | {} | {} | {} |\n",
                            r.crate_name, module, r.match_type, r.name
                        ));
                    }
                },
            Err(e) => md.push_str(&format!("Error: {}\n", e)),
        }
    }

    // ============================================================
    // VALIDATE - Maintenance and validation operations
    // ============================================================

    /// Validate and maintain documentation
    #[tool(description = "Validate and maintain documentation.

Targets:
- agent_docs: Validate agent documentation
- crate_docs: Validate crate API documentation
- all: Validate all

Actions:
- validate: Run validation checks
- health: Show health dashboard with statistics
- check_stale: Check for stale documentation (crate_docs)
- sync: Sync documentation with source files (crate_docs)
- regenerate_index: Regenerate INDEX.md file (agent_docs)
- add_frontmatter: Add frontmatter to documents missing it (agent_docs)
- review_needed: List documents needing review (agent_docs)")]
    async fn validate(
        &self,
        Parameters(input): Parameters<ValidateInput>,
    ) -> Result<CallToolResult, McpError> {
        match input.target {
            ValidateTarget::AgentDocs => self.validate_agent_docs(&input),
            ValidateTarget::CrateDocs =>
                self.validate_crate_docs_target(&input),
            ValidateTarget::All => self.validate_all_target(&input),
        }
    }

    /// Handle `validate` actions scoped to agent documentation.
    fn validate_agent_docs(
        &self,
        input: &ValidateInput,
    ) -> Result<CallToolResult, McpError> {
        match input.action {
            ValidateAction::Validate => Ok(markdown_or_error(
                self.manager.validate().map(|r| r.to_markdown()),
            )),
            ValidateAction::Health => Ok(markdown_or_error(
                self.manager
                    .health_dashboard(input.detailed)
                    .map(|r| r.to_markdown()),
            )),
            ValidateAction::RegenerateIndex => self.regenerate_index_action(input),
            ValidateAction::AddFrontmatter => self.add_frontmatter_action(input),
            ValidateAction::ReviewNeeded => {
                Ok(match self.manager.get_docs_needing_review(input.max_age_days) {
                    Ok(docs) => {
                        let json =
                            serde_json::to_string_pretty(&docs).unwrap_or_default();
                        CallToolResult::success(vec![Content::text(json)])
                    },
                    Err(e) => CallToolResult::error(vec![Content::text(format!(
                        "Error: {}",
                        e
                    ))]),
                })
            },
            ValidateAction::CheckStale | ValidateAction::Sync => {
                Ok(CallToolResult::error(vec![Content::text(
                    "check_stale and sync actions are only valid for crate_docs target",
                )]))
            },
        }
    }

    /// Regenerate an agent-docs INDEX for the requested doc type.
    fn regenerate_index_action(
        &self,
        input: &ValidateInput,
    ) -> Result<CallToolResult, McpError> {
        let doc_type = input
            .doc_type
            .as_ref()
            .and_then(|s| parse_doc_type(s))
            .ok_or_else(|| {
                McpError::invalid_params(
                    "doc_type required for regenerate_index",
                    None,
                )
            })?;
        Ok(match self.manager.update_index(doc_type) {
            Ok(path) => CallToolResult::success(vec![Content::text(format!(
                "Regenerated INDEX at: {}",
                path
            ))]),
            Err(e) => CallToolResult::error(vec![Content::text(format!(
                "Error: {}",
                e
            ))]),
        })
    }

    /// Add frontmatter to agent docs missing it for the requested doc type.
    fn add_frontmatter_action(
        &self,
        input: &ValidateInput,
    ) -> Result<CallToolResult, McpError> {
        let doc_type = match input.doc_type.as_deref() {
            None | Some("all") => None,
            Some(dt_str) => match parse_doc_type(dt_str) {
                Some(dt) => Some(dt),
                None => return Ok(CallToolResult::error(vec![Content::text(
                    format!("Invalid doc_type: {}. Use guide, plan, implemented, bug-report, analysis, or all", dt_str)
                )])),
            },
        };
        Ok(markdown_or_error(
            self.manager
                .add_frontmatter(doc_type, input.dry_run)
                .map(|r| r.to_markdown()),
        ))
    }

    /// Handle `validate` actions scoped to crate documentation.
    fn validate_crate_docs_target(
        &self,
        input: &ValidateInput,
    ) -> Result<CallToolResult, McpError> {
        match input.action {
            ValidateAction::Validate => Ok(markdown_or_error(
                self.crate_manager
                    .validate_crate_docs(input.crate_filter.as_deref())
                    .map(|r| r.to_markdown()),
            )),
            ValidateAction::CheckStale => Ok(markdown_or_error(
                self.crate_manager
                    .check_stale_docs(
                        input.crate_filter.as_deref(),
                        input.stale_threshold_days,
                        input.very_stale_threshold_days,
                    )
                    .map(|r| r.to_markdown()),
            )),
            ValidateAction::Sync => {
                let crate_name = input.crate_filter.as_ref().ok_or_else(|| {
                    McpError::invalid_params(
                        "crate_filter required for sync action",
                        None,
                    )
                })?;
                Ok(markdown_or_error(
                    self.crate_manager
                        .sync_crate_docs(
                            crate_name,
                            input.module_path.as_deref(),
                            input.update_timestamp,
                            input.summary_only,
                        )
                        .map(|r| r.to_markdown()),
                ))
            },
            ValidateAction::Health
            | ValidateAction::RegenerateIndex
            | ValidateAction::AddFrontmatter
            | ValidateAction::ReviewNeeded => {
                Ok(CallToolResult::error(vec![Content::text(
                    "health, regenerate_index, add_frontmatter, and review_needed actions are only valid for agent_docs target",
                )]))
            },
        }
    }

    /// Handle `validate` actions scoped to all documentation targets.
    fn validate_all_target(
        &self,
        input: &ValidateInput,
    ) -> Result<CallToolResult, McpError> {
        match input.action {
            ValidateAction::Validate => {
                let mut md = String::from("# Validation Report\n\n");

                md.push_str("## Agent Documentation\n\n");
                match self.manager.validate() {
                    Ok(report) => md.push_str(&report.to_markdown()),
                    Err(e) => md.push_str(&format!("Error: {}\n", e)),
                }

                md.push_str("\n\n## Crate Documentation\n\n");
                match self.crate_manager.validate_crate_docs(None) {
                    Ok(report) => md.push_str(&report.to_markdown()),
                    Err(e) => md.push_str(&format!("Error: {}\n", e)),
                }

                Ok(CallToolResult::success(vec![Content::text(md)]))
            },
            ValidateAction::Health => {
                let mut md = String::from("# Health Dashboard\n\n");

                md.push_str("## Agent Documentation\n\n");
                match self.manager.health_dashboard(input.detailed) {
                    Ok(dashboard) => md.push_str(&dashboard.to_markdown()),
                    Err(e) => md.push_str(&format!("Error: {}\n", e)),
                }

                // No crate docs health dashboard yet - could add later
                md.push_str("\n\n## Crate Documentation\n\n");
                match self.crate_manager.validate_crate_docs(None) {
                    Ok(report) => {
                        md.push_str(&format!(
                            "**{} crates checked, {} modules checked**\n",
                            report.crates_checked, report.modules_checked
                        ));
                        md.push_str(&format!(
                            "**Issues found:** {}\n",
                            report.issues.len()
                        ));
                    },
                    Err(e) => md.push_str(&format!("Error: {}\n", e)),
                }

                Ok(CallToolResult::success(vec![Content::text(md)]))
            },
            _ => Ok(CallToolResult::error(vec![Content::text(
                "Only validate and health actions are supported for 'all' target",
            )])),
        }
    }

    // ============================================================
    // CREATE - Create new documentation
    // ============================================================

    /// Create new documentation
    #[tool(description = "Create new documentation.

Targets:
- agent_doc: Create agent documentation file
  - Required: doc_type, name, title, summary
  - Optional: tags, status (for plans)
- crate_module: Create crate module documentation
  - Required: crate_name, module_path, name, description")]
    async fn create(
        &self,
        Parameters(input): Parameters<CreateInput>,
    ) -> Result<CallToolResult, McpError> {
        match input.target {
            CreateTarget::AgentDoc => {
                let doc_type_str =
                    input.doc_type.as_ref().ok_or_else(|| {
                        McpError::invalid_params(
                            "doc_type required for agent_doc",
                            None,
                        )
                    })?;
                let doc_type =
                    parse_doc_type(doc_type_str).ok_or_else(|| {
                        McpError::invalid_params(
                            format!("Invalid doc_type: {}", doc_type_str),
                            None,
                        )
                    })?;

                let name = input.name.ok_or_else(|| {
                    McpError::invalid_params(
                        "name required for agent_doc",
                        None,
                    )
                })?;
                let title = input.title.ok_or_else(|| {
                    McpError::invalid_params(
                        "title required for agent_doc",
                        None,
                    )
                })?;
                let summary = input.summary.ok_or_else(|| {
                    McpError::invalid_params(
                        "summary required for agent_doc",
                        None,
                    )
                })?;

                let status =
                    input.status.as_ref().and_then(|s| parse_status(s));

                let params = CreateDocParams {
                    doc_type,
                    name,
                    title,
                    summary,
                    tags: Some(input.tags),
                    status,
                };

                match self.manager.create_document(params) {
                    Ok(result) =>
                        Ok(CallToolResult::success(vec![Content::text(
                            format!(
                                "Created: {}\nPath: {}",
                                result.filename, result.path
                            ),
                        )])),
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(
                        format!("Error: {}", e),
                    )])),
                }
            },
            CreateTarget::CrateModule => {
                let crate_name = input.crate_name.ok_or_else(|| {
                    McpError::invalid_params(
                        "crate_name required for crate_module",
                        None,
                    )
                })?;
                let module_path = input.module_path.ok_or_else(|| {
                    McpError::invalid_params(
                        "module_path required for crate_module",
                        None,
                    )
                })?;
                let name = input.name.ok_or_else(|| {
                    McpError::invalid_params(
                        "name required for crate_module",
                        None,
                    )
                })?;
                let description = input.description.ok_or_else(|| {
                    McpError::invalid_params(
                        "description required for crate_module",
                        None,
                    )
                })?;

                match self.crate_manager.create_module_doc(
                    &crate_name,
                    &module_path,
                    &name,
                    &description,
                ) {
                    Ok(path) =>
                        Ok(CallToolResult::success(vec![Content::text(
                            format!(
                                "Created module documentation at: {}",
                                path
                            ),
                        )])),
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(
                        format!("Error: {}", e),
                    )])),
                }
            },
        }
    }

    // ============================================================
    // UPDATE - Update existing documentation
    // ============================================================

    /// Update existing documentation
    #[tool(description = "Update existing documentation.

Targets:
- agent_doc: Update agent documentation metadata
  - Required: filename
  - Optional: tags, summary, status
- crate_doc: Update crate documentation content
  - Required: crate_name
  - Optional: module_path, index_yaml, readme
- crate_index: Update crate index.yaml configuration
  - Required: crate_name
  - Optional: module_path, source_files, add_source_files, remove_source_files")]
    async fn update(
        &self,
        Parameters(input): Parameters<UpdateInput>,
    ) -> Result<CallToolResult, McpError> {
        match input.target {
            UpdateTarget::AgentDoc => {
                let filename = input.filename.ok_or_else(|| {
                    McpError::invalid_params(
                        "filename required for agent_doc",
                        None,
                    )
                })?;

                let params = tools::agents::UpdateMetaParams {
                    filename: filename.clone(),
                    tags: input.tags,
                    summary: input.summary,
                    status: input.status.as_ref().and_then(|s| parse_status(s)),
                };

                match self.manager.update_document_metadata(params) {
                    Ok(()) => Ok(CallToolResult::success(vec![Content::text(
                        format!("Updated metadata for: {}", filename),
                    )])),
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(
                        format!("Error: {}", e),
                    )])),
                }
            },
            UpdateTarget::CrateDoc => {
                let crate_name = input.crate_name.ok_or_else(|| {
                    McpError::invalid_params(
                        "crate_name required for crate_doc",
                        None,
                    )
                })?;

                match self.crate_manager.update_crate_doc(
                    &crate_name,
                    input.module_path.as_deref(),
                    input.index_yaml.as_deref(),
                    input.readme.as_deref(),
                ) {
                    Ok(()) => {
                        let location = match &input.module_path {
                            Some(p) => format!(
                                "{}::{}",
                                crate_name,
                                p.replace('/', "::")
                            ),
                            None => crate_name,
                        };
                        Ok(CallToolResult::success(vec![Content::text(
                            format!("Updated documentation for: {}", location),
                        )]))
                    },
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(
                        format!("Error: {}", e),
                    )])),
                }
            },
            UpdateTarget::CrateIndex => {
                let crate_name = input.crate_name.ok_or_else(|| {
                    McpError::invalid_params(
                        "crate_name required for crate_index",
                        None,
                    )
                })?;

                match self.crate_manager.update_crate_index(
                    &crate_name,
                    input.module_path.as_deref(),
                    input.source_files,
                    input.add_source_files,
                    input.remove_source_files,
                ) {
                    Ok(result) =>
                        Ok(CallToolResult::success(vec![Content::text(result)])),
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(
                        format!("Error: {}", e),
                    )])),
                }
            },
        }
    }

    // ============================================================
    // DELETE - Delete documentation
    // ============================================================

    /// Delete documentation
    #[tool(description = "Delete documentation.

Targets:
- agent_doc: Delete agent documentation file
  - Required: filename, confirm=true
- crate_module: Delete crate module documentation
  - Required: crate_name, module_path, confirm=true

Set confirm=true to actually delete. Without it, shows what would be deleted.")]
    async fn delete(
        &self,
        Parameters(input): Parameters<DeleteInput>,
    ) -> Result<CallToolResult, McpError> {
        match input.target {
            DeleteTarget::AgentDoc => {
                let filename = input.filename.ok_or_else(|| {
                    McpError::invalid_params(
                        "filename required for agent_doc",
                        None,
                    )
                })?;

                if !input.confirm {
                    return Ok(CallToolResult::success(vec![Content::text(format!(
                        "Would delete agent doc: {}\n\nSet confirm=true to actually delete.",
                        filename
                    ))]));
                }

                match self.manager.delete_document(&filename) {
                    Ok(path) =>
                        Ok(CallToolResult::success(vec![Content::text(
                            format!("Deleted: {}", path),
                        )])),
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(
                        format!("Error: {}", e),
                    )])),
                }
            },
            DeleteTarget::CrateModule => {
                let crate_name = input.crate_name.ok_or_else(|| {
                    McpError::invalid_params(
                        "crate_name required for crate_module",
                        None,
                    )
                })?;
                let module_path = input.module_path.ok_or_else(|| {
                    McpError::invalid_params(
                        "module_path required for crate_module",
                        None,
                    )
                })?;

                if !input.confirm {
                    return Ok(CallToolResult::success(vec![Content::text(format!(
                        "Would delete crate module: {}::{}\n\nSet confirm=true to actually delete.",
                        crate_name,
                        module_path.replace('/', "::")
                    ))]));
                }

                match self
                    .crate_manager
                    .delete_module_doc(&crate_name, &module_path)
                {
                    Ok(path) =>
                        Ok(CallToolResult::success(vec![Content::text(
                            format!(
                                "Deleted module documentation at: {}",
                                path
                            ),
                        )])),
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(
                        format!("Error: {}", e),
                    )])),
                }
            },
        }
    }
}

#[tool_handler]
impl ServerHandler for DocsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "MCP Docs Server for managing structured agent documentation and crate API docs.\n\n\
                 Consolidated CRUD-based API:\n\
                 - list: List/browse/read (targets: agent_docs, crate_docs, crates)\n\
                 - search: Search content (targets: agent_docs, crate_docs, all)\n\
                 - validate: Maintenance operations (targets: agent_docs, crate_docs, all)\n\
                 - create: Create docs (targets: agent_doc, crate_module)\n\
                 - update: Update docs (targets: agent_doc, crate_doc, crate_index)\n\
                 - delete: Delete docs (targets: agent_doc, crate_module)"
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
