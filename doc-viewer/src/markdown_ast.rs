//! Markdown AST parsing and JSON conversion.
//!
//! Uses the `markdown` crate to parse markdown into mdast (Markdown Abstract Syntax Tree)
//! and converts it to JSON for JQ queries.
//!
//! # Example Queries on Markdown AST
//!
//! ```text
//! # Find all level-2 headings
//! .content.children[] | select(.type == "heading" and .depth == 2)
//!
//! # Get all code blocks with a specific language
//! .content.children[] | select(.type == "code" and .lang == "rust")
//!
//! # Find paragraphs containing specific text
//! .content.children[] | select(.type == "paragraph") | select(.children[].value | contains("error"))
//!
//! # Extract all links
//! .. | select(.type? == "link") | {url, title}
//!
//! # Get list items
//! .. | select(.type? == "listItem")
//! ```

use markdown::{
    to_mdast,
    ParseOptions,
};
use serde_json::{
    json,
    Value,
};

/// Parse markdown content into a JSON AST.
pub fn parse_markdown_to_json(content: &str) -> Result<Value, String> {
    let ast = to_mdast(content, &ParseOptions::gfm())
        .map_err(|e| format!("Markdown parse error: {}", e))?;

    Ok(node_to_json(&ast))
}

/// Convert an mdast node to JSON value.
///
/// The `match` over the mdast variants is split across several helpers so no
/// single function exceeds the cyclomatic-complexity budget. Each helper
/// handles a category of nodes and delegates the remainder to the next.
fn node_to_json(node: &markdown::mdast::Node) -> Value {
    use markdown::mdast::Node;

    match node {
        Node::Root(root) => json!({
            "type": "root",
            "children": root.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::Heading(h) => json!({
            "type": "heading",
            "depth": h.depth,
            "children": h.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::Paragraph(p) => json!({
            "type": "paragraph",
            "children": p.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::Text(t) => json!({
            "type": "text",
            "value": t.value
        }),

        Node::Code(c) => json!({
            "type": "code",
            "lang": c.lang,
            "meta": c.meta,
            "value": c.value
        }),

        Node::InlineCode(ic) => json!({
            "type": "inlineCode",
            "value": ic.value
        }),

        Node::List(l) => json!({
            "type": "list",
            "ordered": l.ordered,
            "start": l.start,
            "spread": l.spread,
            "children": l.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::ListItem(li) => json!({
            "type": "listItem",
            "spread": li.spread,
            "checked": li.checked,
            "children": li.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::Blockquote(bq) => json!({
            "type": "blockquote",
            "children": bq.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        other => node_to_json_inline(other),
    }
}

/// Inline formatting nodes; delegates remaining variants to `node_to_json_refs`.
fn node_to_json_inline(node: &markdown::mdast::Node) -> Value {
    use markdown::mdast::Node;

    match node {
        Node::Link(link) => json!({
            "type": "link",
            "url": link.url,
            "title": link.title,
            "children": link.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::Image(img) => json!({
            "type": "image",
            "url": img.url,
            "title": img.title,
            "alt": img.alt
        }),

        Node::Emphasis(em) => json!({
            "type": "emphasis",
            "children": em.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::Strong(s) => json!({
            "type": "strong",
            "children": s.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::Delete(d) => json!({
            "type": "delete",
            "children": d.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::ThematicBreak(_) => json!({
            "type": "thematicBreak"
        }),

        Node::Break(_) => json!({
            "type": "break"
        }),

        Node::Html(html) => json!({
            "type": "html",
            "value": html.value
        }),

        other => node_to_json_refs(other),
    }
}

/// Table, footnote, and reference nodes; delegates the rest to `node_to_json_ext`.
fn node_to_json_refs(node: &markdown::mdast::Node) -> Value {
    use markdown::mdast::Node;

    match node {
        // Tables (GFM)
        Node::Table(t) => json!({
            "type": "table",
            "align": t.align.iter().map(|a| format!("{:?}", a).to_lowercase()).collect::<Vec<_>>(),
            "children": t.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::TableRow(tr) => json!({
            "type": "tableRow",
            "children": tr.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::TableCell(tc) => json!({
            "type": "tableCell",
            "children": tc.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        // Footnotes (GFM)
        Node::FootnoteDefinition(fd) => json!({
            "type": "footnoteDefinition",
            "identifier": fd.identifier,
            "label": fd.label,
            "children": fd.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::FootnoteReference(fr) => json!({
            "type": "footnoteReference",
            "identifier": fr.identifier,
            "label": fr.label
        }),

        // Definition (link references)
        Node::Definition(d) => json!({
            "type": "definition",
            "identifier": d.identifier,
            "label": d.label,
            "url": d.url,
            "title": d.title
        }),

        Node::LinkReference(lr) => json!({
            "type": "linkReference",
            "identifier": lr.identifier,
            "label": lr.label,
            "referenceKind": format!("{:?}", lr.reference_kind).to_lowercase(),
            "children": lr.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::ImageReference(ir) => json!({
            "type": "imageReference",
            "identifier": ir.identifier,
            "label": ir.label,
            "alt": ir.alt,
            "referenceKind": format!("{:?}", ir.reference_kind).to_lowercase()
        }),

        other => node_to_json_ext(other),
    }
}

/// Frontmatter, math, and MDX extension nodes.
fn node_to_json_ext(node: &markdown::mdast::Node) -> Value {
    use markdown::mdast::Node;

    match node {
        // YAML frontmatter
        Node::Yaml(y) => json!({
            "type": "yaml",
            "value": y.value
        }),

        // TOML frontmatter
        Node::Toml(t) => json!({
            "type": "toml",
            "value": t.value
        }),

        // Math (extension)
        Node::Math(m) => json!({
            "type": "math",
            "value": m.value,
            "meta": m.meta
        }),

        Node::InlineMath(im) => json!({
            "type": "inlineMath",
            "value": im.value
        }),

        // MDX (if enabled)
        Node::MdxJsxFlowElement(el) => json!({
            "type": "mdxJsxFlowElement",
            "name": el.name,
            "children": el.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::MdxJsxTextElement(el) => json!({
            "type": "mdxJsxTextElement",
            "name": el.name,
            "children": el.children.iter().map(node_to_json).collect::<Vec<_>>()
        }),

        Node::MdxFlowExpression(expr) => json!({
            "type": "mdxFlowExpression",
            "value": expr.value
        }),

        Node::MdxTextExpression(expr) => json!({
            "type": "mdxTextExpression",
            "value": expr.value
        }),

        Node::MdxjsEsm(esm) => json!({
            "type": "mdxjsEsm",
            "value": esm.value
        }),

        _ => json!({ "type": "unknown" }),
    }
}

/// Extract text content from a markdown AST node (flattens all text).
#[allow(dead_code)]
pub fn extract_text(content: &str) -> Result<String, String> {
    let ast = to_mdast(content, &ParseOptions::gfm())
        .map_err(|e| format!("Markdown parse error: {}", e))?;

    Ok(collect_text(&ast))
}

#[allow(dead_code)]
fn collect_text(node: &markdown::mdast::Node) -> String {
    use markdown::mdast::Node;

    match node {
        Node::Text(t) => t.value.clone(),
        Node::InlineCode(ic) => ic.value.clone(),
        Node::Code(c) => c.value.clone(),
        Node::Html(h) => h.value.clone(),
        Node::Root(r) => r
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Node::Heading(h) => h
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(""),
        Node::Paragraph(p) => p
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(""),
        Node::Blockquote(bq) => bq
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join("\n"),
        other => collect_text_rest(other),
    }
}

#[allow(dead_code)]
fn collect_text_rest(node: &markdown::mdast::Node) -> String {
    use markdown::mdast::Node;

    match node {
        Node::List(l) => l
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Node::ListItem(li) => li
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Node::Emphasis(em) => em
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(""),
        Node::Strong(s) => s
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(""),
        Node::Delete(d) => d
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(""),
        Node::Link(l) => l
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(""),
        other => collect_text_more(other),
    }
}

#[allow(dead_code)]
fn collect_text_more(node: &markdown::mdast::Node) -> String {
    use markdown::mdast::Node;

    match node {
        Node::Table(t) => t
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Node::TableRow(tr) => tr
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(" | "),
        Node::TableCell(tc) => tc
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(""),
        Node::FootnoteDefinition(fd) => fd
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Node::LinkReference(lr) => lr
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(""),
        Node::MdxJsxFlowElement(el) => el
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Node::MdxJsxTextElement(el) => el
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_markdown() {
        let md = "# Hello\n\nThis is a paragraph.";
        let json = parse_markdown_to_json(md).unwrap();

        assert_eq!(json["type"], "root");
        assert_eq!(json["children"][0]["type"], "heading");
        assert_eq!(json["children"][0]["depth"], 1);
        assert_eq!(json["children"][1]["type"], "paragraph");
    }

    #[test]
    fn test_parse_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let json = parse_markdown_to_json(md).unwrap();

        assert_eq!(json["children"][0]["type"], "code");
        assert_eq!(json["children"][0]["lang"], "rust");
        assert_eq!(json["children"][0]["value"], "fn main() {}");
    }

    #[test]
    fn test_parse_list() {
        let md = "- Item 1\n- Item 2\n- Item 3";
        let json = parse_markdown_to_json(md).unwrap();

        assert_eq!(json["children"][0]["type"], "list");
        assert_eq!(json["children"][0]["ordered"], false);
        assert_eq!(
            json["children"][0]["children"].as_array().unwrap().len(),
            3
        );
    }

    #[test]
    fn test_parse_link() {
        let md = "[Click here](https://example.com)";
        let json = parse_markdown_to_json(md).unwrap();

        let link = &json["children"][0]["children"][0];
        assert_eq!(link["type"], "link");
        assert_eq!(link["url"], "https://example.com");
    }

    #[test]
    fn test_parse_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let json = parse_markdown_to_json(md).unwrap();

        assert_eq!(json["children"][0]["type"], "table");
    }

    #[test]
    fn test_extract_text() {
        let md = "# Title\n\nSome **bold** and *italic* text.";
        let text = extract_text(md).unwrap();

        assert!(text.contains("Title"));
        assert!(text.contains("bold"));
        assert!(text.contains("italic"));
    }

    #[test]
    fn test_task_list() {
        let md = "- [x] Done\n- [ ] Todo";
        let json = parse_markdown_to_json(md).unwrap();

        let items = json["children"][0]["children"].as_array().unwrap();
        assert_eq!(items[0]["checked"], true);
        assert_eq!(items[1]["checked"], false);
    }
}
