//! Document schema definitions for structured agent documentation.

use serde::{
    Deserialize,
    Serialize,
};

/// Document category/type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DocType {
    Guide,
    Plan,
    Implemented,
    BugReport,
    Analysis,
}

impl DocType {
    pub fn directory(&self) -> &'static str {
        match self {
            DocType::Guide => "guides",
            DocType::Plan => "plans",
            DocType::Implemented => "implemented",
            DocType::BugReport => "bug-reports",
            DocType::Analysis => "analysis",
        }
    }

    pub fn from_directory(dir: &str) -> Option<Self> {
        match dir {
            "guides" => Some(DocType::Guide),
            "plans" => Some(DocType::Plan),
            "implemented" => Some(DocType::Implemented),
            "bug-reports" => Some(DocType::BugReport),
            "analysis" => Some(DocType::Analysis),
            _ => None,
        }
    }

    pub fn file_prefix(&self) -> &'static str {
        match self {
            DocType::Guide => "",
            DocType::Plan => "PLAN_",
            DocType::Implemented => "",
            DocType::BugReport => "BUG_",
            DocType::Analysis => "",
        }
    }
}

/// Status for plans (only applicable to DocType::Plan).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanStatus {
    /// 📋 Design/planning phase
    Design,
    /// 🚧 In progress
    InProgress,
    /// ✅ Completed (should move to implemented/)
    Completed,
    /// ⚠️ Blocked
    Blocked,
    /// ❌ Superseded/abandoned
    Superseded,
}

impl PlanStatus {
    pub fn emoji(&self) -> &'static str {
        match self {
            PlanStatus::Design => "📋",
            PlanStatus::InProgress => "🚧",
            PlanStatus::Completed => "✅",
            PlanStatus::Blocked => "⚠️",
            PlanStatus::Superseded => "❌",
        }
    }
}

impl std::fmt::Display for PlanStatus {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            PlanStatus::Design => write!(f, "design"),
            PlanStatus::InProgress => write!(f, "in-progress"),
            PlanStatus::Completed => write!(f, "completed"),
            PlanStatus::Blocked => write!(f, "blocked"),
            PlanStatus::Superseded => write!(f, "superseded"),
        }
    }
}

/// Metadata extracted from or written to a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocMetadata {
    pub doc_type: DocType,
    pub date: String,     // YYYYMMDD format
    pub title: String,    // Human-readable title
    pub filename: String, // Full filename with date prefix
    pub tags: Vec<String>,
    pub summary: String, // One-line summary for INDEX
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<PlanStatus>, // Only for plans
}

/// INDEX entry (simplified table row).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub date: String,
    pub filename: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<PlanStatus>,
}

impl From<&DocMetadata> for IndexEntry {
    fn from(meta: &DocMetadata) -> Self {
        IndexEntry {
            date: meta.date.clone(),
            filename: meta.filename.clone(),
            summary: meta.summary.clone(),
            status: meta.status,
        }
    }
}

// =============================================================================
// Crate Documentation Schema
// =============================================================================

/// Reference to a module in a crate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleRef {
    pub name: String,
    pub description: String,
    pub path: String,
}

/// Reference to a submodule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmoduleRef {
    pub name: String,
    pub path: String,
    pub description: String,
}

/// A file entry in a module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub description: String,
}

/// A type entry with module attribution (for browse_crate output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeWithModule {
    pub name: String,
    pub description: Option<String>,
    pub module_path: String,
    pub item_type: String, // "type", "trait", "macro"
}

impl TypeWithModule {
    pub fn from_entry(
        entry: &TypeEntry,
        module_path: &str,
        item_type: &str,
    ) -> Self {
        Self {
            name: entry.name.clone(),
            description: entry.description.clone(),
            module_path: module_path.to_string(),
            item_type: item_type.to_string(),
        }
    }
}

/// A type entry (for key_types)
///
/// Supports YAML formats:
/// - Plain string: `"TypeName"`
/// - Map format: `TypeName: Description text`
#[derive(Debug, Clone, Serialize)]
pub struct TypeEntry {
    pub name: String,
    pub description: Option<String>,
}

impl TypeEntry {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

impl<'de> serde::Deserialize<'de> for TypeEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{
            self,
            MapAccess,
            Visitor,
        };
        use std::fmt;

        struct TypeEntryVisitor;

        impl<'de> Visitor<'de> for TypeEntryVisitor {
            type Value = TypeEntry;

            fn expecting(
                &self,
                formatter: &mut fmt::Formatter,
            ) -> fmt::Result {
                formatter.write_str("a string or a map with one key-value pair")
            }

            fn visit_str<E>(
                self,
                v: &str,
            ) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(TypeEntry {
                    name: v.to_string(),
                    description: None,
                })
            }

            fn visit_map<A>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                if let Some((key, value)) =
                    map.next_entry::<String, String>()?
                {
                    Ok(TypeEntry {
                        name: key,
                        description: Some(value),
                    })
                } else {
                    Err(de::Error::custom(
                        "expected a map with one key-value pair",
                    ))
                }
            }
        }

        deserializer.deserialize_any(TypeEntryVisitor)
    }
}

/// Exported items from a crate
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportedItems {
    #[serde(default)]
    pub types: Vec<TypeEntry>,
    #[serde(default)]
    pub traits: Vec<TypeEntry>,
    #[serde(default)]
    pub macros: Vec<TypeEntry>,
}

/// Crate-level metadata (from index.yaml at crate root)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateMetadata {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    pub description: String,
    #[serde(default)]
    pub modules: Vec<ModuleRef>,
    #[serde(default)]
    pub exported_items: Option<ExportedItems>,
    #[serde(default)]
    pub dependencies: Vec<TypeEntry>,
    #[serde(default)]
    pub features: Vec<TypeEntry>,
    /// Source files that this documentation tracks (for stale detection)
    /// Paths are relative to crate root (e.g., "src/lib.rs")
    #[serde(default)]
    pub source_files: Vec<String>,
    /// ISO 8601 timestamp of last documentation sync
    #[serde(default)]
    pub last_synced: Option<String>,
}

/// Module-level metadata (from index.yaml in module directories)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleMetadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub submodules: Vec<SubmoduleRef>,
    #[serde(default)]
    pub files: Vec<FileEntry>,
    #[serde(default)]
    pub key_types: Vec<TypeEntry>,
    /// Source files that this module documentation tracks (for stale detection)
    /// Paths are relative to crate root (e.g., "src/graph/mod.rs")
    #[serde(default)]
    pub source_files: Vec<String>,
    /// ISO 8601 timestamp of last documentation sync
    #[serde(default)]
    pub last_synced: Option<String>,
}

/// Summary info for a crate (used in list_crates output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateSummary {
    pub name: String,
    pub version: Option<String>,
    pub description: String,
    pub module_count: usize,
    pub has_readme: bool,
    /// Full path to the crate root directory
    pub crate_path: String,
    /// Full path to the agents/docs directory
    pub docs_path: String,
}

/// A node in the module tree for browsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleTreeNode {
    pub name: String,
    pub path: String,
    pub description: String,
    #[serde(default)]
    pub children: Vec<ModuleTreeNode>,
    #[serde(default)]
    pub files: Vec<FileEntry>,
    #[serde(default)]
    pub key_types: Vec<TypeEntry>,
    pub has_readme: bool,
    /// All types/traits/macros with module attribution (only populated at root level)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub all_types: Vec<TypeWithModule>,
}

/// Search result for crate documentation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateSearchResult {
    pub crate_name: String,
    pub module_path: String,
    pub match_type: String, // "type", "trait", "macro", "module", "file", "content"
    pub name: String,
    pub description: Option<String>,
    pub context: Option<String>,
}

/// Validation issue for crate documentation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateValidationIssue {
    pub crate_name: String,
    pub module_path: Option<String>,
    pub issue: String,
    pub severity: String, // "error", "warning"
}

/// Validation report for crate documentation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CrateValidationReport {
    pub crates_checked: usize,
    pub modules_checked: usize,
    pub issues: Vec<CrateValidationIssue>,
}

impl CrateValidationReport {
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# Crate Documentation Validation Report\n\n\
             **Crates checked:** {}\n\
             **Modules checked:** {}\n\
             **Issues found:** {}\n\n",
            self.crates_checked,
            self.modules_checked,
            self.issues.len()
        ));

        if self.issues.is_empty() {
            out.push_str("✅ No issues found!\n");
        } else {
            out.push_str("## Issues\n\n");
            out.push_str("| Severity | Crate | Module | Issue |\n");
            out.push_str("|----------|-------|--------|-------|\n");
            for issue in &self.issues {
                let module = issue.module_path.as_deref().unwrap_or("-");
                let severity_icon = if issue.severity == "error" {
                    "❌"
                } else {
                    "⚠️"
                };
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    severity_icon, issue.crate_name, module, issue.issue
                ));
            }
        }

        out
    }
}

// =============================================================================
// Stale Detection Schema
// =============================================================================

/// Status of staleness for a documentation item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StalenessLevel {
    /// Documentation is up-to-date with source files
    Fresh,
    /// Source files modified recently (within configurable threshold)
    Stale,
    /// Source files significantly modified since last sync
    VeryStale,
    /// No source files configured - cannot determine staleness
    Unknown,
}

impl StalenessLevel {
    pub fn emoji(&self) -> &'static str {
        match self {
            StalenessLevel::Fresh => "✅",
            StalenessLevel::Stale => "⚠️",
            StalenessLevel::VeryStale => "🔴",
            StalenessLevel::Unknown => "❓",
        }
    }
}

/// Information about a file's modification status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileModificationInfo {
    /// Path to the file (relative to crate root)
    pub path: String,
    /// Last modification timestamp from git (ISO 8601)
    pub last_modified: Option<String>,
    /// Short commit hash of last modification
    pub last_commit: Option<String>,
    /// Commit message summary
    pub commit_message: Option<String>,
    /// Whether the file exists
    pub exists: bool,
}

/// Stale status for a single documentation item (crate or module)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleDocItem {
    /// Crate name
    pub crate_name: String,
    /// Module path (empty for crate-level docs)
    pub module_path: Option<String>,
    /// Overall staleness level
    pub staleness: StalenessLevel,
    /// When the documentation was last synced (from index.yaml)
    pub doc_last_synced: Option<String>,
    /// Most recent modification time among tracked source files
    pub source_last_modified: Option<String>,
    /// Days since documentation was synced
    pub days_since_sync: Option<i64>,
    /// Days since source was last modified
    pub days_since_source_change: Option<i64>,
    /// Information about each tracked source file
    pub source_files: Vec<FileModificationInfo>,
    /// Files modified after documentation was last synced
    pub modified_files: Vec<String>,
}

/// Report of stale documentation across crates
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StaleDocsReport {
    /// Total crates checked
    pub crates_checked: usize,
    /// Total modules checked
    pub modules_checked: usize,
    /// Items that are stale or very stale
    pub stale_items: Vec<StaleDocItem>,
    /// Items that are fresh
    pub fresh_items: Vec<StaleDocItem>,
    /// Items with unknown status (no source files configured)
    pub unknown_items: Vec<StaleDocItem>,
    /// Summary statistics
    pub summary: StaleSummary,
}

/// Summary statistics for stale docs report
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StaleSummary {
    pub total_items: usize,
    pub fresh_count: usize,
    pub stale_count: usize,
    pub very_stale_count: usize,
    pub unknown_count: usize,
}

impl StaleDocsReport {
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Documentation Staleness Report\n\n");
        out.push_str(&format!(
            "**Crates checked:** {}\n\
             **Modules checked:** {}\n\n",
            self.crates_checked, self.modules_checked
        ));

        // Summary
        out.push_str("## Summary\n\n");
        out.push_str(&format!(
            "| Status | Count |\n\
             |--------|-------|\n\
             | ✅ Fresh | {} |\n\
             | ⚠️ Stale | {} |\n\
             | 🔴 Very Stale | {} |\n\
             | ❓ Unknown | {} |\n\
             | **Total** | {} |\n\n",
            self.summary.fresh_count,
            self.summary.stale_count,
            self.summary.very_stale_count,
            self.summary.unknown_count,
            self.summary.total_items
        ));

        // Stale items (prioritize these)
        if !self.stale_items.is_empty() {
            out.push_str("## Stale Documentation\n\n");
            out.push_str(
                "| Status | Location | Days Since Sync | Modified Files |\n",
            );
            out.push_str(
                "|--------|----------|-----------------|----------------|\n",
            );
            for item in &self.stale_items {
                let location = match &item.module_path {
                    Some(mp) => format!(
                        "{}::{}",
                        item.crate_name,
                        mp.replace('/', "::")
                    ),
                    None => item.crate_name.clone(),
                };
                let days = item
                    .days_since_sync
                    .map(|d| d.to_string())
                    .unwrap_or("-".to_string());
                let files = if item.modified_files.is_empty() {
                    "-".to_string()
                } else {
                    item.modified_files.join(", ")
                };
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    item.staleness.emoji(),
                    location,
                    days,
                    files
                ));
            }
            out.push('\n');
        }

        // Unknown items
        if !self.unknown_items.is_empty() {
            out.push_str("## No Source Files Configured\n\n");
            out.push_str("These documentation items don't have `source_files` configured and cannot be checked for staleness:\n\n");
            for item in &self.unknown_items {
                let location = match &item.module_path {
                    Some(mp) => format!(
                        "{}::{}",
                        item.crate_name,
                        mp.replace('/', "::")
                    ),
                    None => item.crate_name.clone(),
                };
                out.push_str(&format!("- {}\n", location));
            }
            out.push('\n');
        }

        out
    }
}

// =============================================================================
// Sync Documentation Schema
// =============================================================================

/// A suggested change for documentation sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSuggestion {
    /// Type of change: "add", "update", "remove"
    pub change_type: String,
    /// What kind of item: "type", "trait", "macro", "module", "function"
    pub item_kind: String,
    /// Name of the item
    pub item_name: String,
    /// Description extracted from source (if available)
    pub description: Option<String>,
    /// Source file where item was found
    pub source_file: String,
    /// Line number in source file
    pub line_number: Option<usize>,
}

/// Result of analyzing source files for sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAnalysisResult {
    /// Crate name
    pub crate_name: String,
    /// Module path (empty for crate-level)
    pub module_path: Option<String>,
    /// Suggested changes
    pub suggestions: Vec<SyncSuggestion>,
    /// Public types found in source (omitted in summary mode)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub public_types: Vec<String>,
    /// Public traits found in source (omitted in summary mode)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub public_traits: Vec<String>,
    /// Public macros found in source (omitted in summary mode)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub public_macros: Vec<String>,
    /// Source files analyzed
    pub files_analyzed: Vec<String>,
    /// Errors encountered during analysis
    pub errors: Vec<String>,
    /// Summary counts (for quick overview)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<SyncSummary>,
}

/// Summary counts for sync analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSummary {
    pub types_found: usize,
    pub traits_found: usize,
    pub macros_found: usize,
    pub to_add: usize,
    pub to_remove: usize,
}

impl SyncAnalysisResult {
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let location = match &self.module_path {
            Some(mp) =>
                format!("{}::{}", self.crate_name, mp.replace('/', "::")),
            None => self.crate_name.clone(),
        };
        out.push_str(&format!("# Sync Analysis: {}\n\n", location));

        out.push_str(&format!(
            "**Files analyzed:** {}\n\n",
            self.files_analyzed.len()
        ));

        self.push_errors_section(&mut out);
        self.push_summary_section(&mut out);
        self.push_public_items_section(&mut out);
        self.push_suggestions_section(&mut out);

        out
    }

    fn push_errors_section(
        &self,
        out: &mut String,
    ) {
        if self.errors.is_empty() {
            return;
        }
        out.push_str("## Errors\n\n");
        for err in &self.errors {
            out.push_str(&format!("- {}\n", err));
        }
        out.push('\n');
    }

    fn push_summary_section(
        &self,
        out: &mut String,
    ) {
        let Some(summary) = &self.summary else {
            return;
        };
        out.push_str("## Summary\n\n");
        out.push_str(&format!(
            "- **Types found:** {}\n- **Traits found:** {}\n- **Macros found:** {}\n- **To add:** {}\n- **To remove:** {}\n\n",
            summary.types_found, summary.traits_found, summary.macros_found,
            summary.to_add, summary.to_remove
        ));
    }

    fn push_public_items_section(
        &self,
        out: &mut String,
    ) {
        if self.public_types.is_empty()
            && self.public_traits.is_empty()
            && self.public_macros.is_empty()
        {
            return;
        }
        out.push_str("## Public Items Found\n\n");
        push_public_item_group(out, "Types", &self.public_types);
        push_public_item_group(out, "Traits", &self.public_traits);
        push_public_item_group(out, "Macros", &self.public_macros);
    }

    fn push_suggestions_section(
        &self,
        out: &mut String,
    ) {
        if self.suggestions.is_empty() {
            out.push_str(
                "✅ No suggested changes - documentation appears up to date.\n",
            );
            return;
        }
        out.push_str("## Suggested Changes\n\n");
        out.push_str("| Action | Kind | Name | Source |\n");
        out.push_str("|--------|------|------|--------|\n");
        for sug in &self.suggestions {
            let action_icon = match sug.change_type.as_str() {
                "add" => "➕",
                "update" => "🔄",
                "remove" => "➖",
                _ => "❓",
            };
            let source = match sug.line_number {
                Some(ln) => format!("{}:{}", sug.source_file, ln),
                None => sug.source_file.clone(),
            };
            out.push_str(&format!(
                "| {} {} | {} | `{}` | {} |\n",
                action_icon,
                sug.change_type,
                sug.item_kind,
                sug.item_name,
                source
            ));
        }
    }
}

/// Append a `**Label (n):** a, b, c` line for a non-empty public-item group.
fn push_public_item_group(
    out: &mut String,
    label: &str,
    items: &[String],
) {
    if items.is_empty() {
        return;
    }
    out.push_str(&format!(
        "**{} ({}):** {}\n\n",
        label,
        items.len(),
        items.join(", ")
    ));
}
