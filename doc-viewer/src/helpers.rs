//! Helper functions for parsing and formatting.

use std::path::Path;

use viewer_api::to_unix_path;

use crate::{
    schema::{
        DocType,
        ModuleTreeNode,
        PlanStatus,
    },
    tools,
};

pub fn unix_path(path: &Path) -> String {
    to_unix_path(path)
}

pub fn normalize_path_str(path: &str) -> String {
    path.replace('\\', "/")
}

pub fn to_vscode_file_uri(path: &Path) -> String {
    format!("vscode://file/{}", unix_path(path))
}

/// Parse a document type string
pub fn parse_doc_type(s: &str) -> Option<DocType> {
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

/// Parse a detail level string
pub fn parse_detail_level(s: &str) -> tools::DetailLevel {
    match s.to_lowercase().as_str() {
        "outline" => tools::DetailLevel::Outline,
        "full" => tools::DetailLevel::Full,
        _ => tools::DetailLevel::Summary,
    }
}

/// Parse a plan status string
pub fn parse_status(s: &str) -> Option<PlanStatus> {
    match s.to_lowercase().as_str() {
        "design" => Some(PlanStatus::Design),
        "in-progress" | "in_progress" | "inprogress" =>
            Some(PlanStatus::InProgress),
        "completed" | "complete" | "done" => Some(PlanStatus::Completed),
        "blocked" => Some(PlanStatus::Blocked),
        "superseded" | "abandoned" => Some(PlanStatus::Superseded),
        _ => None,
    }
}

/// Format a module tree node as markdown
pub fn format_module_tree(
    node: &ModuleTreeNode,
    depth: usize,
) -> String {
    use std::fmt::Write;
    let mut md = String::new();
    let indent = "  ".repeat(depth);
    let prefix = if depth == 0 {
        "#"
    } else {
        &"#".repeat((depth + 1).min(4))
    };

    let _ = writeln!(md, "{} {}", prefix, node.name);
    if !node.description.is_empty() {
        let _ = writeln!(md, "{}*{}*\n", indent, node.description);
    }

    // Show key types
    if !node.key_types.is_empty() {
        let _ = writeln!(md, "{}**Key Types:**", indent);
        for t in &node.key_types {
            let desc = t
                .description()
                .map(|d| format!(" - {}", d))
                .unwrap_or_default();
            let _ = writeln!(md, "{}- `{}`{}", indent, t.name(), desc);
        }
        let _ = writeln!(md);
    }

    // Show files
    if !node.files.is_empty() {
        let _ = writeln!(md, "{}**Files:**", indent);
        for f in &node.files {
            let _ =
                writeln!(md, "{}- `{}` - {}", indent, f.name, f.description);
        }
        let _ = writeln!(md);
    }

    // Recurse into children
    for child in &node.children {
        md.push_str(&format_module_tree(child, depth + 1));
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_unix_paths() {
        let path = Path::new("C:\\Users\\test\\file.txt");
        assert_eq!(unix_path(path), "C:/Users/test/file.txt");
        assert_eq!(
            normalize_path_str("src\\graph\\mod.rs"),
            "src/graph/mod.rs"
        );
    }

    #[test]
    fn builds_vscode_uri_with_unix_path() {
        let path = Path::new("C:\\Users\\test\\file.txt");
        assert_eq!(
            to_vscode_file_uri(path),
            "vscode://file/C:/Users/test/file.txt"
        );
    }
}
