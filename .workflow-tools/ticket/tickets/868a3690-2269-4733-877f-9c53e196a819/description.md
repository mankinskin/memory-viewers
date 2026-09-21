# T5: Feature — Code Viewer with Source File References

## Problem

Log entries carry source file paths and line numbers, but the Leptos frontend has no way to view source code. The TS version has a right panel CodeViewer with Prism.js syntax highlighting, clickable source references on log entries, inline 5-line snippets in expanded entries, and full-file display with highlighted target lines.

## Reference: TS Implementation

### CodeViewer.tsx (viewer-api-frontend)
- **L6–14**: Props: `file, content, highlightLine?, placeholderMessage?, placeholderIcon?`
- **L16–25**: `getLanguage()` — extension-based detection (rs, ts, js, json, toml, yaml, md)
- **L60–66**: Prism.js highlighting: `Prism.highlight(content, grammar, language)`
- **L74–88**: Line rendering: numbered lines, gold highlight on target line (`rgba(255, 213, 0, 0.15)`)
- **L38–42**: Auto-scroll to highlighted line: `scrollIntoView({ behavior: 'instant', block: 'center' })`
- **L63–71**: Header: filename + language badge + line count

### LogEntryRow.tsx (log-viewer frontend)
- **L56–57**: `useSourceSnippet(entry.file, entry.source_line)` — fetches 5-line context
- **L59–64**: `handleLocationClick()` — calls `openSourceFile(file, line)` from store
- **L192**: Clickable button: `<button class="header-location">filename:line</button>` with LocationPin icon

### source.rs (log-viewer backend)
- **L103–189**: `GET /api/source/{*path}?line=N&context=N`
- Full file response: `{ path, content, language, total_lines }`
- Snippet response: `{ path, content, start_line, end_line, highlight_line, language }`
- **L27–93**: `fetch_content()` — local disk read or remote URL fetch via reqwest
- **L129**: `extract_snippet(&content, line, context)` 
- **L131**: `detect_language(&path)` by extension

### FileContentViewer.tsx (viewer-api-frontend)
- Wrapper around CodeViewer with `onRenderCustom` hook for markdown/custom rendering
- Falls back to CodeViewer if custom renderer returns null

## Design

### Step 1: syntect WASM integration

Add `syntect` as a dependency with the `default-syntaxes` and `default-themes` features. Use the `ClassedHTMLGenerator` to produce `<span class="...">` elements styled via CSS (avoids inline style overhead).

```toml
# Cargo.toml
[dependencies]
syntect = { version = "5", default-features = false, features = ["default-syntaxes", "default-themes", "html"] }
```

**Syntax highlighting function**:
```rust
use syntect::parsing::SyntaxSet;
use syntect::html::{ClassedHTMLGenerator, ClassStyle};

thread_local! {
    static SYNTAX_SET: SyntaxSet = SyntaxSet::load_defaults_newlines();
}

pub fn highlight_code(content: &str, language: &str) -> Vec<String> {
    SYNTAX_SET.with(|ss| {
        let syntax = ss.find_syntax_by_token(language)
            .unwrap_or_else(|| ss.find_syntax_plain_text());
        let mut generator = ClassedHTMLGenerator::new_with_class_style(
            syntax, ss, ClassStyle::Spaced
        );
        for line in LinesWithEndings::from(content) {
            generator.parse_html_for_line_which_includes_newline(line).ok();
        }
        // Split output by line for per-line rendering
        generator.finalize()
            .split('\n')
            .map(String::from)
            .collect()
    })
}
```

**OneDark CSS theme** — generate from syntect's theme or hand-write CSS classes:
```css
.code-viewer .keyword { color: #c678dd; }
.code-viewer .string { color: #98c379; }
.code-viewer .comment { color: var(--text-muted); }
.code-viewer .function { color: #61afef; }
.code-viewer .type { color: #e5c07b; }
.code-viewer .number { color: #d19a66; }
.code-viewer .operator { color: #56b6c2; }
.code-viewer .property { color: #e06c75; }
```

### Step 2: CodeViewer component

```rust
// src/components/code_viewer.rs

#[component]
pub fn CodeViewer(
    file: Signal<Option<String>>,
    content: Signal<String>,
    #[prop(optional)]
    highlight_line: Option<Signal<Option<usize>>>,
    #[prop(default = "Click a source file to view code")]
    placeholder_message: &'static str,
) -> impl IntoView {
    // Derive language from file extension
    let language = move || {
        file.get().as_deref()
            .and_then(detect_language_from_path)
            .unwrap_or("text")
    };
    
    // Highlighted lines (Vec<String> of HTML spans)
    let highlighted = move || {
        let code = content.get();
        if code.is_empty() { return vec![]; }
        highlight_code(&code, language())
    };
    
    // Scroll-to-line ref
    let highlight_ref = create_node_ref::<html::Div>();
    
    // Auto-scroll when highlight line changes
    create_effect(move |_| {
        if let Some(el) = highlight_ref.get() {
            el.scroll_into_view_with_scroll_into_view_options(
                web_sys::ScrollIntoViewOptions::new()
                    .behavior(web_sys::ScrollBehavior::Instant)
                    .block(web_sys::ScrollLogicalPosition::Center)
            );
        }
    });
    
    view! {
        <div class="code-viewer">
            <Show when=move || file.get().is_some() fallback=|| placeholder>
                <div class="code-header">
                    <span class="code-filename">{move || display_filename(&file.get())}</span>
                    <span class="code-language">{language}</span>
                    <span class="code-lines">{move || format!("{} lines", highlighted().len())}</span>
                </div>
                <div class="code-content">
                    <pre class="code-pre">
                        <For each=move || highlighted().into_iter().enumerate()
                             key=|(i, _)| *i
                             let:(i, line_html)>
                            {
                                let line_num = i + 1;
                                let is_hl = move || highlight_line
                                    .and_then(|s| s.get())
                                    .map_or(false, |l| l == line_num);
                                view! {
                                    <div class="code-line" class:highlight=is_hl
                                         node_ref=if is_hl() { Some(highlight_ref) } else { None }>
                                        <span class="line-number">{line_num}</span>
                                        <code inner_html=line_html />
                                    </div>
                                }
                            }
                        </For>
                    </pre>
                </div>
            </Show>
        </div>
    }
}
```

**CSS**:
```css
.code-viewer { display: flex; flex-direction: column; height: 100%; background: var(--bg-primary); }
.code-header {
    display: flex; gap: 8px; align-items: center;
    padding: 6px 12px; background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-color);
}
.code-filename { font-family: monospace; font-size: 13px; overflow: hidden; text-overflow: ellipsis; }
.code-language { font-size: 11px; text-transform: uppercase; color: var(--text-muted); padding: 1px 6px; border-radius: 3px; background: var(--bg-tertiary); }
.code-lines { margin-left: auto; font-size: 11px; color: var(--text-muted); }
.code-content { flex: 1; overflow: auto; }
.code-pre { margin: 0; font-family: monospace; font-size: 13px; line-height: 1.5; }
.code-line { display: flex; padding: 0 8px; }
.code-line.highlight { background: rgba(255, 213, 0, 0.15); }
.line-number { width: 50px; text-align: right; padding-right: 16px; color: var(--text-muted); user-select: none; flex-shrink: 0; }
.code-line code { flex: 1; white-space: pre; }
```

### Step 3: Right panel with ResizeHandle

Wire CodeViewer into the main layout as a collapsible right panel:

```rust
// app.rs — add to lv-main area
let code_panel_width = create_rw_signal(400.0_f64);
let code_panel_open = create_rw_signal(false);

view! {
    <main class="lv-main">
        <TabBar ... />
        <div class="lv-content-split">
            <div class="lv-view-container" style=move || {
                if code_panel_open.get() {
                    format!("flex: 1; min-width: 0;")
                } else {
                    "flex: 1;".to_string()
                }
            }>
                // Tab content
            </div>
            <Show when=move || code_panel_open.get()>
                <ResizeHandle
                    on_resize=Callback::new(move |delta| {
                        let new_w = (code_panel_width.get() - delta).clamp(200.0, 800.0);
                        code_panel_width.set(new_w);
                    })
                    direction="horizontal"
                    delta_sign=-1.0  // Right-anchored panel
                />
                <div class="lv-code-panel" style=move || format!("width: {}px", code_panel_width.get())>
                    <CodeViewer
                        file=store.code_viewer_file
                        content=store.code_viewer_content
                        highlight_line=Some(store.code_viewer_line)
                    />
                </div>
            </Show>
        </div>
    </main>
}
```

### Step 4: Store signals for code viewer

```rust
// store.rs — add signals
pub code_viewer_file: RwSignal<Option<String>>,
pub code_viewer_content: RwSignal<String>,
pub code_viewer_line: RwSignal<Option<usize>>,
```

**open_source_file()** action:
```rust
// actions.rs
pub async fn open_source_file(store: &Store, file: &str, line: Option<usize>) {
    store.code_viewer_file.set(Some(file.to_string()));
    store.code_viewer_line.set(line);
    
    // Fetch full file from backend
    match gloo_net::http::Request::get(&format!("/api/source/{}", file))
        .send().await
    {
        Ok(resp) if resp.ok() => {
            if let Ok(data) = resp.json::<SourceResponse>().await {
                store.code_viewer_content.set(data.content);
            }
        }
        _ => { /* handle error */ }
    }
}
```

### Step 5: Source reference buttons on log entries

Add clickable source location to log entry rows:

```rust
// In LogEntryRow component:
{entry.file.as_ref().map(|file| {
    let file = file.clone();
    let line = entry.source_line;
    view! {
        <button class="header-location"
                on:click=move |e| {
                    e.stop_propagation();
                    spawn_local(open_source_file(&store, &file, line));
                }
                title=format!("{}:{}", file, line.unwrap_or(0))
        >
            <LocationPinIcon size=8 />
            {format!("{}:{}", file.split(&['/', '\\']).last().unwrap_or(&file), 
                     line.unwrap_or(0))}
        </button>
    }
})}
```

### Step 6: Inline source snippets

**useSourceSnippet** equivalent — a Leptos resource that fetches a 5-line snippet:

```rust
pub fn use_source_snippet(
    file: Option<String>,
    line: Option<usize>,
    context: usize,  // default 5
) -> Resource<Option<SourceSnippet>> {
    create_resource(
        move || (file.clone(), line),
        move |(file, line)| async move {
            let (file, line) = (file?, line?);
            let url = format!("/api/source/{}?line={}&context={}", file, line, context);
            gloo_net::http::Request::get(&url)
                .send().await.ok()?
                .json::<SourceSnippet>().await.ok()
        }
    )
}
```

Render inline in expanded log entries:
```rust
// In expanded LogEntryRow:
{snippet.with(|s| s.as_ref().map(|snip| {
    view! {
        <div class="source-snippet">
            <div class="snippet-header">{&snip.path} L{snip.start_line}-{snip.end_line}</div>
            <CodeSnippet content=snip.content.clone()
                         language=snip.language.clone()
                         start_line=snip.start_line
                         highlight_line=snip.highlight_line />
        </div>
    }
}))}
```

### Step 7: FileContentViewer wrapper

Generic wrapper for future extensibility (markdown rendering in doc-viewer):

```rust
#[component]
pub fn FileContentViewer(
    file: Signal<Option<String>>,
    content: Signal<String>,
    highlight_line: Option<Signal<Option<usize>>>,
    /// Custom renderer — if returns Some(view), uses that instead of CodeViewer
    #[prop(optional)]
    on_render_custom: Option<Callback<(String, String), Option<View>>>,
) -> impl IntoView {
    move || {
        if let Some(renderer) = &on_render_custom {
            if let (Some(f), c) = (file.get(), content.get()) {
                if let Some(custom_view) = renderer.call((f, c)) {
                    return custom_view;
                }
            }
        }
        view! { <CodeViewer file=file content=content highlight_line=highlight_line /> }.into_view()
    }
}
```

## Files to Create

| File | Purpose |
|------|---------|
| `src/components/code_viewer.rs` | CodeViewer with syntect highlighting |
| `src/components/code_snippet.rs` | Inline snippet for expanded log entries |
| `src/components/file_content_viewer.rs` | Wrapper with custom renderer hook |

## Files to Modify

| File | Change |
|------|--------|
| `Cargo.toml` | Add `syntect` dependency (with `html` feature) |
| `src/store.rs` | Add `code_viewer_file`, `code_viewer_content`, `code_viewer_line` signals |
| `src/actions.rs` | Add `open_source_file()` async action |
| `src/app.rs` | Add right panel with CodeViewer + ResizeHandle |
| `src/components/log_entry_row.rs` (or equivalent) | Add clickable source reference buttons |
| `src/types.rs` | Add `SourceResponse`, `SourceSnippet` structs |
| `style.css` | CodeViewer, code-header, line-number, highlight, snippet, OneDark syntax theme |

## Acceptance Criteria

1. Right panel CodeViewer with syntect syntax highlighting (Rust, TS, JS, JSON, YAML, TOML, Markdown)
2. Line numbers (50px column), gold highlight on target line, auto-scroll to highlighted line
3. Header with filename, language badge, line count
4. ResizeHandle between main content and CodeViewer panel (reuse from T4)
5. Clickable source reference buttons on log entries (`filename:line` with location icon)
6. Click → GET `/api/source/{path}` → full file in right panel with highlighted line
7. Inline 5-line snippet in expanded log entries via `/api/source/{path}?line=N&context=5`
8. Remote source resolution (URL-based repos) handled by existing backend
9. FileContentViewer wrapper with custom renderer hook
10. CodeViewer designed for extraction to viewer-api-leptos (T6)
