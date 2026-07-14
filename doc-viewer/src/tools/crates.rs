//! Crate API documentation management (crates/*/agents/docs/)
//!
//! This module handles the structured API documentation for each crate,
//! including module trees, type entries, and sync with source files.

use super::{
    compile_search_regex,
    regex_matches,
    ToolError,
    ToolResult,
};
use crate::{
    git::{
        current_timestamp,
        days_since,
        get_files_info,
        get_files_modified_since,
        get_most_recent_modification,
        is_git_repository,
    },
    helpers::{
        normalize_path_str,
        to_vscode_file_uri,
        unix_path,
    },
    parser::{
        parse_crate_index,
        parse_module_index,
        read_markdown_file,
    },
    schema::{
        CrateMetadata,
        CrateSearchResult,
        CrateSummary,
        CrateValidationIssue,
        CrateValidationReport,
        ModuleMetadata,
        ModuleTreeNode,
        StaleDocItem,
        StaleDocsReport,
        StaleSummary,
        StalenessLevel,
        SyncAnalysisResult,
        SyncSuggestion,
        SyncSummary,
        TypeEntry,
        TypeWithModule,
    },
};
use regex::Regex;
use serde::Serialize;
use std::{
    fs,
    path::{
        Path,
        PathBuf,
    },
};

mod crates_sync;
/// Result of crate discovery with diagnostics
#[derive(Debug, Serialize)]
pub struct CrateDiscoveryResult {
    pub crates: Vec<CrateSummary>,
    pub diagnostics: Vec<String>,
    /// Directories that were scanned (display strings)
    pub crates_dirs: Vec<String>,
    /// Which directories exist
    pub dirs_exist: Vec<bool>,
}

/// Manager for crate API documentation in crates/*/agents/docs/
///
/// Supports multiple crate directories (e.g., `crates/` and `tools/`) to allow
/// documentation from different parts of the workspace.
pub struct CrateDocsManager {
    crates_dirs: Vec<PathBuf>,
}

impl CrateDocsManager {
    pub fn new(crates_dirs: Vec<PathBuf>) -> Self {
        Self { crates_dirs }
    }

    /// Get the configured crates directories
    pub fn crates_dirs(&self) -> &[PathBuf] {
        &self.crates_dirs
    }

    /// Resolve a crate name to its root path (public version).
    pub fn get_crate_path(
        &self,
        crate_name: &str,
    ) -> Option<PathBuf> {
        self.resolve_crate_path(crate_name)
    }

    /// Resolve a crate name to its root path.
    ///
    /// Searches all configured directories for a crate with matching name
    /// that has an agents/docs/index.yaml file.
    fn resolve_crate_path(
        &self,
        crate_name: &str,
    ) -> Option<PathBuf> {
        for crates_dir in &self.crates_dirs {
            let crate_path = crates_dir.join(crate_name);
            let index_path =
                crate_path.join("agents").join("docs").join("index.yaml");
            if index_path.exists() {
                return Some(crate_path);
            }
        }
        None
    }

    /// Discover all crates with agents/docs directories
    /// Returns both successful crates and diagnostic information about failures
    ///
    /// Scans all configured directories for subdirectories that have
    /// an `agents/docs/index.yaml` file.
    pub fn discover_crates_with_diagnostics(
        &self
    ) -> ToolResult<CrateDiscoveryResult> {
        let mut result = CrateDiscoveryResult {
            crates: Vec::new(),
            diagnostics: Vec::new(),
            crates_dirs: self
                .crates_dirs
                .iter()
                .map(|p| unix_path(p))
                .collect(),
            dirs_exist: self.crates_dirs.iter().map(|p| p.exists()).collect(),
        };

        for crates_dir in &self.crates_dirs {
            if !crates_dir.exists() {
                result.diagnostics.push(format!(
                    "Crates directory does not exist: {}",
                    unix_path(crates_dir)
                ));
                continue;
            }

            let entries = match fs::read_dir(crates_dir) {
                Ok(entries) => entries,
                Err(e) => {
                    result.diagnostics.push(format!(
                        "Failed to read {}: {}",
                        unix_path(crates_dir),
                        e
                    ));
                    continue;
                },
            };

            for entry in entries {
                self.collect_crate_entry(entry, crates_dir, &mut result);
            }
        }

        result.crates.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    /// Process a single directory entry while discovering crates, pushing a
    /// [`CrateSummary`] or a diagnostic message into `result`.
    fn collect_crate_entry(
        &self,
        entry: std::io::Result<fs::DirEntry>,
        crates_dir: &Path,
        result: &mut CrateDiscoveryResult,
    ) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                result.diagnostics.push(format!(
                    "Failed to read entry in {}: {}",
                    unix_path(crates_dir),
                    e
                ));
                return;
            },
        };
        let path = entry.path();

        if !path.is_dir() {
            return;
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();

        // Skip if we already have a crate with this name (first directory wins)
        if result.crates.iter().any(|c| c.name == name) {
            return;
        }

        let docs_path = path.join("agents").join("docs");
        let index_path = docs_path.join("index.yaml");

        if !docs_path.exists() || !index_path.exists() {
            return; // Silently skip directories without docs
        }

        match parse_crate_index(&index_path) {
            Ok(meta) => {
                let readme_path = docs_path.join("README.md");
                result.crates.push(CrateSummary {
                    name: meta.name,
                    version: meta.version,
                    description: meta.description,
                    module_count: meta.modules.len(),
                    has_readme: readme_path.exists(),
                    crate_path: unix_path(&path),
                    docs_path: unix_path(&docs_path),
                });
            },
            Err(e) => {
                result
                    .diagnostics
                    .push(format!("{}: YAML parse error - {}", name, e));
            },
        }
    }

    /// Discover all context-* crates (simplified, no diagnostics)
    pub fn discover_crates(&self) -> ToolResult<Vec<CrateSummary>> {
        Ok(self.discover_crates_with_diagnostics()?.crates)
    }

    /// Browse a crate's module tree
    pub fn browse_crate(
        &self,
        crate_name: &str,
    ) -> ToolResult<ModuleTreeNode> {
        let crate_path =
            self.resolve_crate_path(crate_name).ok_or_else(|| {
                ToolError::NotFound(format!(
                    "Crate docs not found: {}",
                    crate_name
                ))
            })?;
        let docs_path = crate_path.join("agents").join("docs");
        let index_path = docs_path.join("index.yaml");

        let meta = parse_crate_index(&index_path)?;
        let readme_path = docs_path.join("README.md");

        let mut children = Vec::new();
        let mut all_types = Vec::new();

        for module_ref in &meta.modules {
            // Strip trailing slashes from path to avoid double slashes in paths
            let mod_path = module_ref.path.trim_end_matches('/');
            let module_path = docs_path.join(mod_path);
            if let Ok(node) =
                self.build_module_tree(&module_path, &module_ref.name, mod_path)
            {
                // Collect types from this module with attribution
                for entry in &node.key_types {
                    all_types.push(TypeWithModule::from_entry(
                        entry, mod_path, "type",
                    ));
                }
                // Recursively collect from children
                self.collect_types_from_tree(&node, &mut all_types);
                children.push(node);
            }
        }

        // Collect exported items as key_types and all_types
        let mut key_types = Vec::new();
        if let Some(exported) = &meta.exported_items {
            for entry in &exported.types {
                key_types.push(entry.clone());
                all_types.push(TypeWithModule::from_entry(entry, "", "type"));
            }
            for entry in &exported.traits {
                key_types.push(entry.clone());
                all_types.push(TypeWithModule::from_entry(entry, "", "trait"));
            }
            for entry in &exported.macros {
                key_types.push(entry.clone());
                all_types.push(TypeWithModule::from_entry(entry, "", "macro"));
            }
        }

        Ok(ModuleTreeNode {
            name: meta.name,
            path: String::new(),
            description: meta.description,
            children,
            files: Vec::new(),
            key_types,
            has_readme: readme_path.exists(),
            all_types,
        })
    }

    /// Recursively collect types from module tree with attribution
    fn collect_types_from_tree(
        &self,
        node: &ModuleTreeNode,
        all_types: &mut Vec<TypeWithModule>,
    ) {
        for child in &node.children {
            let child_path = if node.path.is_empty() {
                child.name.clone()
            } else {
                format!("{}/{}", node.path, child.name)
            };
            for entry in &child.key_types {
                all_types.push(TypeWithModule::from_entry(
                    entry,
                    &child_path,
                    "type",
                ));
            }
            self.collect_types_from_tree(child, all_types);
        }
    }

    /// Build a module tree node recursively
    fn build_module_tree(
        &self,
        module_path: &Path,
        name: &str,
        rel_path: &str,
    ) -> ToolResult<ModuleTreeNode> {
        let index_path = module_path.join("index.yaml");

        if !index_path.exists() {
            return Err(ToolError::NotFound(format!(
                "Module docs not found: {}",
                unix_path(module_path)
            )));
        }

        let meta = parse_module_index(&index_path)?;
        let readme_path = module_path.join("README.md");

        let mut children = Vec::new();
        for submodule in &meta.submodules {
            // Strip trailing slashes from path to avoid double slashes in paths
            let submod_path = submodule.path.trim_end_matches('/');
            let sub_path = module_path.join(submod_path);
            let sub_rel_path = format!("{}/{}", rel_path, submod_path);
            if let Ok(node) = self.build_module_tree(
                &sub_path,
                &submodule.name,
                &sub_rel_path,
            ) {
                children.push(node);
            }
        }

        Ok(ModuleTreeNode {
            name: name.to_string(),
            path: rel_path.to_string(),
            description: meta.description,
            children,
            files: meta.files,
            key_types: meta.key_types,
            has_readme: readme_path.exists(),
            all_types: Vec::new(), // Only populated at root level
        })
    }

    /// Read documentation for a crate or module
    pub fn read_crate_doc(
        &self,
        crate_name: &str,
        module_path: Option<&str>,
        include_readme: bool,
    ) -> ToolResult<CrateDocResult> {
        let crate_path =
            self.resolve_crate_path(crate_name).ok_or_else(|| {
                ToolError::NotFound(format!(
                    "Crate docs not found: {}",
                    crate_name
                ))
            })?;
        let docs_path = crate_path.join("agents").join("docs");

        let target_path = match module_path {
            Some(rel_path) => docs_path.join(rel_path),
            None => docs_path.clone(),
        };

        let index_path = target_path.join("index.yaml");
        let readme_path = target_path.join("README.md");

        if !index_path.exists() {
            return Err(ToolError::NotFound(format!(
                "Documentation not found: {}/{}",
                crate_name,
                module_path.unwrap_or("")
            )));
        }

        let index_content = fs::read_to_string(&index_path)?;
        let readme_content = if include_readme && readme_path.exists() {
            Some(fs::read_to_string(&readme_path)?)
        } else {
            None
        };

        // Parse source_files from index.yaml and create file links
        let source_files =
            self.extract_source_file_links(&index_content, &crate_path);

        let crate_path_str = unix_path(&crate_path);

        Ok(CrateDocResult {
            crate_name: crate_name.to_string(),
            module_path: module_path.map(normalize_path_str),
            index_yaml: index_content,
            readme: readme_content,
            crate_path: crate_path_str,
            source_files,
        })
    }

    /// Extract source file links from index.yaml content
    fn extract_source_file_links(
        &self,
        yaml_content: &str,
        crate_path: &Path,
    ) -> Vec<SourceFileLink> {
        // Try to extract source_files from YAML - handles both crate and module formats
        let source_files: Vec<String> =
            serde_yaml::from_str::<serde_yaml::Value>(yaml_content)
                .ok()
                .and_then(|v| v.get("source_files").cloned())
                .and_then(|v| serde_yaml::from_value(v).ok())
                .unwrap_or_default();

        source_files
            .into_iter()
            .map(|rel_path| {
                let rel_path = normalize_path_str(&rel_path);
                let abs_path = crate_path.join(&rel_path);
                let abs_path_str = unix_path(&abs_path);
                let vscode_uri = to_vscode_file_uri(&abs_path);
                SourceFileLink {
                    rel_path,
                    abs_path: abs_path_str,
                    vscode_uri,
                }
            })
            .collect()
    }

    /// Update documentation for a crate or module
    pub fn update_crate_doc(
        &self,
        crate_name: &str,
        module_path: Option<&str>,
        index_yaml: Option<&str>,
        readme: Option<&str>,
    ) -> ToolResult<()> {
        let crate_path =
            self.resolve_crate_path(crate_name).ok_or_else(|| {
                ToolError::NotFound(format!(
                    "Crate docs not found: {}",
                    crate_name
                ))
            })?;
        let docs_path = crate_path.join("agents").join("docs");

        let target_path = match module_path {
            Some(rel_path) => docs_path.join(rel_path),
            None => docs_path.clone(),
        };

        if !target_path.exists() {
            return Err(ToolError::NotFound(format!(
                "Documentation path not found: {}/{}",
                crate_name,
                module_path.unwrap_or("")
            )));
        }

        // Validate YAML before writing
        if let Some(yaml) = index_yaml {
            // Try to parse the YAML to validate it
            if module_path.is_some() {
                serde_yaml::from_str::<ModuleMetadata>(yaml).map_err(|e| {
                    ToolError::InvalidInput(format!(
                        "Invalid module YAML: {}",
                        e
                    ))
                })?;
            } else {
                serde_yaml::from_str::<CrateMetadata>(yaml).map_err(|e| {
                    ToolError::InvalidInput(format!(
                        "Invalid crate YAML: {}",
                        e
                    ))
                })?;
            }
            fs::write(target_path.join("index.yaml"), yaml)?;
        }

        if let Some(md) = readme {
            fs::write(target_path.join("README.md"), md)?;
        }

        Ok(())
    }

    /// Create documentation for a new module
    pub fn create_module_doc(
        &self,
        crate_name: &str,
        module_path: &str,
        name: &str,
        description: &str,
    ) -> ToolResult<String> {
        let crate_path =
            self.resolve_crate_path(crate_name).ok_or_else(|| {
                ToolError::NotFound(format!(
                    "Crate docs not found: {}",
                    crate_name
                ))
            })?;
        let docs_path =
            crate_path.join("agents").join("docs").join(module_path);

        if docs_path.exists() {
            return Err(ToolError::AlreadyExists(format!(
                "Module docs already exist: {}/{}",
                crate_name, module_path
            )));
        }

        fs::create_dir_all(&docs_path)?;

        let meta = ModuleMetadata {
            name: name.to_string(),
            description: description.to_string(),
            submodules: Vec::new(),
            files: Vec::new(),
            key_types: Vec::new(),
            source_files: Vec::new(),
            last_synced: None,
        };

        let yaml = serde_yaml::to_string(&meta).map_err(|e| {
            ToolError::InvalidInput(format!("YAML serialization error: {}", e))
        })?;

        fs::write(docs_path.join("index.yaml"), yaml)?;

        Ok(unix_path(&docs_path))
    }

    /// Delete module documentation directory
    pub fn delete_module_doc(
        &self,
        crate_name: &str,
        module_path: &str,
    ) -> ToolResult<String> {
        let crate_path =
            self.resolve_crate_path(crate_name).ok_or_else(|| {
                ToolError::NotFound(format!(
                    "Crate docs not found: {}",
                    crate_name
                ))
            })?;
        let docs_path =
            crate_path.join("agents").join("docs").join(module_path);

        if !docs_path.exists() {
            return Err(ToolError::NotFound(format!(
                "Module docs not found: {}/{}",
                crate_name, module_path
            )));
        }

        // Remove the directory and all contents
        fs::remove_dir_all(&docs_path)?;

        Ok(unix_path(&docs_path))
    }

    /// Apply `set` / `add` / `remove` source-file mutations to an index's
    /// `source_files` list, recording a human-readable change per mutation.
    fn apply_source_file_updates(
        source_files: &mut Vec<String>,
        set: Option<Vec<String>>,
        add: Option<Vec<String>>,
        remove: Option<Vec<String>>,
        changes: &mut Vec<String>,
    ) {
        if let Some(files) = set {
            *source_files = files;
            changes.push("Set source_files".to_string());
        }
        if let Some(files) = add {
            for f in files {
                if !source_files.contains(&f) {
                    source_files.push(f.clone());
                    changes.push(format!("Added source file: {}", f));
                }
            }
        }
        if let Some(files) = remove {
            for f in &files {
                if let Some(pos) = source_files.iter().position(|x| x == f) {
                    source_files.remove(pos);
                    changes.push(format!("Removed source file: {}", f));
                }
            }
        }
    }

    /// Update specific fields in a crate or module's index.yaml
    ///
    /// This allows programmatic updates to source_files and other metadata
    /// without having to rewrite the entire file.
    pub fn update_crate_index(
        &self,
        crate_name: &str,
        module_path: Option<&str>,
        source_files: Option<Vec<String>>,
        add_source_files: Option<Vec<String>>,
        remove_source_files: Option<Vec<String>>,
    ) -> ToolResult<String> {
        let crate_path =
            self.resolve_crate_path(crate_name).ok_or_else(|| {
                ToolError::NotFound(format!(
                    "Crate docs not found: {}",
                    crate_name
                ))
            })?;
        let docs_path = crate_path.join("agents").join("docs");

        let target_path = match module_path {
            Some(mp) => docs_path.join(mp),
            None => docs_path,
        };

        let index_path = target_path.join("index.yaml");

        if !index_path.exists() {
            return Err(ToolError::NotFound(format!(
                "Index not found: {}/{}",
                crate_name,
                module_path.unwrap_or("")
            )));
        }

        let content = fs::read_to_string(&index_path)?;
        let mut changes = Vec::new();

        let source_files = source_files.map(|files| {
            files
                .into_iter()
                .map(|f| normalize_path_str(&f))
                .collect::<Vec<_>>()
        });
        let add_source_files = add_source_files.map(|files| {
            files
                .into_iter()
                .map(|f| normalize_path_str(&f))
                .collect::<Vec<_>>()
        });
        let remove_source_files = remove_source_files.map(|files| {
            files
                .into_iter()
                .map(|f| normalize_path_str(&f))
                .collect::<Vec<_>>()
        });

        if module_path.is_some() {
            let mut meta: ModuleMetadata = serde_yaml::from_str(&content)
                .map_err(|e| {
                    ToolError::InvalidInput(format!("Invalid YAML: {}", e))
                })?;

            meta.source_files = meta
                .source_files
                .into_iter()
                .map(|f| normalize_path_str(&f))
                .collect();

            Self::apply_source_file_updates(
                &mut meta.source_files,
                source_files,
                add_source_files,
                remove_source_files,
                &mut changes,
            );

            let yaml = serde_yaml::to_string(&meta).map_err(|e| {
                ToolError::InvalidInput(format!(
                    "YAML serialization error: {}",
                    e
                ))
            })?;
            fs::write(&index_path, yaml)?;
        } else {
            let mut meta: CrateMetadata = serde_yaml::from_str(&content)
                .map_err(|e| {
                    ToolError::InvalidInput(format!("Invalid YAML: {}", e))
                })?;

            meta.source_files = meta
                .source_files
                .into_iter()
                .map(|f| normalize_path_str(&f))
                .collect();

            Self::apply_source_file_updates(
                &mut meta.source_files,
                source_files,
                add_source_files,
                remove_source_files,
                &mut changes,
            );

            let yaml = serde_yaml::to_string(&meta).map_err(|e| {
                ToolError::InvalidInput(format!(
                    "YAML serialization error: {}",
                    e
                ))
            })?;
            fs::write(&index_path, yaml)?;
        }

        if changes.is_empty() {
            Ok("No changes made".to_string())
        } else {
            Ok(format!(
                "Updated {}/{}:\n- {}",
                crate_name,
                module_path.unwrap_or(""),
                changes.join("\n- ")
            ))
        }
    }

    /// Search crate documentation (supports regex patterns for flexible matching)
    pub fn search_crate_docs(
        &self,
        query: &str,
        crate_filter: Option<&str>,
        search_types: bool,
        search_content: bool,
    ) -> ToolResult<Vec<CrateSearchResult>> {
        let mut results = Vec::new();
        let regex = compile_search_regex(query)?;

        let crates = self.discover_crates()?;

        for crate_summary in crates {
            if let Some(filter) = crate_filter {
                if crate_summary.name != filter {
                    continue;
                }
            }

            self.search_within_crate(
                &crate_summary,
                &regex,
                search_types,
                search_content,
                &mut results,
            );
        }

        Ok(results)
    }

    /// Search a single crate's index, exported items, modules, and README,
    /// appending any matches to `results`.
    fn search_within_crate(
        &self,
        crate_summary: &CrateSummary,
        regex: &Option<Regex>,
        search_types: bool,
        search_content: bool,
        results: &mut Vec<CrateSearchResult>,
    ) {
        let docs_path = PathBuf::from(&crate_summary.docs_path);

        // Search crate-level
        if let Ok(meta) = parse_crate_index(&docs_path.join("index.yaml")) {
            // Search description
            if regex_matches(&meta.description, regex) {
                results.push(CrateSearchResult {
                    crate_name: crate_summary.name.clone(),
                    module_path: String::new(),
                    match_type: "crate".to_string(),
                    name: meta.name.clone(),
                    description: Some(meta.description.clone()),
                    context: None,
                });
            }

            // Search exported items
            if search_types {
                if let Some(exported) = &meta.exported_items {
                    results.extend(self.search_type_entries(
                        &exported.types,
                        regex,
                        &crate_summary.name,
                        "",
                        "type",
                    ));
                    results.extend(self.search_type_entries(
                        &exported.traits,
                        regex,
                        &crate_summary.name,
                        "",
                        "trait",
                    ));
                    results.extend(self.search_type_entries(
                        &exported.macros,
                        regex,
                        &crate_summary.name,
                        "",
                        "macro",
                    ));
                }
            }

            // Search modules recursively
            for module_ref in &meta.modules {
                let searchable =
                    format!("{} {}", module_ref.name, module_ref.description);
                if regex_matches(&searchable, regex) {
                    results.push(CrateSearchResult {
                        crate_name: crate_summary.name.clone(),
                        module_path: module_ref.path.clone(),
                        match_type: "module".to_string(),
                        name: module_ref.name.clone(),
                        description: Some(module_ref.description.clone()),
                        context: None,
                    });
                }

                // Search within module
                let module_path = docs_path.join(&module_ref.path);
                results.extend(self.search_module(
                    &module_path,
                    regex,
                    &crate_summary.name,
                    &module_ref.path,
                    search_types,
                    search_content,
                ));
            }
        }

        // Search README content
        if search_content {
            let readme_path = docs_path.join("README.md");
            if let Ok(content) = read_markdown_file(&readme_path) {
                if let Some(context) =
                    self.find_context_in_content(&content, regex)
                {
                    results.push(CrateSearchResult {
                        crate_name: crate_summary.name.clone(),
                        module_path: String::new(),
                        match_type: "content".to_string(),
                        name: "README.md".to_string(),
                        description: None,
                        context: Some(context),
                    });
                }
            }
        }
    }

    fn search_type_entries(
        &self,
        entries: &[TypeEntry],
        regex: &Option<Regex>,
        crate_name: &str,
        module_path: &str,
        match_type: &str,
    ) -> Vec<CrateSearchResult> {
        entries
            .iter()
            .filter_map(|entry| {
                let name = entry.name();
                let desc = entry.description().unwrap_or("");

                // Combine name and description for regex search
                let searchable = format!("{} {}", name, desc);

                if regex_matches(&searchable, regex) {
                    // Build context showing the description
                    let context = if !desc.is_empty() {
                        Some(truncate(desc, 100))
                    } else {
                        None
                    };

                    Some(CrateSearchResult {
                        crate_name: crate_name.to_string(),
                        module_path: module_path.to_string(),
                        match_type: match_type.to_string(),
                        name: entry.name().to_string(),
                        description: entry.description().map(|s| s.to_string()),
                        context,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    fn search_module(
        &self,
        module_path: &Path,
        regex: &Option<Regex>,
        crate_name: &str,
        rel_path: &str,
        search_types: bool,
        search_content: bool,
    ) -> Vec<CrateSearchResult> {
        let mut results = Vec::new();
        let index_path = module_path.join("index.yaml");

        if let Ok(meta) = parse_module_index(&index_path) {
            // Search key_types
            if search_types {
                results.extend(self.search_type_entries(
                    &meta.key_types,
                    regex,
                    crate_name,
                    rel_path,
                    "type",
                ));
            }

            // Search files
            for file in &meta.files {
                let searchable = format!("{} {}", file.name, file.description);
                if regex_matches(&searchable, regex) {
                    results.push(CrateSearchResult {
                        crate_name: crate_name.to_string(),
                        module_path: rel_path.to_string(),
                        match_type: "file".to_string(),
                        name: file.name.clone(),
                        description: Some(file.description.clone()),
                        context: None,
                    });
                }
            }

            // Search submodules recursively
            for submodule in &meta.submodules {
                let searchable =
                    format!("{} {}", submodule.name, submodule.description);
                if regex_matches(&searchable, regex) {
                    let sub_rel_path =
                        format!("{}/{}", rel_path, submodule.path);
                    results.push(CrateSearchResult {
                        crate_name: crate_name.to_string(),
                        module_path: sub_rel_path.clone(),
                        match_type: "module".to_string(),
                        name: submodule.name.clone(),
                        description: Some(submodule.description.clone()),
                        context: None,
                    });
                }

                let sub_path = module_path.join(&submodule.path);
                let sub_rel_path = format!("{}/{}", rel_path, submodule.path);
                results.extend(self.search_module(
                    &sub_path,
                    regex,
                    crate_name,
                    &sub_rel_path,
                    search_types,
                    search_content,
                ));
            }

            // Search README content
            if search_content {
                let readme_path = module_path.join("README.md");
                if let Ok(content) = read_markdown_file(&readme_path) {
                    if let Some(context) =
                        self.find_context_in_content(&content, regex)
                    {
                        results.push(CrateSearchResult {
                            crate_name: crate_name.to_string(),
                            module_path: rel_path.to_string(),
                            match_type: "content".to_string(),
                            name: "README.md".to_string(),
                            description: None,
                            context: Some(context),
                        });
                    }
                }
            }
        }

        results
    }

    fn find_context_in_content(
        &self,
        content: &str,
        regex: &Option<Regex>,
    ) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if regex_matches(line, regex) {
                // Include previous and next line for context
                let mut context_parts = Vec::new();
                if i > 0 {
                    let prev = truncate(lines[i - 1].trim(), 50);
                    if !prev.is_empty() {
                        context_parts.push(prev);
                    }
                }
                context_parts.push(truncate(line.trim(), 100));
                if i + 1 < lines.len() {
                    let next = truncate(lines[i + 1].trim(), 50);
                    if !next.is_empty() {
                        context_parts.push(next);
                    }
                }
                return Some(context_parts.join(" | "));
            }
        }
        None
    }

    /// Validate crate documentation for consistency
    pub fn validate_crate_docs(
        &self,
        crate_filter: Option<&str>,
    ) -> ToolResult<CrateValidationReport> {
        let mut report = CrateValidationReport::default();
        let crates = self.discover_crates()?;

        for crate_summary in crates {
            if let Some(filter) = crate_filter {
                if crate_summary.name != filter {
                    continue;
                }
            }

            report.crates_checked += 1;

            let docs_path = PathBuf::from(&crate_summary.docs_path);
            let index_path = docs_path.join("index.yaml");

            // Check crate index
            match parse_crate_index(&index_path) {
                Ok(meta) => {
                    // Check all referenced modules exist
                    for module_ref in &meta.modules {
                        let module_path = docs_path.join(&module_ref.path);
                        if !module_path.exists() {
                            report.issues.push(CrateValidationIssue {
                                crate_name: crate_summary.name.clone(),
                                module_path: Some(module_ref.path.clone()),
                                issue: format!(
                                    "Referenced module '{}' does not exist",
                                    module_ref.path
                                ),
                                severity: "error".to_string(),
                            });
                        } else {
                            // Recursively validate module
                            self.validate_module(
                                &module_path,
                                &crate_summary.name,
                                &module_ref.path,
                                &mut report,
                            );
                        }
                    }

                    // Warn about missing README
                    if !docs_path.join("README.md").exists() {
                        report.issues.push(CrateValidationIssue {
                            crate_name: crate_summary.name.clone(),
                            module_path: None,
                            issue: "Missing README.md".to_string(),
                            severity: "warning".to_string(),
                        });
                    }
                },
                Err(e) => {
                    report.issues.push(CrateValidationIssue {
                        crate_name: crate_summary.name.clone(),
                        module_path: None,
                        issue: format!("Failed to parse index.yaml: {}", e),
                        severity: "error".to_string(),
                    });
                },
            }
        }

        Ok(report)
    }

    fn validate_module(
        &self,
        module_path: &Path,
        crate_name: &str,
        rel_path: &str,
        report: &mut CrateValidationReport,
    ) {
        report.modules_checked += 1;

        let index_path = module_path.join("index.yaml");

        match parse_module_index(&index_path) {
            Ok(meta) => {
                // Check all referenced submodules exist
                for submodule in &meta.submodules {
                    let sub_path = module_path.join(&submodule.path);
                    if !sub_path.exists() {
                        report.issues.push(CrateValidationIssue {
                            crate_name: crate_name.to_string(),
                            module_path: Some(format!(
                                "{}/{}",
                                rel_path, submodule.path
                            )),
                            issue: format!(
                                "Referenced submodule '{}' does not exist",
                                submodule.path
                            ),
                            severity: "error".to_string(),
                        });
                    } else {
                        let sub_rel_path =
                            format!("{}/{}", rel_path, submodule.path);
                        self.validate_module(
                            &sub_path,
                            crate_name,
                            &sub_rel_path,
                            report,
                        );
                    }
                }

                // Warn about missing description
                if meta.description.is_empty() {
                    report.issues.push(CrateValidationIssue {
                        crate_name: crate_name.to_string(),
                        module_path: Some(rel_path.to_string()),
                        issue: "Empty description".to_string(),
                        severity: "warning".to_string(),
                    });
                }
            },
            Err(e) => {
                report.issues.push(CrateValidationIssue {
                    crate_name: crate_name.to_string(),
                    module_path: Some(rel_path.to_string()),
                    issue: format!("Failed to parse index.yaml: {}", e),
                    severity: "error".to_string(),
                });
            },
        }
    }

    // =========================================================================
    // Stale Detection
    // =========================================================================

    /// Check documentation staleness using git history
    ///
    /// Compares the `last_synced` timestamp in index.yaml files against
    /// the git modification times of tracked `source_files`.
    ///
    /// # Arguments
    /// * `crate_filter` - Optional crate name to check only one crate
    /// * `stale_threshold_days` - Number of days after which docs are considered stale (default: 7)
    /// * `very_stale_threshold_days` - Number of days after which docs are considered very stale (default: 30)
    pub fn check_stale_docs(
        &self,
        crate_filter: Option<&str>,
        stale_threshold_days: i64,
        very_stale_threshold_days: i64,
    ) -> ToolResult<StaleDocsReport> {
        let mut report = StaleDocsReport::default();
        let crates = self.discover_crates()?;

        for crate_summary in crates {
            if let Some(filter) = crate_filter {
                if crate_summary.name != filter {
                    continue;
                }
            }

            let crate_path = PathBuf::from(&crate_summary.crate_path);

            // Check if this crate is in a git repo
            if !is_git_repository(&crate_path) {
                // Skip crates not in a git repository
                continue;
            }

            report.crates_checked += 1;

            let docs_path = PathBuf::from(&crate_summary.docs_path);
            let index_path = docs_path.join("index.yaml");

            // Check crate-level staleness
            if let Ok(meta) = parse_crate_index(&index_path) {
                let item = self.check_staleness_for_item(
                    &crate_path,
                    &crate_summary.name,
                    None,
                    &meta.source_files,
                    meta.last_synced.as_deref(),
                    stale_threshold_days,
                    very_stale_threshold_days,
                );

                self.categorize_stale_item(&mut report, item);

                // Check module-level staleness
                for module_ref in &meta.modules {
                    self.check_module_staleness(
                        &crate_path,
                        &docs_path.join(&module_ref.path),
                        &crate_summary.name,
                        &module_ref.path,
                        stale_threshold_days,
                        very_stale_threshold_days,
                        &mut report,
                    );
                }
            }
        }

        // Calculate summary
        report.summary = StaleSummary {
            total_items: report.fresh_items.len()
                + report.stale_items.len()
                + report.unknown_items.len(),
            fresh_count: report.fresh_items.len(),
            stale_count: report
                .stale_items
                .iter()
                .filter(|i| i.staleness == StalenessLevel::Stale)
                .count(),
            very_stale_count: report
                .stale_items
                .iter()
                .filter(|i| i.staleness == StalenessLevel::VeryStale)
                .count(),
            unknown_count: report.unknown_items.len(),
        };

        Ok(report)
    }

    fn check_module_staleness(
        &self,
        crate_path: &Path,
        module_docs_path: &Path,
        crate_name: &str,
        module_rel_path: &str,
        stale_threshold_days: i64,
        very_stale_threshold_days: i64,
        report: &mut StaleDocsReport,
    ) {
        report.modules_checked += 1;

        let index_path = module_docs_path.join("index.yaml");

        if let Ok(meta) = parse_module_index(&index_path) {
            let item = self.check_staleness_for_item(
                crate_path,
                crate_name,
                Some(module_rel_path),
                &meta.source_files,
                meta.last_synced.as_deref(),
                stale_threshold_days,
                very_stale_threshold_days,
            );

            self.categorize_stale_item(report, item);

            // Recursively check submodules
            for submodule in &meta.submodules {
                let sub_path = module_docs_path.join(&submodule.path);
                let sub_rel_path =
                    format!("{}/{}", module_rel_path, submodule.path);
                self.check_module_staleness(
                    crate_path,
                    &sub_path,
                    crate_name,
                    &sub_rel_path,
                    stale_threshold_days,
                    very_stale_threshold_days,
                    report,
                );
            }
        }
    }

    fn check_staleness_for_item(
        &self,
        crate_path: &Path,
        crate_name: &str,
        module_path: Option<&str>,
        source_files: &[String],
        last_synced: Option<&str>,
        stale_threshold_days: i64,
        very_stale_threshold_days: i64,
    ) -> StaleDocItem {
        // If no source files are configured, status is unknown
        if source_files.is_empty() {
            return StaleDocItem {
                crate_name: crate_name.to_string(),
                module_path: module_path.map(normalize_path_str),
                staleness: StalenessLevel::Unknown,
                doc_last_synced: last_synced.map(|s| s.to_string()),
                source_last_modified: None,
                days_since_sync: last_synced.and_then(days_since),
                days_since_source_change: None,
                source_files: Vec::new(),
                modified_files: Vec::new(),
            };
        }

        // Get git info for source files
        let file_infos = get_files_info(crate_path, source_files);
        let source_last_modified = get_most_recent_modification(&file_infos);

        // Determine modified files since last sync
        let modified_files = match last_synced {
            Some(synced) => get_files_modified_since(&file_infos, synced),
            None => file_infos.iter().map(|f| f.path.clone()).collect(),
        };

        // Calculate days
        let days_since_sync = last_synced.and_then(days_since);
        let days_since_source_change =
            source_last_modified.as_ref().and_then(|ts| days_since(ts));

        // Determine staleness level
        let staleness = if modified_files.is_empty() {
            StalenessLevel::Fresh
        } else {
            match days_since_sync {
                Some(days) if days >= very_stale_threshold_days =>
                    StalenessLevel::VeryStale,
                Some(days) if days >= stale_threshold_days =>
                    StalenessLevel::Stale,
                Some(_) => {
                    // Recent sync but still have modified files
                    if modified_files.is_empty() {
                        StalenessLevel::Fresh
                    } else {
                        StalenessLevel::Stale
                    }
                },
                None => {
                    // Never synced
                    StalenessLevel::VeryStale
                },
            }
        };

        StaleDocItem {
            crate_name: crate_name.to_string(),
            module_path: module_path.map(normalize_path_str),
            staleness,
            doc_last_synced: last_synced.map(|s| s.to_string()),
            source_last_modified,
            days_since_sync,
            days_since_source_change,
            source_files: file_infos,
            modified_files,
        }
    }

    fn categorize_stale_item(
        &self,
        report: &mut StaleDocsReport,
        item: StaleDocItem,
    ) {
        match item.staleness {
            StalenessLevel::Fresh => report.fresh_items.push(item),
            StalenessLevel::Stale | StalenessLevel::VeryStale =>
                report.stale_items.push(item),
            StalenessLevel::Unknown => report.unknown_items.push(item),
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

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
// Result Types
// =============================================================================

/// Result of reading crate documentation
#[derive(Debug, Serialize)]
pub struct CrateDocResult {
    pub crate_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_path: Option<String>,
    pub index_yaml: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readme: Option<String>,
    /// Absolute path to the crate root directory
    pub crate_path: String,
    /// Source files with absolute paths and editor URIs
    #[serde(default)]
    pub source_files: Vec<SourceFileLink>,
}

/// Source file information with linkable paths
#[derive(Debug, Clone, Serialize)]
pub struct SourceFileLink {
    /// Relative path from crate root (e.g., "src/lib.rs")
    pub rel_path: String,
    /// Absolute filesystem path
    pub abs_path: String,
    /// VS Code URI to open the file (vscode://file/...)
    pub vscode_uri: String,
}

impl CrateDocResult {
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        let location = match &self.module_path {
            Some(path) =>
                format!("{}::{}", self.crate_name, path.replace('/', "::")),
            None => self.crate_name.clone(),
        };
        md.push_str(&format!("# Documentation: {}\n\n", location));

        // Add source files section with clickable links
        if !self.source_files.is_empty() {
            md.push_str("## Source Files\n\n");
            for file in &self.source_files {
                md.push_str(&format!(
                    "- [{}]({})\n",
                    file.rel_path, file.vscode_uri
                ));
            }
            md.push_str("\n");
        }

        md.push_str("## index.yaml\n\n```yaml\n");
        md.push_str(&self.index_yaml);
        md.push_str("```\n\n");
        if let Some(readme) = &self.readme {
            md.push_str("## README.md\n\n");
            md.push_str(readme);
        }
        md
    }
}
