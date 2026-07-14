//! Agent documentation management (guides, plans, bug reports, etc.)
//!
//! This module handles the `agents/` directory documentation including:
//! - guides/ - How-to guides and troubleshooting
//! - plans/ - Task plans before execution
//! - implemented/ - Completed feature documentation
//! - bug-reports/ - Known issues and analyses
//! - analysis/ - Algorithm analysis and comparisons

use super::{
    compile_search_regex,
    regex_matches,
    DetailLevel,
    ListFilter,
    ToolError,
    ToolResult,
};
use crate::{
    helpers::unix_path,
    parser::{
        extract_metadata,
        parse_filename,
        parse_frontmatter,
        parse_title,
    },
    schema::{
        DocMetadata,
        DocType,
        IndexEntry,
        PlanStatus,
    },
    templates::{
        generate_document,
        generate_index,
    },
};
use serde::{
    Deserialize,
    Serialize,
};
use std::{
    fs,
    path::{
        Path,
        PathBuf,
    },
};

/// Documentation manager handling all operations.
pub struct DocsManager {
    agents_dir: PathBuf,
}

impl DocsManager {
    pub fn new(agents_dir: PathBuf) -> Self {
        Self { agents_dir }
    }

    /// Create a new document from parameters.
    pub fn create_document(
        &self,
        params: CreateDocParams,
    ) -> ToolResult<CreateDocResult> {
        let date = chrono::Local::now().format("%Y%m%d").to_string();
        let prefix = params.doc_type.file_prefix();
        let name_upper = params
            .name
            .to_uppercase()
            .replace(' ', "_")
            .replace('-', "_");
        let filename = format!("{}{}_{}.md", date, prefix, name_upper);

        let dir = self.agents_dir.join(params.doc_type.directory());
        let path = dir.join(&filename);

        if path.exists() {
            return Err(ToolError::AlreadyExists(filename));
        }

        let meta = DocMetadata {
            doc_type: params.doc_type,
            date,
            title: params.title,
            filename: filename.clone(),
            tags: params.tags.unwrap_or_default(),
            summary: params.summary,
            status: params.status,
        };

        let content = generate_document(&meta);
        fs::create_dir_all(&dir)?;
        fs::write(&path, &content)?;

        // Update INDEX
        self.update_index(params.doc_type)?;

        Ok(CreateDocResult {
            path: unix_path(&path),
            filename,
        })
    }

    /// List all documents of a given type.
    pub fn list_documents(
        &self,
        doc_type: DocType,
    ) -> ToolResult<Vec<DocSummary>> {
        let dir = self.agents_dir.join(doc_type.directory());
        let mut docs = Vec::new();

        if !dir.exists() {
            return Ok(docs);
        }

        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map_or(false, |e| e == "md") {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string();

                if filename == "INDEX.md" {
                    continue;
                }

                let content = fs::read_to_string(&path)?;
                if let Some(meta) = extract_metadata(&path, &content) {
                    docs.push(DocSummary {
                        filename: meta.filename,
                        title: meta.title,
                        date: meta.date,
                        summary: meta.summary,
                        tags: meta.tags,
                        status: meta.status,
                    });
                }
            }
        }

        // Sort by date descending
        docs.sort_by(|a, b| b.date.cmp(&a.date));

        Ok(docs)
    }

    /// Update the INDEX.md for a document type.
    pub fn update_index(
        &self,
        doc_type: DocType,
    ) -> ToolResult<String> {
        let docs = self.list_documents(doc_type)?;
        let entries: Vec<IndexEntry> = docs
            .iter()
            .map(|d| IndexEntry {
                date: d.date.clone(),
                filename: d.filename.clone(),
                summary: d.summary.clone(),
                status: d.status,
            })
            .collect();

        let content = generate_index(doc_type, &entries);
        let path = self.agents_dir.join(doc_type.directory()).join("INDEX.md");
        fs::write(&path, &content)?;

        Ok(unix_path(&path))
    }

    /// Update metadata for an existing document.
    pub fn update_document_metadata(
        &self,
        params: UpdateMetaParams,
    ) -> ToolResult<()> {
        let path = self.find_document(&params.filename)?;
        let content = fs::read_to_string(&path)?;

        // Parse existing and merge updates
        let mut meta = extract_metadata(&path, &content).ok_or_else(|| {
            ToolError::InvalidInput("Cannot parse document".into())
        })?;

        if let Some(tags) = params.tags {
            meta.tags = tags;
        }
        if let Some(summary) = params.summary {
            meta.summary = summary;
        }
        if let Some(status) = params.status {
            meta.status = Some(status);
        }

        // Regenerate frontmatter only (preserve body)
        let body = extract_body(&content);
        let new_content = format!("{}\n{}", generate_frontmatter(&meta), body);
        fs::write(&path, new_content)?;

        // Update index
        self.update_index(meta.doc_type)?;

        Ok(())
    }

    /// Validate all documents and indexes.
    pub fn validate(&self) -> ToolResult<ValidationReport> {
        let mut report = ValidationReport::default();

        for doc_type in [
            DocType::Guide,
            DocType::Plan,
            DocType::Implemented,
            DocType::BugReport,
            DocType::Analysis,
        ] {
            let dir = self.agents_dir.join(doc_type.directory());
            let category = doc_type.directory().to_string();
            if !dir.exists() {
                report.issues.push(ValidationIssue {
                    file: category.clone(),
                    category: category.clone(),
                    issue: "Directory does not exist".to_string(),
                    severity: IssueSeverity::Warning,
                });
                continue;
            }

            // Check INDEX.md exists
            let index_path = dir.join("INDEX.md");
            if !index_path.exists() {
                report.issues.push(ValidationIssue {
                    file: "INDEX.md".to_string(),
                    category: category.clone(),
                    issue: "Missing INDEX.md file".to_string(),
                    severity: IssueSeverity::Error,
                });
            }

            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                self.validate_document_file(
                    &entry.path(),
                    &category,
                    doc_type == DocType::Plan,
                    &mut report,
                )?;
            }

            self.validate_index_file(&dir, &index_path, &category, &mut report);
        }

        Ok(report)
    }

    /// Validate a single markdown document (naming, frontmatter, title).
    fn validate_document_file(
        &self,
        path: &Path,
        category: &str,
        is_plan: bool,
        report: &mut ValidationReport,
    ) -> ToolResult<()> {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if filename == "INDEX.md" || !filename.ends_with(".md") {
            return Ok(());
        }

        // Check naming convention (YYYYMMDD_ prefix)
        if parse_filename(filename).is_none() {
            report.issues.push(ValidationIssue {
                file: filename.to_string(),
                category: category.to_string(),
                issue: "Invalid filename format - expected YYYYMMDD_NAME.md"
                    .to_string(),
                severity: IssueSeverity::Error,
            });
        }

        // Check file content
        let content = fs::read_to_string(path)?;

        // Check frontmatter exists
        if !content.starts_with("---") {
            report.issues.push(ValidationIssue {
                file: filename.to_string(),
                category: category.to_string(),
                issue: "Missing frontmatter (should start with ---)"
                    .to_string(),
                severity: IssueSeverity::Error,
            });
        } else if let Some(fm) = parse_frontmatter(&content) {
            // Check tags exist
            if fm.tags.is_empty() {
                report.issues.push(ValidationIssue {
                    file: filename.to_string(),
                    category: category.to_string(),
                    issue: "No tags defined in frontmatter".to_string(),
                    severity: IssueSeverity::Warning,
                });
            }

            // Plans should have status
            if is_plan && fm.status.is_none() {
                report.issues.push(ValidationIssue {
                    file: filename.to_string(),
                    category: category.to_string(),
                    issue: "Plan document missing status field".to_string(),
                    severity: IssueSeverity::Warning,
                });
            }
        } else {
            report.issues.push(ValidationIssue {
                file: filename.to_string(),
                category: category.to_string(),
                issue: "Could not parse frontmatter".to_string(),
                severity: IssueSeverity::Warning,
            });
        }

        // Check for H1 title
        if parse_title(&content).is_none() {
            report.issues.push(ValidationIssue {
                file: filename.to_string(),
                category: category.to_string(),
                issue: "Missing H1 title (# Title)".to_string(),
                severity: IssueSeverity::Warning,
            });
        }

        report.documents_checked += 1;
        Ok(())
    }

    /// Validate a category's INDEX.md against the documents on disk.
    fn validate_index_file(
        &self,
        dir: &Path,
        index_path: &Path,
        category: &str,
        report: &mut ValidationReport,
    ) {
        if !index_path.exists() {
            return;
        }
        let index_content = match fs::read_to_string(index_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        // Collect all document filenames in directory
        let mut doc_files: Vec<String> = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".md") && fname != "INDEX.md" {
                    doc_files.push(fname);
                }
            }
        }

        Self::validate_index_coverage(
            &index_content,
            &doc_files,
            category,
            report,
        );
        Self::validate_index_format(
            &index_content,
            &doc_files,
            category,
            report,
        );
    }

    /// Check that every document is listed and no stale entries remain.
    fn validate_index_coverage(
        index_content: &str,
        doc_files: &[String],
        category: &str,
        report: &mut ValidationReport,
    ) {
        // Check each document is mentioned in INDEX
        for doc_file in doc_files {
            if !index_content.contains(doc_file) {
                report.issues.push(ValidationIssue {
                    file: "INDEX.md".to_string(),
                    category: category.to_string(),
                    issue: format!(
                        "Document '{}' not listed in INDEX",
                        doc_file
                    ),
                    severity: IssueSeverity::Warning,
                });
            }
        }

        // Check for stale entries in INDEX (files mentioned but don't exist)
        let filename_pattern =
            regex::Regex::new(r"\d{8}_[A-Za-z0-9_-]+\.md").unwrap();
        for caps in filename_pattern.find_iter(index_content) {
            let mentioned_file = caps.as_str();
            if !doc_files.contains(&mentioned_file.to_string()) {
                report.issues.push(ValidationIssue {
                    file: "INDEX.md".to_string(),
                    category: category.to_string(),
                    issue: format!(
                        "Stale entry '{}' - file does not exist",
                        mentioned_file
                    ),
                    severity: IssueSeverity::Error,
                });
            }
        }
    }

    /// Check INDEX formatting conventions (title, table, verbosity, length).
    fn validate_index_format(
        index_content: &str,
        doc_files: &[String],
        category: &str,
        report: &mut ValidationReport,
    ) {
        // Check INDEX has H1 title
        if parse_title(index_content).is_none() {
            report.issues.push(ValidationIssue {
                file: "INDEX.md".to_string(),
                category: category.to_string(),
                issue: "INDEX.md missing H1 title".to_string(),
                severity: IssueSeverity::Warning,
            });
        }

        // Check INDEX uses minimal table format
        let has_doc_table = index_content.contains("| Date | File | Summary |")
            || index_content.contains("|------|------|");

        if !has_doc_table {
            report.issues.push(ValidationIssue {
                file: "INDEX.md".to_string(),
                category: category.to_string(),
                issue: "INDEX.md should use minimal table format: | Date | File | Summary |".to_string(),
                severity: IssueSeverity::Warning,
            });
        }

        // Check for verbose formatting patterns that should be avoided
        let verbose_patterns = [
            "**What it provides:**",
            "**Key locations:**",
            "**Solves:**",
            "**Benefits:**",
            "**Technique:**",
            "**Tags:** `#", // Tags should be in document, not INDEX
        ];

        for pattern in verbose_patterns {
            if index_content.contains(pattern) {
                report.issues.push(ValidationIssue {
                    file: "INDEX.md".to_string(),
                    category: category.to_string(),
                    issue: format!("INDEX.md too verbose - remove '{}' sections (use table format)", pattern),
                    severity: IssueSeverity::Warning,
                });
            }
        }

        // Check for excessive line count (INDEX should be concise)
        let line_count = index_content.lines().count();
        let expected_max = 20 + (doc_files.len() * 2); // Header + 2 lines per doc max
        if line_count > expected_max {
            report.issues.push(ValidationIssue {
                file: "INDEX.md".to_string(),
                category: category.to_string(),
                issue: format!("INDEX.md too long ({} lines, expected <{}). Use minimal table format.", line_count, expected_max),
                severity: IssueSeverity::Warning,
            });
        }
    }

    fn find_document(
        &self,
        filename: &str,
    ) -> ToolResult<PathBuf> {
        for doc_type in [
            DocType::Guide,
            DocType::Plan,
            DocType::Implemented,
            DocType::BugReport,
            DocType::Analysis,
        ] {
            let path =
                self.agents_dir.join(doc_type.directory()).join(filename);
            if path.exists() {
                return Ok(path);
            }
        }
        Err(ToolError::NotFound(filename.to_string()))
    }

    /// Delete a document and update the index.
    pub fn delete_document(
        &self,
        filename: &str,
    ) -> ToolResult<String> {
        let path = self.find_document(filename)?;

        // Get doc type to update index
        let content = fs::read_to_string(&path)?;
        let meta = extract_metadata(&path, &content).ok_or_else(|| {
            ToolError::InvalidInput("Cannot parse document".into())
        })?;

        // Delete the file
        fs::remove_file(&path)?;

        // Update index
        self.update_index(meta.doc_type)?;

        Ok(unix_path(&path))
    }

    /// Read the full content of a document.
    pub fn read_document(
        &self,
        filename: &str,
        detail: DetailLevel,
    ) -> ToolResult<ReadDocResult> {
        let path = self.find_document(filename)?;
        let content = fs::read_to_string(&path)?;

        let meta = extract_metadata(&path, &content).ok_or_else(|| {
            ToolError::InvalidInput("Cannot parse document".into())
        })?;

        let body = match detail {
            DetailLevel::Outline => extract_outline(&content),
            DetailLevel::Summary => None,
            DetailLevel::Full => Some(extract_body(&content)),
        };

        Ok(ReadDocResult {
            filename: meta.filename,
            doc_type: meta.doc_type.directory().to_string(),
            title: meta.title,
            date: meta.date,
            summary: meta.summary,
            tags: meta.tags,
            status: meta.status,
            body,
        })
    }

    /// List documents with optional filters.
    pub fn list_documents_filtered(
        &self,
        doc_type: DocType,
        filter: &ListFilter,
    ) -> ToolResult<Vec<DocSummary>> {
        let docs = self.list_documents(doc_type)?;

        let filtered = docs
            .into_iter()
            .filter(|doc| {
                // Filter by tag
                if let Some(tag) = &filter.tag {
                    let tag_lower =
                        tag.to_lowercase().trim_start_matches('#').to_string();
                    if !doc.tags.iter().any(|t| t.to_lowercase() == tag_lower) {
                        return false;
                    }
                }
                // Filter by status
                if let Some(status) = &filter.status {
                    if doc.status.as_ref() != Some(status) {
                        return false;
                    }
                }
                true
            })
            .collect();

        Ok(filtered)
    }

    /// Browse documentation structure (TOC view).
    pub fn browse_docs(
        &self,
        doc_type: Option<DocType>,
        filter: &ListFilter,
    ) -> ToolResult<BrowseResult> {
        let doc_types = match doc_type {
            Some(dt) => vec![dt],
            None => vec![
                DocType::Guide,
                DocType::Plan,
                DocType::Implemented,
                DocType::BugReport,
                DocType::Analysis,
            ],
        };

        let mut categories = Vec::new();
        let mut total_docs = 0;

        for dt in doc_types {
            let docs = self.list_documents_filtered(dt, filter)?;
            let count = docs.len();
            total_docs += count;

            let items: Vec<TocItem> = docs
                .into_iter()
                .map(|d| TocItem {
                    filename: d.filename,
                    date: d.date,
                    summary: d.summary,
                })
                .collect();

            categories.push(CategorySummary {
                category: dt.directory().to_string(),
                doc_count: count,
                items,
            });
        }

        Ok(BrowseResult {
            total_documents: total_docs,
            categories,
        })
    }

    /// Get documents that may need review (old documents).
    pub fn get_docs_needing_review(
        &self,
        max_age_days: u32,
    ) -> ToolResult<Vec<ReviewCandidate>> {
        let mut candidates = Vec::new();
        let today = chrono::Local::now().date_naive();

        for doc_type in [
            DocType::Guide,
            DocType::Plan,
            DocType::Implemented,
            DocType::BugReport,
            DocType::Analysis,
        ] {
            let docs = self.list_documents(doc_type)?;

            for doc in docs {
                // Parse date from YYYYMMDD format
                let doc_date =
                    chrono::NaiveDate::parse_from_str(&doc.date, "%Y%m%d")
                        .unwrap_or(today);
                let age_days = (today - doc_date).num_days().max(0) as u64;

                // Check age
                if age_days > max_age_days as u64 {
                    candidates.push(ReviewCandidate {
                        filename: doc.filename,
                        doc_type: doc_type.directory().to_string(),
                        date: doc.date,
                        age_days,
                        summary: doc.summary,
                        reason: format!("Old ({} days)", age_days),
                    });
                }
            }
        }

        // Sort by age descending (oldest first)
        candidates.sort_by(|a, b| b.age_days.cmp(&a.age_days));

        Ok(candidates)
    }

    /// Search document content using regex pattern (case-insensitive).
    pub fn search_content(
        &self,
        query: &str,
        doc_type: Option<DocType>,
        filter: &ListFilter,
        lines_before: usize,
        lines_after: usize,
    ) -> ToolResult<ContentSearchResult> {
        let doc_types = match doc_type {
            Some(dt) => vec![dt],
            None => vec![
                DocType::Guide,
                DocType::Plan,
                DocType::Implemented,
                DocType::BugReport,
                DocType::Analysis,
            ],
        };

        let regex = compile_search_regex(query)?;
        let mut matches = Vec::new();
        let mut files_searched = 0;
        let mut total_matches = 0;

        for dt in doc_types {
            let docs = self.list_documents_filtered(dt, filter)?;

            for doc in docs {
                files_searched += 1;
                let path =
                    self.agents_dir.join(dt.directory()).join(&doc.filename);
                let content = fs::read_to_string(&path)?;
                let lines: Vec<&str> = content.lines().collect();

                let mut excerpts = Vec::new();

                for (idx, line) in lines.iter().enumerate() {
                    // Check if regex matches the line
                    if regex_matches(line, &regex) {
                        total_matches += 1;

                        // Gather context before
                        let start = idx.saturating_sub(lines_before);
                        let context_before: Vec<String> = lines[start..idx]
                            .iter()
                            .map(|s| s.to_string())
                            .collect();

                        // Gather context after
                        let end = (idx + 1 + lines_after).min(lines.len());
                        let context_after: Vec<String> = lines[idx + 1..end]
                            .iter()
                            .map(|s| s.to_string())
                            .collect();

                        excerpts.push(MatchExcerpt {
                            line_number: idx + 1,
                            line: line.to_string(),
                            context_before,
                            context_after,
                        });
                    }
                }

                if !excerpts.is_empty() {
                    matches.push(FileMatch {
                        filename: doc.filename,
                        doc_type: dt.directory().to_string(),
                        match_count: excerpts.len(),
                        excerpts,
                    });
                }
            }
        }

        Ok(ContentSearchResult {
            query: query.to_string(),
            total_matches,
            files_searched,
            matches,
        })
    }

    /// Enhanced search: search by regex query and/or tag, optionally searching content.
    /// Query is a case-insensitive regex pattern.
    pub fn search_docs(
        &self,
        query: Option<&str>,
        tag: Option<&str>,
        search_content: bool,
        doc_type: Option<DocType>,
    ) -> ToolResult<Vec<DocSummary>> {
        let doc_types = match doc_type {
            Some(dt) => vec![dt],
            None => vec![
                DocType::Guide,
                DocType::Plan,
                DocType::Implemented,
                DocType::BugReport,
                DocType::Analysis,
            ],
        };

        let regex = query.map(compile_search_regex).transpose()?.flatten();
        let tag_lower =
            tag.map(|t| t.to_lowercase().trim_start_matches('#').to_string());

        let mut results = Vec::new();

        for dt in doc_types {
            let docs = self.list_documents(dt)?;

            for doc in docs {
                // Check tag if provided
                let tag_ok = tag_lower.as_ref().map_or(true, |tag_l| {
                    doc.tags.iter().any(|t| t.to_lowercase() == *tag_l)
                });

                // Check regex if provided
                let query_ok = if regex.is_none() {
                    true
                } else {
                    // Combine title/summary/tags for searching
                    let searchable = format!(
                        "{} {} {}",
                        doc.title,
                        doc.summary,
                        doc.tags.join(" ")
                    );

                    if regex_matches(&searchable, &regex) {
                        true
                    } else if search_content {
                        let path = self
                            .agents_dir
                            .join(dt.directory())
                            .join(&doc.filename);
                        if let Ok(content) = fs::read_to_string(&path) {
                            regex_matches(&content, &regex)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };

                if tag_ok && query_ok {
                    results.push(doc);
                }
            }
        }

        results.sort_by(|a, b| b.date.cmp(&a.date));
        Ok(results)
    }

    /// Add frontmatter to documents that are missing it
    pub fn add_frontmatter(
        &self,
        doc_type: Option<DocType>,
        dry_run: bool,
    ) -> ToolResult<AddFrontmatterResult> {
        let doc_types = match doc_type {
            Some(dt) => vec![dt],
            None => vec![
                DocType::Guide,
                DocType::Plan,
                DocType::Implemented,
                DocType::BugReport,
                DocType::Analysis,
            ],
        };

        let mut result = AddFrontmatterResult {
            processed: 0,
            updated: 0,
            skipped: 0,
            errors: Vec::new(),
            changes: Vec::new(),
        };

        for dt in doc_types {
            let dir = self.agents_dir.join(dt.directory());
            if !dir.exists() {
                continue;
            }

            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                self.process_frontmatter_file(
                    &entry.path(),
                    dt,
                    dry_run,
                    &mut result,
                )?;
            }
        }

        Ok(result)
    }

    /// Add inferred frontmatter to a single document if it is missing.
    fn process_frontmatter_file(
        &self,
        path: &Path,
        dt: DocType,
        dry_run: bool,
        result: &mut AddFrontmatterResult,
    ) -> ToolResult<()> {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        // Skip non-md files and INDEX.md
        if !filename.ends_with(".md") || filename == "INDEX.md" {
            return Ok(());
        }

        result.processed += 1;

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                result.errors.push(format!("{}: {}", filename, e));
                return Ok(());
            },
        };

        // Check if frontmatter exists
        if content.trim_start().starts_with("---")
            && parse_frontmatter(&content).is_some()
        {
            result.skipped += 1;
            return Ok(());
        }

        // Needs frontmatter - infer metadata
        let (date, _name) = parse_filename(filename).unwrap_or_else(|| {
            (
                "00000000".to_string(),
                filename.trim_end_matches(".md").to_string(),
            )
        });

        let title =
            parse_title(&content).unwrap_or_else(|| filename.to_string());

        // Try to extract summary from first paragraph
        let summary = extract_summary(&content).unwrap_or_default();

        // Infer tags from filename and content
        let tags = infer_tags(filename, &content, dt);

        let meta = DocMetadata {
            doc_type: dt,
            date: date.clone(),
            filename: filename.to_string(),
            tags: tags.clone(),
            summary: summary.clone(),
            status: if dt == DocType::Plan {
                Some(PlanStatus::Design)
            } else {
                None
            },
            title: title.clone(),
        };

        let frontmatter = generate_frontmatter(&meta);
        let new_content =
            format!("{}\n\n{}", frontmatter, content.trim_start());

        result.changes.push(FrontmatterChange {
            filename: filename.to_string(),
            doc_type: dt.directory().to_string(),
            inferred_tags: tags,
            inferred_summary: summary,
        });

        if !dry_run {
            if let Err(e) = fs::write(path, new_content) {
                result.errors.push(format!("{}: {}", filename, e));
                return Ok(());
            }
        }

        result.updated += 1;
        Ok(())
    }

    /// Get a health dashboard summarizing documentation status
    pub fn health_dashboard(
        &self,
        detailed: bool,
    ) -> ToolResult<HealthDashboard> {
        let validation = self.validate()?;

        let mut dashboard = HealthDashboard {
            total_documents: 0,
            frontmatter_coverage: 0.0,
            index_sync_issues: 0,
            naming_issues: 0,
            old_documents: 0,
            categories: Vec::new(),
        };

        let mut docs_with_frontmatter = 0;

        for doc_type in [
            DocType::Guide,
            DocType::Plan,
            DocType::Implemented,
            DocType::BugReport,
            DocType::Analysis,
        ] {
            let cat = self
                .tally_category_health(doc_type, &mut docs_with_frontmatter);
            dashboard.total_documents += cat.total;
            dashboard.old_documents += cat.old;

            if detailed {
                dashboard.categories.push(cat);
            }
        }

        // Calculate metrics from validation
        for issue in &validation.issues {
            match issue.issue.as_str() {
                s if s.contains("not listed in INDEX") =>
                    dashboard.index_sync_issues += 1,
                s if s.contains("Invalid filename") =>
                    dashboard.naming_issues += 1,
                _ => {},
            }
        }

        dashboard.frontmatter_coverage = if dashboard.total_documents > 0 {
            (docs_with_frontmatter as f64 / dashboard.total_documents as f64)
                * 100.0
        } else {
            100.0
        };

        Ok(dashboard)
    }

    /// Tally document counts, frontmatter coverage, and age for one category.
    fn tally_category_health(
        &self,
        doc_type: DocType,
        docs_with_frontmatter: &mut usize,
    ) -> CategoryHealth {
        let docs = self.list_documents(doc_type).unwrap_or_default();
        let count = docs.len();
        let dir = self.agents_dir.join(doc_type.directory());
        let today = chrono::Local::now().format("%Y%m%d").to_string();
        let mut fm_count = 0;
        let mut old_count = 0;

        for doc in &docs {
            let path = dir.join(&doc.filename);
            if let Ok(content) = fs::read_to_string(&path) {
                if content.trim_start().starts_with("---")
                    && parse_frontmatter(&content).is_some()
                {
                    fm_count += 1;
                    *docs_with_frontmatter += 1;
                }
            }

            if calculate_age_days(&doc.date, &today) > 30 {
                old_count += 1;
            }
        }

        CategoryHealth {
            name: doc_type.directory().to_string(),
            total: count,
            with_frontmatter: fm_count,
            old: old_count,
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Pick a status icon from an "ok" / "warning" threshold pair.
fn status_icon(
    ok: bool,
    warn: bool,
) -> &'static str {
    if ok {
        "✅"
    } else if warn {
        "⚠️"
    } else {
        "❌"
    }
}

/// Calculate age in days between two YYYYMMDD dates
fn calculate_age_days(
    date: &str,
    today: &str,
) -> u32 {
    use chrono::NaiveDate;
    let parse_date = |s: &str| NaiveDate::parse_from_str(s, "%Y%m%d").ok();

    if let (Some(d), Some(t)) = (parse_date(date), parse_date(today)) {
        (t - d).num_days().max(0) as u32
    } else {
        0
    }
}

/// Extract summary from document content (first non-header paragraph)
fn extract_summary(content: &str) -> Option<String> {
    let body = extract_body(content);
    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with('-')
            && !trimmed.starts_with('|')
            && !trimmed.starts_with("**")
        {
            // Truncate to reasonable length
            let summary = if trimmed.len() > 150 {
                format!("{}...", &trimmed[..147])
            } else {
                trimmed.to_string()
            };
            return Some(summary);
        }
    }
    None
}

/// Infer tags from filename and content
fn infer_tags(
    filename: &str,
    content: &str,
    doc_type: DocType,
) -> Vec<String> {
    let mut tags = Vec::new();

    // Add doc type as tag
    tags.push(doc_type.directory().trim_end_matches('s').to_string());

    // Check for crate mentions
    let crates = [
        "context-trace",
        "context-search",
        "context-insert",
        "context-read",
    ];
    for crate_name in crates {
        if filename
            .to_lowercase()
            .contains(&crate_name.replace('-', "_"))
            || content.to_lowercase().contains(crate_name)
        {
            tags.push(crate_name.to_string());
        }
    }

    // Check for common concepts
    let concepts = [
        ("algorithm", "algorithm"),
        ("bug", "debugging"),
        ("test", "testing"),
        ("refactor", "refactoring"),
        ("api", "api"),
        ("performance", "performance"),
    ];

    let lower_name = filename.to_lowercase();
    let lower_content = content.to_lowercase();
    for (pattern, tag) in concepts {
        if lower_name.contains(pattern) || lower_content.contains(pattern) {
            if !tags.contains(&tag.to_string()) {
                tags.push(tag.to_string());
            }
        }
    }

    tags
}

fn generate_frontmatter(meta: &DocMetadata) -> String {
    let tags_str = meta
        .tags
        .iter()
        .map(|t| format!("`#{}`", t))
        .collect::<Vec<_>>()
        .join(" ");

    let mut lines = vec![
        "---".to_string(),
        format!("tags: {}", tags_str),
        format!("summary: {}", meta.summary),
    ];

    if let Some(status) = &meta.status {
        lines.push(format!("status: {}", status.emoji()));
    }

    lines.push("---".to_string());
    lines.join("\n")
}

fn extract_body(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();

    if lines.first().map_or(true, |l| l.trim() != "---") {
        return content.to_string();
    }

    // Find end of frontmatter
    if let Some(end_idx) = lines.iter().skip(1).position(|l| l.trim() == "---")
    {
        lines[end_idx + 2..].join("\n")
    } else {
        content.to_string()
    }
}

/// Extract just the headers/outline from document content
fn extract_outline(content: &str) -> Option<String> {
    let body = extract_body(content);
    let headers: Vec<&str> =
        body.lines().filter(|line| line.starts_with('#')).collect();

    if headers.is_empty() {
        None
    } else {
        Some(headers.join("\n"))
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn truncate(
    s: &str,
    max_len: usize,
) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

// =============================================================================
// Parameter and Result Types
// =============================================================================

#[path = "agents_types.rs"]
mod agents_types;
pub use agents_types::*;
