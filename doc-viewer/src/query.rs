//! JQ-style query language for filtering and querying documentation.
//!
//! Re-exports the shared query engine from `viewer_api::query` and provides
//! doc-viewer-specific tests.

// Re-export everything from the shared query module
pub use viewer_api::query::*;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_compile_simple_filter() {
        let filter = JqFilter::compile(".");
        assert!(filter.is_ok());
    }

    #[test]
    fn test_compile_invalid_filter() {
        let filter = JqFilter::compile("[invalid");
        assert!(filter.is_err());
    }

    #[test]
    fn test_filter_select() {
        let filter = JqFilter::compile("select(.level == \"ERROR\")").unwrap();

        let error_doc = json!({"level": "ERROR", "title": "Bug"});
        let info_doc = json!({"level": "INFO", "title": "Guide"});

        assert!(filter.matches(&error_doc));
        assert!(!filter.matches(&info_doc));
    }

    #[test]
    fn test_filter_contains() {
        // Note: contains() is case-sensitive in jq
        let filter =
            JqFilter::compile("select(.title | contains(\"search\"))").unwrap();

        let match_doc = json!({"title": "search guide"});
        let no_match = json!({"title": "Other guide"});

        assert!(filter.matches(&match_doc));
        assert!(!filter.matches(&no_match));
    }

    #[test]
    fn test_filter_case_insensitive() {
        // Use test() with "i" flag for case-insensitive matching
        let filter =
            JqFilter::compile("select(.title | test(\"search\"; \"i\"))")
                .unwrap();

        let match_upper = json!({"title": "Search Guide"});
        let match_lower = json!({"title": "search guide"});
        let no_match = json!({"title": "Other guide"});

        assert!(filter.matches(&match_upper));
        assert!(filter.matches(&match_lower));
        assert!(!filter.matches(&no_match));
    }

    #[test]
    fn test_filter_array_any() {
        let filter =
            JqFilter::compile("select(.tags | any(. == \"testing\"))").unwrap();

        let match_doc = json!({"tags": ["testing", "debug"]});
        let no_match = json!({"tags": ["production"]});

        assert!(filter.matches(&match_doc));
        assert!(!filter.matches(&no_match));
    }

    #[test]
    fn test_transform_values() {
        let docs = vec![
            json!({"title": "Doc A", "date": "20250101"}),
            json!({"title": "Doc B", "date": "20250102"}),
        ];

        let results = transform_values(docs.iter(), "{title}").unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0], json!({"title": "Doc A"}));
        assert_eq!(results[1], json!({"title": "Doc B"}));
    }

    #[test]
    fn test_filter_values() {
        let docs = vec![
            json!({"doc_type": "guide", "title": "Guide"}),
            json!({"doc_type": "plan", "title": "Plan"}),
            json!({"doc_type": "guide", "title": "Another Guide"}),
        ];

        let results =
            filter_values(docs.iter(), "select(.doc_type == \"guide\")")
                .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["title"], "Guide");
        assert_eq!(results[1]["title"], "Another Guide");
    }

    #[test]
    fn test_date_comparison() {
        let filter =
            JqFilter::compile("select(.date >= \"20250201\")").unwrap();

        let after = json!({"date": "20250215"});
        let before = json!({"date": "20250115"});

        assert!(filter.matches(&after));
        assert!(!filter.matches(&before));
    }
}
