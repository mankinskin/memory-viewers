use super::*;
pub struct CreateDocParams {
    pub doc_type: DocType,
    pub name: String,
    pub title: String,
    pub summary: String,
    pub tags: Option<Vec<String>>,
    pub status: Option<PlanStatus>,
}

#[derive(Debug, Serialize)]
pub struct CreateDocResult {
    pub path: String,
    pub filename: String,
}

#[derive(Debug, Serialize)]
pub struct DocSummary {
    pub filename: String,
    pub title: String,
    pub date: String,
    pub summary: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<PlanStatus>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMetaParams {
    pub filename: String,
    pub tags: Option<Vec<String>>,
    pub summary: Option<String>,
    pub status: Option<PlanStatus>,
}

#[derive(Debug, Default, Serialize)]
pub struct ValidationReport {
    pub documents_checked: usize,
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str("# Documentation Validation Report\n\n");
        md.push_str(&format!(
            "**Documents Checked:** {}\n",
            self.documents_checked
        ));

        let errors: Vec<_> = self
            .issues
            .iter()
            .filter(|i| matches!(i.severity, IssueSeverity::Error))
            .collect();
        let warnings: Vec<_> = self
            .issues
            .iter()
            .filter(|i| matches!(i.severity, IssueSeverity::Warning))
            .collect();

        md.push_str(&format!("**Errors:** {}\n", errors.len()));
        md.push_str(&format!("**Warnings:** {}\n\n", warnings.len()));

        if self.issues.is_empty() {
            md.push_str("✅ **All documents pass validation!**\n");
        } else {
            if !errors.is_empty() {
                md.push_str("## ❌ Errors\n\n");
                md.push_str("| Category | File | Issue |\n");
                md.push_str("|----------|------|-------|\n");
                for issue in &errors {
                    md.push_str(&format!(
                        "| {} | {} | {} |\n",
                        issue.category, issue.file, issue.issue
                    ));
                }
                md.push_str("\n");
            }

            if !warnings.is_empty() {
                md.push_str("## ⚠️ Warnings\n\n");
                md.push_str("| Category | File | Issue |\n");
                md.push_str("|----------|------|-------|\n");
                for issue in &warnings {
                    md.push_str(&format!(
                        "| {} | {} | {} |\n",
                        issue.category, issue.file, issue.issue
                    ));
                }
            }
        }

        md
    }
}

#[derive(Debug, Serialize)]
pub struct ValidationIssue {
    pub file: String,
    pub category: String,
    pub issue: String,
    pub severity: IssueSeverity,
}

#[derive(Debug, Serialize)]
pub enum IssueSeverity {
    Error,
    Warning,
}

/// Result of reading a document
#[derive(Debug, Serialize)]
pub struct ReadDocResult {
    pub filename: String,
    pub doc_type: String,
    pub title: String,
    pub date: String,
    pub summary: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<PlanStatus>,
    /// The body content (only present for 'full' detail level, or outline for 'outline')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// Result of browsing documentation structure
#[derive(Debug, Serialize)]
pub struct BrowseResult {
    pub total_documents: usize,
    pub categories: Vec<CategorySummary>,
}

/// Summary of a document category
#[derive(Debug, Serialize)]
pub struct CategorySummary {
    pub category: String,
    pub doc_count: usize,
    pub items: Vec<TocItem>,
}

/// Table of contents item (minimal info)
#[derive(Debug, Serialize)]
pub struct TocItem {
    pub filename: String,
    pub date: String,
    pub summary: String,
}

/// Document flagged for review
#[derive(Debug, Serialize)]
pub struct ReviewCandidate {
    pub filename: String,
    pub doc_type: String,
    pub date: String,
    pub age_days: u64,
    pub summary: String,
    pub reason: String,
}

/// Result of content search
#[derive(Debug, Serialize)]
pub struct ContentSearchResult {
    pub query: String,
    pub total_matches: usize,
    pub files_searched: usize,
    pub matches: Vec<FileMatch>,
}

/// Matches within a single file
#[derive(Debug, Serialize)]
pub struct FileMatch {
    pub filename: String,
    pub doc_type: String,
    pub match_count: usize,
    pub excerpts: Vec<MatchExcerpt>,
}

/// A single match with context
#[derive(Debug, Serialize)]
pub struct MatchExcerpt {
    /// Line number where the match occurs (1-indexed)
    pub line_number: usize,
    /// The matching line
    pub line: String,
    /// Lines before the match
    pub context_before: Vec<String>,
    /// Lines after the match
    pub context_after: Vec<String>,
}

/// Result of add_frontmatter operation
#[derive(Debug, Serialize)]
pub struct AddFrontmatterResult {
    pub processed: usize,
    pub updated: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub changes: Vec<FrontmatterChange>,
}

/// A single frontmatter change
#[derive(Debug, Serialize)]
pub struct FrontmatterChange {
    pub filename: String,
    pub doc_type: String,
    pub inferred_tags: Vec<String>,
    pub inferred_summary: String,
}

impl AddFrontmatterResult {
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!(
            "# Add Frontmatter Results\n\n**Processed:** {} | **Updated:** {} | **Skipped:** {} | **Errors:** {}\n\n",
            self.processed, self.updated, self.skipped, self.errors.len()
        ));

        if !self.changes.is_empty() {
            md.push_str("## Changes\n\n");
            md.push_str("| File | Type | Tags |\n");
            md.push_str("|------|------|------|\n");
            for change in &self.changes {
                let tags = change.inferred_tags.join(", ");
                md.push_str(&format!(
                    "| {} | {} | {} |\n",
                    change.filename, change.doc_type, tags
                ));
            }
            md.push('\n');
        }

        if !self.errors.is_empty() {
            md.push_str("## Errors\n\n");
            for err in &self.errors {
                md.push_str(&format!("- {}\n", err));
            }
        }

        md
    }
}

/// Health dashboard metrics
#[derive(Debug, Serialize)]
pub struct HealthDashboard {
    pub total_documents: usize,
    pub frontmatter_coverage: f64,
    pub index_sync_issues: usize,
    pub naming_issues: usize,
    pub old_documents: usize,
    pub categories: Vec<CategoryHealth>,
}

/// Health metrics for a single category
#[derive(Debug, Serialize)]
pub struct CategoryHealth {
    pub name: String,
    pub total: usize,
    pub with_frontmatter: usize,
    pub old: usize,
}

impl HealthDashboard {
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# Documentation Health Dashboard\n\n");

        // Overall metrics
        md.push_str("## Overview\n\n");
        md.push_str("| Metric | Value | Status |\n");
        md.push_str("|--------|-------|--------|\n");

        let fm_status = status_icon(
            self.frontmatter_coverage >= 90.0,
            self.frontmatter_coverage >= 50.0,
        );
        md.push_str(&format!(
            "| Frontmatter Coverage | {:.1}% | {} |\n",
            self.frontmatter_coverage, fm_status
        ));

        let idx_status = status_icon(
            self.index_sync_issues == 0,
            self.index_sync_issues <= 5,
        );
        md.push_str(&format!(
            "| INDEX Sync Issues | {} | {} |\n",
            self.index_sync_issues, idx_status
        ));

        let name_status =
            status_icon(self.naming_issues == 0, self.naming_issues <= 3);
        md.push_str(&format!(
            "| Naming Issues | {} | {} |\n",
            self.naming_issues, name_status
        ));

        md.push_str(&format!(
            "| Total Documents | {} | ℹ️ |\n",
            self.total_documents
        ));
        md.push_str(&format!(
            "| Old Documents (>30d) | {} | ℹ️ |\n",
            self.old_documents
        ));
        md.push('\n');

        // Category breakdown
        if !self.categories.is_empty() {
            md.push_str("## By Category\n\n");
            md.push_str("| Category | Total | Frontmatter | Old |\n");
            md.push_str("|----------|-------|-------------|-----|\n");
            for cat in &self.categories {
                let fm_pct = if cat.total > 0 {
                    (cat.with_frontmatter as f64 / cat.total as f64) * 100.0
                } else {
                    100.0
                };
                md.push_str(&format!(
                    "| {} | {} | {} ({:.0}%) | {} |\n",
                    cat.name, cat.total, cat.with_frontmatter, fm_pct, cat.old
                ));
            }
            md.push('\n');
        }

        // Recommendations
        md.push_str("## Recommendations\n\n");
        if self.frontmatter_coverage < 100.0 {
            let missing = self.total_documents
                - (self.total_documents as f64 * self.frontmatter_coverage
                    / 100.0) as usize;
            md.push_str(&format!(
                "- 🔧 Run `add_frontmatter` to add frontmatter to {} documents\n",
                missing
            ));
        }
        if self.index_sync_issues > 0 {
            md.push_str("- 🔧 Run `regenerate_index` for categories with INDEX sync issues\n");
        }
        if self.naming_issues > 0 {
            md.push_str("- 📝 Rename files with invalid naming conventions to YYYYMMDD_NAME.md format\n");
        }
        if self.old_documents > 10 {
            md.push_str("- 📋 Review old documents with `get_docs_needing_review` for potential updates\n");
        }

        md
    }
}

// =============================================================================
// Markdown Formatting
// =============================================================================

impl BrowseResult {
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!(
            "# Documentation Overview\n\n**Total Documents:** {}\n\n",
            self.total_documents
        ));

        for cat in &self.categories {
            md.push_str(&format!(
                "## {} ({} docs)\n\n",
                capitalize(&cat.category),
                cat.doc_count
            ));

            if cat.items.is_empty() {
                md.push_str("*No documents*\n\n");
            } else {
                md.push_str("| Date | File | Summary |\n");
                md.push_str("|------|------|---------|\n");
                for item in &cat.items {
                    md.push_str(&format!(
                        "| {} | {} | {} |\n",
                        &item.date,
                        &item.filename,
                        truncate(&item.summary, 50)
                    ));
                }
                md.push('\n');
            }
        }

        md
    }
}

impl ReadDocResult {
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# {}\n\n", self.title));
        md.push_str(&format!(
            "**File:** `{}`  \n**Type:** {}  \n**Date:** {}  \n",
            self.filename, self.doc_type, self.date,
        ));

        if !self.tags.is_empty() {
            let tags: Vec<String> =
                self.tags.iter().map(|t| format!("`#{}`", t)).collect();
            md.push_str(&format!("**Tags:** {}  \n", tags.join(" ")));
        }

        if let Some(status) = &self.status {
            md.push_str(&format!("**Status:** {}  \n", status.emoji()));
        }

        md.push_str(&format!("\n**Summary:** {}\n", self.summary));

        if let Some(body) = &self.body {
            md.push_str("\n---\n\n");
            md.push_str(body);
        }

        md
    }
}

impl ContentSearchResult {
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# Search Results: \"{}\"\n\n", self.query));
        md.push_str(&format!(
            "**Matches:** {} in {} files searched\n\n",
            self.total_matches, self.files_searched
        ));

        for file_match in &self.matches {
            md.push_str(&format!(
                "## {} ({})\n\n",
                file_match.filename, file_match.doc_type
            ));
            md.push_str(&format!("*{} match(es)*\n\n", file_match.match_count));

            for excerpt in &file_match.excerpts {
                md.push_str(&format!("**Line {}:**\n", excerpt.line_number));
                md.push_str("```\n");

                for line in &excerpt.context_before {
                    md.push_str(&format!("  {}\n", line));
                }
                md.push_str(&format!("> {}\n", excerpt.line));
                for line in &excerpt.context_after {
                    md.push_str(&format!("  {}\n", line));
                }

                md.push_str("```\n\n");
            }
        }

        md
    }
}
