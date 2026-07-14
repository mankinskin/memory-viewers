//! Parse existing documents to extract metadata.

use crate::{
    helpers::unix_path,
    schema::{
        CrateMetadata,
        DocMetadata,
        DocType,
        ModuleMetadata,
        PlanStatus,
    },
};
use regex::Regex;
use serde::de::DeserializeOwned;
use std::{
    fs,
    path::Path,
};

/// Parse a document filename to extract date and base name.
pub fn parse_filename(filename: &str) -> Option<(String, String)> {
    let re = Regex::new(r"^(\d{8})_(.+)\.md$").ok()?;
    let caps = re.captures(filename)?;
    Some((caps[1].to_string(), caps[2].to_string()))
}

/// Parse frontmatter from document content.
pub fn parse_frontmatter(content: &str) -> Option<FrontMatter> {
    let lines: Vec<&str> = content.lines().collect();

    if lines.first()?.trim() != "---" {
        return None;
    }

    let end_idx = lines.iter().skip(1).position(|l| l.trim() == "---")? + 1;

    let mut fm = FrontMatter::default();

    for line in &lines[1..end_idx] {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();

            match key {
                "tags" => fm.tags = parse_tags(value),
                "summary" => fm.summary = Some(value.to_string()),
                "status" => fm.status = parse_status(value),
                _ => {},
            }
        }
    }

    Some(fm)
}

#[derive(Default)]
pub struct FrontMatter {
    pub tags: Vec<String>,
    pub summary: Option<String>,
    pub status: Option<PlanStatus>,
}

fn parse_tags(value: &str) -> Vec<String> {
    let re = Regex::new(r"`#([^`]+)`").unwrap();
    re.captures_iter(value)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn parse_status(value: &str) -> Option<PlanStatus> {
    match value.trim() {
        "📋" => Some(PlanStatus::Design),
        "🚧" => Some(PlanStatus::InProgress),
        "✅" => Some(PlanStatus::Completed),
        "⚠️" => Some(PlanStatus::Blocked),
        "❌" => Some(PlanStatus::Superseded),
        _ => None,
    }
}

/// Parse title from first H1 heading.
pub fn parse_title(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            return Some(trimmed[2..].trim().to_string());
        }
    }
    None
}

/// Extract document metadata from file path and content.
pub fn extract_metadata(
    path: &Path,
    content: &str,
) -> Option<DocMetadata> {
    let filename = path.file_name()?.to_str()?;
    let parent = path.parent()?.file_name()?.to_str()?;

    let doc_type = DocType::from_directory(parent)?;
    let (date, _base_name) = parse_filename(filename)?;

    let fm = parse_frontmatter(content).unwrap_or_default();
    let title = parse_title(content).unwrap_or_else(|| filename.to_string());

    Some(DocMetadata {
        doc_type,
        date,
        title,
        filename: filename.to_string(),
        tags: fm.tags,
        summary: fm.summary.unwrap_or_default(),
        status: fm.status,
    })
}

// =============================================================================
// YAML Parsing for Crate Documentation
// =============================================================================

/// Parse a YAML file into a typed structure.
pub fn parse_yaml_file<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", unix_path(path), e))?;
    parse_yaml_content(&content)
}

/// Parse YAML content into a typed structure.
pub fn parse_yaml_content<T: DeserializeOwned>(
    content: &str
) -> Result<T, String> {
    serde_yaml::from_str(content)
        .map_err(|e| format!("YAML parse error: {}", e))
}

/// Parse a crate's root index.yaml file.
pub fn parse_crate_index(path: &Path) -> Result<CrateMetadata, String> {
    parse_yaml_file(path)
}

/// Parse a module's index.yaml file.
pub fn parse_module_index(path: &Path) -> Result<ModuleMetadata, String> {
    parse_yaml_file(path)
}

/// Read a markdown file's content.
pub fn read_markdown_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", unix_path(path), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_filename() {
        let (date, name) =
            parse_filename("20251203_SEARCH_ALGORITHM_GUIDE.md").unwrap();
        assert_eq!(date, "20251203");
        assert_eq!(name, "SEARCH_ALGORITHM_GUIDE");
    }

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
tags: `#testing` `#api`
summary: A test document
---

# Title
"#;
        let fm = parse_frontmatter(content).unwrap();
        assert_eq!(fm.tags, vec!["testing", "api"]);
        assert_eq!(fm.summary, Some("A test document".to_string()));
    }
}
