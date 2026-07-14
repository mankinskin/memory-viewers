use super::*;

impl CrateDocsManager {
    // =========================================================================
    // Sync Documentation
    // =========================================================================

    /// Analyze source files and suggest documentation updates
    ///
    /// Parses Rust source files to extract public items and compares
    /// them against the current documentation to suggest additions,
    /// updates, or removals.
    ///
    /// If `summary_only` is true, returns only counts and suggestions
    /// without listing all found items.
    pub fn sync_crate_docs(
        &self,
        crate_name: &str,
        module_path: Option<&str>,
        update_timestamp: bool,
        summary_only: bool,
    ) -> ToolResult<SyncAnalysisResult> {
        let crate_path =
            self.resolve_crate_path(crate_name).ok_or_else(|| {
                ToolError::NotFound(format!(
                    "Crate docs not found: {}",
                    crate_name
                ))
            })?;
        let docs_path = crate_path.join("agents").join("docs");

        let target_docs_path = match module_path {
            Some(mp) => docs_path.join(mp),
            None => docs_path.clone(),
        };

        let index_path = target_docs_path.join("index.yaml");

        if !index_path.exists() {
            return Err(ToolError::NotFound(format!(
                "Documentation not found: {}/{}",
                crate_name,
                module_path.unwrap_or("")
            )));
        }

        let mut result = SyncAnalysisResult {
            crate_name: crate_name.to_string(),
            module_path: module_path.map(|s| s.to_string()),
            suggestions: Vec::new(),
            public_types: Vec::new(),
            public_traits: Vec::new(),
            public_macros: Vec::new(),
            files_analyzed: Vec::new(),
            errors: Vec::new(),
            summary: None,
        };

        // Get source files to analyze
        let source_files =
            Self::gather_sync_source_files(&index_path, module_path);

        if source_files.is_empty() {
            result
                .errors
                .push("No source_files configured in index.yaml".to_string());
            return Ok(result);
        }

        // Analyze each source file
        self.analyze_sync_source_files(&crate_path, &source_files, &mut result);

        // Compare with existing documentation and generate suggestions
        self.compare_sync_docs(&index_path, module_path, &mut result);

        // Update last_synced timestamp if requested
        if update_timestamp {
            self.update_last_synced(&index_path, module_path.is_some())?;
        }

        // Calculate summary
        let to_add = result
            .suggestions
            .iter()
            .filter(|s| s.change_type == "add")
            .count();
        let to_remove = result
            .suggestions
            .iter()
            .filter(|s| s.change_type == "remove")
            .count();
        result.summary = Some(SyncSummary {
            types_found: result.public_types.len(),
            traits_found: result.public_traits.len(),
            macros_found: result.public_macros.len(),
            to_add,
            to_remove,
        });

        // In summary mode, clear verbose data
        if summary_only {
            result.public_types.clear();
            result.public_traits.clear();
            result.public_macros.clear();
        }

        Ok(result)
    }

    /// Read the configured `source_files` list from a crate or module index.
    fn gather_sync_source_files(
        index_path: &Path,
        module_path: Option<&str>,
    ) -> Vec<String> {
        if module_path.is_some() {
            parse_module_index(index_path)
                .map(|meta| meta.source_files)
                .unwrap_or_default()
        } else {
            parse_crate_index(index_path)
                .map(|meta| meta.source_files)
                .unwrap_or_default()
        }
    }

    /// Analyze each configured source file, recording public items or errors.
    fn analyze_sync_source_files(
        &self,
        crate_path: &Path,
        source_files: &[String],
        result: &mut SyncAnalysisResult,
    ) {
        for source_file in source_files {
            let file_path = crate_path.join(source_file);
            if !file_path.exists() {
                result
                    .errors
                    .push(format!("Source file not found: {}", source_file));
                continue;
            }

            result.files_analyzed.push(source_file.clone());

            match fs::read_to_string(&file_path) {
                Ok(content) => {
                    self.analyze_rust_source(&content, source_file, result);
                },
                Err(e) => {
                    result
                        .errors
                        .push(format!("Failed to read {}: {}", source_file, e));
                },
            }
        }
    }

    /// Compare analyzed source items against the documented index and push
    /// add/remove suggestions.
    fn compare_sync_docs(
        &self,
        index_path: &Path,
        module_path: Option<&str>,
        result: &mut SyncAnalysisResult,
    ) {
        if module_path.is_some() {
            if let Ok(meta) = parse_module_index(index_path) {
                self.compare_module_docs(&meta, result);
            }
        } else if let Ok(meta) = parse_crate_index(index_path) {
            self.compare_crate_docs(&meta, result);
        }
    }

    /// Simple Rust source analysis using regex patterns
    ///
    /// Note: This is a simplified parser that looks for common patterns.
    /// For full accuracy, a proper Rust parser like syn would be needed.
    fn analyze_rust_source(
        &self,
        content: &str,
        _file_path: &str,
        result: &mut SyncAnalysisResult,
    ) {
        use regex::Regex;

        // Match public structs: pub struct Name
        let struct_re = Regex::new(r"(?m)^pub\s+struct\s+(\w+)").unwrap();
        for cap in struct_re.captures_iter(content) {
            let name = cap[1].to_string();
            if !result.public_types.contains(&name) {
                result.public_types.push(name);
            }
        }

        // Match public enums: pub enum Name
        let enum_re = Regex::new(r"(?m)^pub\s+enum\s+(\w+)").unwrap();
        for cap in enum_re.captures_iter(content) {
            let name = cap[1].to_string();
            if !result.public_types.contains(&name) {
                result.public_types.push(name);
            }
        }

        // Match public traits: pub trait Name
        let trait_re = Regex::new(r"(?m)^pub\s+trait\s+(\w+)").unwrap();
        for cap in trait_re.captures_iter(content) {
            let name = cap[1].to_string();
            if !result.public_traits.contains(&name) {
                result.public_traits.push(name);
            }
        }

        // Match macros: macro_rules! name or pub macro name (though latter is rare)
        let macro_re =
            Regex::new(r"(?m)^(?:#\[macro_export\]\s*\n)?macro_rules!\s+(\w+)")
                .unwrap();
        for cap in macro_re.captures_iter(content) {
            let name = cap[1].to_string();
            if !result.public_macros.contains(&name) {
                result.public_macros.push(name);
            }
        }

        // Match pub(crate) type aliases: pub type Name
        let type_alias_re = Regex::new(r"(?m)^pub\s+type\s+(\w+)").unwrap();
        for cap in type_alias_re.captures_iter(content) {
            let name = cap[1].to_string();
            if !result.public_types.contains(&name) {
                result.public_types.push(name);
            }
        }
    }

    /// Push "add" suggestions for source items missing from the docs.
    fn push_missing_doc_suggestions(
        source_items: &[String],
        documented: &[String],
        item_kind: &str,
        first_file: &str,
        suggestions: &mut Vec<SyncSuggestion>,
    ) {
        for name in source_items {
            if !documented.contains(name) {
                suggestions.push(SyncSuggestion {
                    change_type: "add".to_string(),
                    item_kind: item_kind.to_string(),
                    item_name: name.clone(),
                    description: None,
                    source_file: first_file.to_string(),
                    line_number: None,
                });
            }
        }
    }

    /// Push "remove" suggestions for documented items missing from source.
    fn push_stale_doc_suggestions(
        documented: &[String],
        source_items: &[String],
        item_kind: &str,
        suggestions: &mut Vec<SyncSuggestion>,
    ) {
        for name in documented {
            if !source_items.contains(name) {
                suggestions.push(SyncSuggestion {
                    change_type: "remove".to_string(),
                    item_kind: item_kind.to_string(),
                    item_name: name.clone(),
                    description: Some(
                        "Not found in analyzed source files".to_string(),
                    ),
                    source_file: String::new(),
                    line_number: None,
                });
            }
        }
    }

    fn compare_crate_docs(
        &self,
        meta: &CrateMetadata,
        result: &mut SyncAnalysisResult,
    ) {
        // Get documented types
        let mut documented_types: Vec<String> = Vec::new();
        let mut documented_traits: Vec<String> = Vec::new();
        let mut documented_macros: Vec<String> = Vec::new();

        if let Some(exported) = &meta.exported_items {
            documented_types
                .extend(exported.types.iter().map(|t| t.name.clone()));
            documented_traits
                .extend(exported.traits.iter().map(|t| t.name.clone()));
            documented_macros
                .extend(exported.macros.iter().map(|t| t.name.clone()));
        }

        let first_file =
            result.files_analyzed.first().cloned().unwrap_or_default();

        // Find items in source but not documented
        Self::push_missing_doc_suggestions(
            &result.public_types,
            &documented_types,
            "type",
            &first_file,
            &mut result.suggestions,
        );
        Self::push_missing_doc_suggestions(
            &result.public_traits,
            &documented_traits,
            "trait",
            &first_file,
            &mut result.suggestions,
        );
        Self::push_missing_doc_suggestions(
            &result.public_macros,
            &documented_macros,
            "macro",
            &first_file,
            &mut result.suggestions,
        );

        // Find documented items that don't exist in source (potential removals)
        Self::push_stale_doc_suggestions(
            &documented_types,
            &result.public_types,
            "type",
            &mut result.suggestions,
        );
        Self::push_stale_doc_suggestions(
            &documented_traits,
            &result.public_traits,
            "trait",
            &mut result.suggestions,
        );
        Self::push_stale_doc_suggestions(
            &documented_macros,
            &result.public_macros,
            "macro",
            &mut result.suggestions,
        );
    }

    fn compare_module_docs(
        &self,
        meta: &ModuleMetadata,
        result: &mut SyncAnalysisResult,
    ) {
        // Get documented key_types
        let documented_types: Vec<String> =
            meta.key_types.iter().map(|t| t.name.clone()).collect();

        // Combine all public items from source
        let mut all_source_items: Vec<String> = Vec::new();
        all_source_items.extend(result.public_types.clone());
        all_source_items.extend(result.public_traits.clone());
        all_source_items.extend(result.public_macros.clone());

        // Find types in source but not documented
        for type_name in &result.public_types {
            if !documented_types.contains(type_name) {
                result.suggestions.push(SyncSuggestion {
                    change_type: "add".to_string(),
                    item_kind: "type".to_string(),
                    item_name: type_name.clone(),
                    description: None,
                    source_file: result
                        .files_analyzed
                        .first()
                        .cloned()
                        .unwrap_or_default(),
                    line_number: None,
                });
            }
        }

        for trait_name in &result.public_traits {
            if !documented_types.contains(trait_name) {
                result.suggestions.push(SyncSuggestion {
                    change_type: "add".to_string(),
                    item_kind: "trait".to_string(),
                    item_name: trait_name.clone(),
                    description: None,
                    source_file: result
                        .files_analyzed
                        .first()
                        .cloned()
                        .unwrap_or_default(),
                    line_number: None,
                });
            }
        }

        // Find documented items that don't exist in source
        for type_name in &documented_types {
            if !all_source_items.contains(type_name) {
                result.suggestions.push(SyncSuggestion {
                    change_type: "remove".to_string(),
                    item_kind: "type".to_string(),
                    item_name: type_name.clone(),
                    description: Some(
                        "Not found in analyzed source files".to_string(),
                    ),
                    source_file: String::new(),
                    line_number: None,
                });
            }
        }
    }

    fn update_last_synced(
        &self,
        index_path: &Path,
        is_module: bool,
    ) -> ToolResult<()> {
        let content = fs::read_to_string(index_path)?;
        let timestamp = current_timestamp();

        let new_content = if is_module {
            let mut meta: ModuleMetadata = serde_yaml::from_str(&content)
                .map_err(|e| {
                    ToolError::InvalidInput(format!(
                        "Failed to parse YAML: {}",
                        e
                    ))
                })?;
            meta.last_synced = Some(timestamp);
            serde_yaml::to_string(&meta).map_err(|e| {
                ToolError::InvalidInput(format!(
                    "Failed to serialize YAML: {}",
                    e
                ))
            })?
        } else {
            let mut meta: CrateMetadata = serde_yaml::from_str(&content)
                .map_err(|e| {
                    ToolError::InvalidInput(format!(
                        "Failed to parse YAML: {}",
                        e
                    ))
                })?;
            meta.last_synced = Some(timestamp);
            serde_yaml::to_string(&meta).map_err(|e| {
                ToolError::InvalidInput(format!(
                    "Failed to serialize YAML: {}",
                    e
                ))
            })?
        };

        fs::write(index_path, new_content)?;
        Ok(())
    }
}
