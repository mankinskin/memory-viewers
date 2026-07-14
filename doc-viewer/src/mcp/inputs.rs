//! MCP input types for documentation server tools.
//!
//! Consolidated into 6 CRUD-based tools:
//! - list: List/browse/read resources
//! - search: Search documentation content
//! - validate: Maintenance & validation operations
//! - create: Create new documentation
//! - update: Update existing documentation
//! - delete: Delete documentation

use rmcp::schemars::{
    self,
    JsonSchema,
};
use serde::Deserialize;

// === LIST Tool ===

/// Target for list operations
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(inline)]
#[serde(rename_all = "snake_case")]
pub enum ListTarget {
    /// List agent documentation (guides, plans, bug-reports, etc.)
    AgentDocs,
    /// List crate API documentation modules
    CrateDocs,
    /// List all available crates
    Crates,
}

/// List, browse, or read documentation
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListInput {
    /// What to list: "agent_docs", "crate_docs", or "crates"
    pub target: ListTarget,

    // --- Agent docs filters ---
    /// Document type for agent_docs: "guide", "plan", "implemented", "bug-report", "analysis"
    #[serde(default)]
    pub doc_type: Option<String>,
    /// Filter by tag (agent_docs)
    #[serde(default)]
    pub tag: Option<String>,
    /// Filter by status for plans (agent_docs)
    #[serde(default)]
    pub status: Option<String>,

    // --- Crate docs filters ---
    /// Crate name for crate_docs (required for browsing modules)
    #[serde(default)]
    pub crate_name: Option<String>,
    /// Module path within crate (e.g., "graph/path")
    #[serde(default)]
    pub module_path: Option<String>,

    // --- Read specific item ---
    /// Filename to read (agent_docs) - returns full document content
    #[serde(default)]
    pub filename: Option<String>,
    /// Detail level when reading: "outline", "summary", "full" (default: "summary")
    #[serde(default = "default_detail_level")]
    pub detail: String,
    /// Include README.md content when reading crate docs (default: true)
    #[serde(default = "default_true")]
    pub include_readme: bool,
}

// === SEARCH Tool ===

/// Target for search operations
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(inline)]
#[serde(rename_all = "snake_case")]
pub enum SearchTarget {
    /// Search agent documentation
    AgentDocs,
    /// Search crate API documentation
    CrateDocs,
    /// Search all documentation
    All,
}

/// Search documentation content
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchInput {
    /// What to search: "agent_docs", "crate_docs", or "all"
    pub target: SearchTarget,
    /// Search query. Supports: regex patterns (graph|path, init.*), quoted literals ("hello world"), backslash escaping (\|). Case-insensitive.
    pub query: String,

    // --- Agent docs filters ---
    /// Document type filter for agent_docs
    #[serde(default)]
    pub doc_type: Option<String>,
    /// Tag filter
    #[serde(default)]
    pub tag: Option<String>,
    /// Include content search (vs just metadata) for agent_docs
    #[serde(default)]
    pub include_content: bool,
    /// Lines of context before matches (when searching content)
    #[serde(default = "default_context_lines")]
    pub lines_before: usize,
    /// Lines of context after matches (when searching content)
    #[serde(default = "default_context_lines")]
    pub lines_after: usize,

    // --- Crate docs filters ---
    /// Crate filter for crate_docs
    #[serde(default)]
    pub crate_filter: Option<String>,
    /// Search in type/trait/macro names (default: true)
    #[serde(default = "default_true")]
    pub search_types: bool,
    /// Search in README content (default: true)
    #[serde(default = "default_true")]
    pub search_readme: bool,
}

// === VALIDATE Tool ===

/// Target for validate operations
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(inline)]
#[serde(rename_all = "snake_case")]
pub enum ValidateTarget {
    /// Validate agent documentation
    AgentDocs,
    /// Validate crate API documentation
    CrateDocs,
    /// Validate all documentation
    All,
}

/// Action for validate operations
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(inline)]
#[serde(rename_all = "snake_case")]
pub enum ValidateAction {
    /// Run validation checks
    Validate,
    /// Show health dashboard with statistics
    Health,
    /// Check for stale documentation (crate_docs)
    CheckStale,
    /// Sync documentation with source files (crate_docs)
    Sync,
    /// Regenerate INDEX.md file (agent_docs)
    RegenerateIndex,
    /// Add frontmatter to documents missing it (agent_docs)
    AddFrontmatter,
    /// List documents needing review (agent_docs)
    ReviewNeeded,
}

/// Validate and maintain documentation
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ValidateInput {
    /// What to validate: "agent_docs", "crate_docs", or "all"
    pub target: ValidateTarget,
    /// Validation action to perform
    #[serde(default = "default_validate_action")]
    pub action: ValidateAction,

    // --- Agent docs options ---
    /// Document type for regenerate_index/add_frontmatter (or "all")
    #[serde(default)]
    pub doc_type: Option<String>,
    /// Preview changes without writing for add_frontmatter
    #[serde(default)]
    pub dry_run: bool,
    /// Include detailed breakdown for health dashboard
    #[serde(default = "default_true")]
    pub detailed: bool,
    /// Max age in days for review_needed (default: 30)
    #[serde(default = "default_max_age_days")]
    pub max_age_days: u32,

    // --- Crate docs options ---
    /// Crate filter
    #[serde(default)]
    pub crate_filter: Option<String>,
    /// Module path for sync
    #[serde(default)]
    pub module_path: Option<String>,
    /// Days after which docs are stale (default: 7)
    #[serde(default = "default_stale_threshold")]
    pub stale_threshold_days: i64,
    /// Days after which docs are very stale (default: 30)
    #[serde(default = "default_very_stale_threshold")]
    pub very_stale_threshold_days: i64,
    /// Update last_synced timestamp during sync
    #[serde(default)]
    pub update_timestamp: bool,
    /// Summary only for sync
    #[serde(default)]
    pub summary_only: bool,
}

// === CREATE Tool ===

/// Target for create operations
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(inline)]
#[serde(rename_all = "snake_case")]
pub enum CreateTarget {
    /// Create agent documentation
    AgentDoc,
    /// Create crate module documentation
    CrateModule,
}

/// Create new documentation
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateInput {
    /// What to create: "agent_doc" or "crate_module"
    pub target: CreateTarget,

    // --- Agent doc fields ---
    /// Document type for agent_doc: "guide", "plan", "implemented", "bug-report", "analysis"
    #[serde(default)]
    pub doc_type: Option<String>,
    /// Short name for the file (becomes UPPER_SNAKE_CASE in filename)
    #[serde(default)]
    pub name: Option<String>,
    /// Human-readable title for the document header
    #[serde(default)]
    pub title: Option<String>,
    /// One-line summary for the INDEX file
    #[serde(default)]
    pub summary: Option<String>,
    /// Tags for categorization (without #)
    #[serde(default)]
    pub tags: Vec<String>,
    /// Status for plans: "design", "in-progress", "completed", "blocked", "superseded"
    #[serde(default)]
    pub status: Option<String>,

    // --- Crate module fields ---
    /// Crate name for crate_module
    #[serde(default)]
    pub crate_name: Option<String>,
    /// Module path for crate_module (e.g., "new_module" or "parent/child")
    #[serde(default)]
    pub module_path: Option<String>,
    /// Description for crate_module
    #[serde(default)]
    pub description: Option<String>,
}

// === UPDATE Tool ===

/// Target for update operations
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(inline)]
#[serde(rename_all = "snake_case")]
pub enum UpdateTarget {
    /// Update agent documentation metadata
    AgentDoc,
    /// Update crate documentation (index.yaml and/or README.md)
    CrateDoc,
    /// Update crate index.yaml configuration
    CrateIndex,
}

/// Update existing documentation
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateInput {
    /// What to update: "agent_doc", "crate_doc", or "crate_index"
    pub target: UpdateTarget,

    // --- Agent doc fields ---
    /// Filename of the document to update (agent_doc)
    #[serde(default)]
    pub filename: Option<String>,
    /// New tags (replaces existing)
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// New summary
    #[serde(default)]
    pub summary: Option<String>,
    /// New status for plans
    #[serde(default)]
    pub status: Option<String>,

    // --- Crate doc/index fields ---
    /// Crate name
    #[serde(default)]
    pub crate_name: Option<String>,
    /// Module path
    #[serde(default)]
    pub module_path: Option<String>,
    /// New index.yaml content (crate_doc)
    #[serde(default)]
    pub index_yaml: Option<String>,
    /// New README.md content (crate_doc)
    #[serde(default)]
    pub readme: Option<String>,
    /// Set source_files (crate_index, replaces existing)
    #[serde(default)]
    pub source_files: Option<Vec<String>>,
    /// Add to source_files (crate_index)
    #[serde(default)]
    pub add_source_files: Option<Vec<String>>,
    /// Remove from source_files (crate_index)
    #[serde(default)]
    pub remove_source_files: Option<Vec<String>>,
}

// === DELETE Tool ===

/// Target for delete operations
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(inline)]
#[serde(rename_all = "snake_case")]
pub enum DeleteTarget {
    /// Delete agent documentation
    AgentDoc,
    /// Delete crate module documentation
    CrateModule,
}

/// Delete documentation
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteInput {
    /// What to delete: "agent_doc" or "crate_module"
    pub target: DeleteTarget,

    // --- Agent doc fields ---
    /// Filename of the document to delete (agent_doc)
    #[serde(default)]
    pub filename: Option<String>,

    // --- Crate module fields ---
    /// Crate name (crate_module)
    #[serde(default)]
    pub crate_name: Option<String>,
    /// Module path to delete (crate_module)
    #[serde(default)]
    pub module_path: Option<String>,

    /// Confirm deletion (required to actually delete)
    #[serde(default)]
    pub confirm: bool,
}

// === Helper functions ===

fn default_detail_level() -> String {
    "summary".to_string()
}

fn default_true() -> bool {
    true
}

fn default_context_lines() -> usize {
    2
}

fn default_validate_action() -> ValidateAction {
    ValidateAction::Validate
}

fn default_max_age_days() -> u32 {
    30
}

fn default_stale_threshold() -> i64 {
    7
}

fn default_very_stale_threshold() -> i64 {
    30
}
