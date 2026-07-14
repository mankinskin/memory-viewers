//! Document templates for each documentation type.

use crate::schema::{
    DocMetadata,
    DocType,
};

/// Generate document content from metadata using the appropriate template.
pub fn generate_document(meta: &DocMetadata) -> String {
    match meta.doc_type {
        DocType::Guide => generate_guide(meta),
        DocType::Plan => generate_plan(meta),
        DocType::Implemented => generate_implemented(meta),
        DocType::BugReport => generate_bug_report(meta),
        DocType::Analysis => generate_analysis(meta),
    }
}

fn format_frontmatter(meta: &DocMetadata) -> String {
    let tags_str = meta
        .tags
        .iter()
        .map(|t| format!("`#{}`", t))
        .collect::<Vec<_>>()
        .join(" ");

    let mut lines = vec![
        format!("---"),
        format!("tags: {}", tags_str),
        format!("summary: {}", meta.summary),
    ];

    if let Some(status) = &meta.status {
        lines.push(format!("status: {}", status.emoji()));
    }

    lines.push(format!("---"));
    lines.join("\n")
}

fn generate_guide(meta: &DocMetadata) -> String {
    format!(
        r#"{}

# {}

## Problem

<!-- What problem does this guide solve? When would you need this? -->


## Solution

<!-- Step-by-step solution or pattern -->


## Example

```rust
// Example code here
```

## Common Mistakes

<!-- What NOT to do -->

- 

## Related

<!-- Links to related guides, files, or documentation -->

- 
"#,
        format_frontmatter(meta),
        meta.title
    )
}

fn generate_plan(meta: &DocMetadata) -> String {
    format!(
        r#"{}

# {}

## Objective

<!-- One clear sentence: What are we building/fixing/changing? -->


## Context

### Files Affected

<!-- List all files that will be modified -->

- 

### Dependencies

<!-- What other code/systems does this touch? -->

- 

## Analysis

### Current State

<!-- How does it work now? What's the problem? -->


### Desired State

<!-- How should it work after changes? -->


## Execution Steps

<!-- Atomic, testable steps. Each step <5 min. -->

- [ ] Step 1: 
- [ ] Step 2: 
- [ ] Step 3: 

## Validation

<!-- How to verify the changes work -->

- [ ] Tests pass: `cargo test -p <crate>`
- [ ] 

## Risks

<!-- What could go wrong? -->

- 
"#,
        format_frontmatter(meta),
        meta.title
    )
}

fn generate_implemented(meta: &DocMetadata) -> String {
    format!(
        r#"{}

# {}

## Summary

<!-- 2-3 sentence summary of what was implemented -->


## Changes

<!-- Key changes made -->

| File | Change |
|------|--------|
| | |

## API

<!-- New or changed APIs (if applicable) -->

```rust
// Key types or functions
```

## Migration

<!-- How to update code using old APIs (if breaking change) -->


## Testing

<!-- How this was validated -->

- 
"#,
        format_frontmatter(meta),
        meta.title
    )
}

fn generate_bug_report(meta: &DocMetadata) -> String {
    format!(
        r#"{}

# {}

## Symptoms

<!-- What goes wrong? Error messages, panics, incorrect behavior -->


## Reproduction

<!-- Minimal steps to reproduce -->

1. 
2. 
3. 

## Root Cause

<!-- Why does this happen? -->


## Location

<!-- Where in the code is the bug? -->

- File: 
- Function: 
- Line: 

## Fix

<!-- How to fix it (or options if unclear) -->

### Option A

<!-- Preferred fix -->


### Option B (if applicable)

<!-- Alternative fix -->


## Verification

<!-- How to verify the fix works -->

- [ ] 
"#,
        format_frontmatter(meta),
        meta.title
    )
}

fn generate_analysis(meta: &DocMetadata) -> String {
    format!(
        r#"{}

# {}

## Overview

<!-- What is being analyzed and why? -->


## Findings

### Key Finding 1

<!-- Description -->


### Key Finding 2

<!-- Description -->


## Comparison (if applicable)

| Aspect | Option A | Option B |
|--------|----------|----------|
| | | |

## Conclusions

<!-- What should be done based on this analysis? -->


## References

<!-- Related files, docs, or external resources -->

- 
"#,
        format_frontmatter(meta),
        meta.title
    )
}

/// Generate INDEX.md content for a document type.
pub fn generate_index(
    doc_type: DocType,
    entries: &[crate::schema::IndexEntry],
) -> String {
    let header = match doc_type {
        DocType::Guide =>
            "# Guides Index\n\nHow-to guides and troubleshooting patterns.",
        DocType::Plan => "# Plans Index\n\nTask plans before implementation.",
        DocType::Implemented =>
            "# Implemented Index\n\nCompleted features and enhancements.",
        DocType::BugReport =>
            "# Bug Reports Index\n\nKnown issues and problem analyses.",
        DocType::Analysis =>
            "# Analysis Index\n\nAlgorithm analysis and comparisons.",
    };

    let mut content = String::from(header);
    content.push_str("\n\n");

    if doc_type == DocType::Plan {
        content.push_str("| Date | Status | File | Summary |\n");
        content.push_str("|------|--------|------|---------|\n");
        for entry in entries {
            let status = entry.status.map(|s| s.emoji()).unwrap_or("📋");
            content.push_str(&format!(
                "| {} | {} | [{}]({}) | {} |\n",
                format_date(&entry.date),
                status,
                entry.filename,
                entry.filename,
                entry.summary
            ));
        }
    } else {
        content.push_str("| Date | File | Summary |\n");
        content.push_str("|------|------|---------|\n");
        for entry in entries {
            content.push_str(&format!(
                "| {} | [{}]({}) | {} |\n",
                format_date(&entry.date),
                entry.filename,
                entry.filename,
                entry.summary
            ));
        }
    }

    content
}

/// Format YYYYMMDD as YYYY-MM-DD for display.
fn format_date(date: &str) -> String {
    if date.len() == 8 {
        format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8])
    } else {
        date.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_guide() {
        let meta = DocMetadata {
            doc_type: DocType::Guide,
            date: "20260131".to_string(),
            title: "Test Guide".to_string(),
            filename: "20260131_TEST_GUIDE.md".to_string(),
            tags: vec!["testing".to_string()],
            summary: "A test guide".to_string(),
            status: None,
        };
        let content = generate_document(&meta);
        assert!(content.contains("# Test Guide"));
        assert!(content.contains("tags: `#testing`"));
    }
}
