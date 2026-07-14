//! MCP tool definitions for documentation management.
//!
//! This module is organized into:
//! - `agents`: Agent documentation management (guides, plans, bug reports, etc.)
//! - `crates`: Crate API documentation management (crates/*/agents/docs/)

pub mod agents;
pub mod crates;

use regex::Regex;
use thiserror::Error;

/// Result type for tool operations.
pub type ToolResult<T> = Result<T, ToolError>;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Document not found: {0}")]
    NotFound(String),
    #[error("Document already exists: {0}")]
    AlreadyExists(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Invalid regex: {0}")]
    InvalidRegex(String),
}

impl From<String> for ToolError {
    fn from(s: String) -> Self {
        ToolError::ParseError(s)
    }
}

impl From<regex::Error> for ToolError {
    fn from(e: regex::Error) -> Self {
        ToolError::InvalidRegex(e.to_string())
    }
}

/// Detail level for document reading
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetailLevel {
    /// Headers/outline only - just structure
    Outline,
    /// Metadata without body content (default)
    Summary,
    /// Full content including body
    Full,
}

/// Filter criteria for listing documents
#[derive(Debug, Default)]
pub struct ListFilter {
    pub tag: Option<String>,
    pub status: Option<crate::schema::PlanStatus>,
}

// Re-export main manager types (other types available via agents:: and crates::)
pub use agents::DocsManager;
pub use crates::CrateDocsManager;

/// Compile a case-insensitive regex from the query string.
///
/// Supports:
/// - Quoted strings (`"hello world"` or `'hello world'`) - treated as literal text
/// - Regex patterns (`graph|path`, `init.*interval`) - full regex syntax
/// - Backslash escaping for literal metacharacters (`\|`, `\.`, `\*`)
///
/// Returns None if query is empty, Err if regex is invalid.
pub fn compile_search_regex(query: &str) -> ToolResult<Option<Regex>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(None);
    }

    // Check for quoted literal strings
    let pattern = if (query.starts_with('"') && query.ends_with('"'))
        || (query.starts_with('\'') && query.ends_with('\''))
    {
        // Strip quotes and escape for literal matching
        let inner = &query[1..query.len() - 1];
        format!("(?i){}", regex::escape(inner))
    } else {
        // Treat as regex pattern
        format!("(?i){}", query)
    };

    Ok(Some(Regex::new(&pattern)?))
}

/// Check if the regex matches the given text.
/// Returns true if regex is None (empty query matches all).
pub fn regex_matches(
    text: &str,
    regex: &Option<Regex>,
) -> bool {
    match regex {
        Some(re) => re.is_match(text),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to test if query matches text
    fn matches(
        query: &str,
        text: &str,
    ) -> bool {
        let regex = compile_search_regex(query).unwrap();
        regex_matches(text, &regex)
    }

    // Helper to check if query compiles successfully
    fn compiles(query: &str) -> bool {
        compile_search_regex(query).is_ok()
    }

    #[test]
    fn test_empty_query_matches_all() {
        let regex = compile_search_regex("").unwrap();
        assert!(regex.is_none());
        assert!(regex_matches("anything", &regex));
        assert!(regex_matches("", &regex));

        // Whitespace-only also matches all
        let regex = compile_search_regex("   ").unwrap();
        assert!(regex.is_none());
    }

    #[test]
    fn test_simple_text_match() {
        assert!(matches("hello", "hello world"));
        assert!(matches("world", "hello world"));
        assert!(matches("HELLO", "hello world")); // case-insensitive
        assert!(!matches("foo", "hello world"));
    }

    #[test]
    fn test_regex_or_pattern() {
        assert!(matches("graph|path", "this is a graph"));
        assert!(matches("graph|path", "this is a path"));
        assert!(!matches("graph|path", "this is neither"));
    }

    #[test]
    fn test_regex_wildcard() {
        assert!(matches("init.*interval", "init some interval"));
        assert!(matches("init.*interval", "initialize the interval"));
        assert!(!matches("init.*interval", "interval init")); // order matters
    }

    #[test]
    fn test_quoted_literal_double() {
        // Quoted strings escape regex metacharacters
        assert!(matches("\"hello world\"", "say hello world to me"));
        assert!(!matches("\"hello world\"", "helloworld")); // space matters
        assert!(!matches("\"hello world\"", "hello  world")); // exact match
    }

    #[test]
    fn test_quoted_literal_single() {
        assert!(matches("'hello world'", "say hello world to me"));
        assert!(!matches("'hello world'", "helloworld"));
    }

    #[test]
    fn test_quoted_escapes_metacharacters() {
        // Pipe in quotes is literal, not OR
        assert!(matches("\"a|b\"", "a|b"));
        assert!(!matches("\"a|b\"", "a"));
        assert!(!matches("\"a|b\"", "b"));

        // Dot in quotes is literal, not any char
        assert!(matches("\"a.b\"", "a.b"));
        assert!(!matches("\"a.b\"", "aXb"));

        // Asterisk in quotes is literal
        assert!(matches("\"a*b\"", "a*b"));
        assert!(!matches("\"a*b\"", "aaaaab"));
    }

    #[test]
    fn test_backslash_escaping() {
        // Escape pipe to match literally
        assert!(matches(r"a\|b", "a|b"));
        assert!(!matches(r"a\|b", "a"));

        // Escape dot
        assert!(matches(r"file\.rs", "file.rs"));
        assert!(!matches(r"file\.rs", "fileXrs"));

        // Escape asterisk
        assert!(matches(r"a\*b", "a*b"));
    }

    #[test]
    fn test_invalid_regex_error() {
        // Unclosed group
        assert!(!compiles("(unclosed"));
        // Invalid escape
        assert!(!compiles(r"\"));
    }

    #[test]
    fn test_case_insensitive() {
        assert!(matches("GRAPH", "this is a graph"));
        assert!(matches("Graph", "this is a GRAPH"));
        assert!(matches("\"Hello World\"", "HELLO WORLD is here"));
    }

    #[test]
    fn test_edge_cases() {
        // Single character
        assert!(matches("a", "a"));
        assert!(!matches("a", "b"));

        // Query with only quotes (empty content)
        assert!(matches("\"\"", "anything")); // empty matches anywhere

        // Mismatched quotes treated as regex
        assert!(matches("\"hello", "\"hello world")); // starts with quote literal
        assert!(matches("hello\"", "say hello\" there"));

        // Single quote at start, double at end
        assert!(matches("'hello\"", "'hello\" world"));
    }

    #[test]
    fn test_whitespace_in_pattern() {
        // Unquoted whitespace: regex treats it normally
        assert!(matches("hello world", "hello world"));

        // Quoted preserves exact whitespace
        assert!(matches("\"hello  world\"", "hello  world"));
        assert!(!matches("\"hello  world\"", "hello world"));
    }

    #[test]
    fn test_special_regex_features() {
        // Character class
        assert!(matches("[abc]", "a"));
        assert!(matches("[abc]", "b"));
        assert!(!matches("[abc]", "d"));

        // Word boundary
        assert!(matches(r"\bword\b", "a word here"));
        assert!(!matches(r"\bword\b", "awordhere"));

        // Start/end anchors
        assert!(matches("^start", "start of line"));
        assert!(!matches("^start", "not start"));
        assert!(matches("end$", "at the end"));
        assert!(!matches("end$", "end not"));
    }
}
