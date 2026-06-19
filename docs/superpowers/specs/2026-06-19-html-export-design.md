# HTML Export — Design Spec

**Date**: 2026-06-19
**Branch**: N/A (to be created from main)
**Status**: approved

## 1. Overview

新增 Markdown → HTML 文件导出功能。生成完整、自包含的 HTML 文件（内嵌 CSS + 语法高亮），浏览器可直接打开。与 PDF 导出并列在 `export_engine` crate。

## 2. Architecture

### 2.1 New module: `html.rs`

```
crates/export_engine/src/html.rs
```

**Public API**:

```rust
pub fn generate_html(doc: &Document, config: &HtmlConfig) -> Result<String>
```

Returns a complete HTML document string (`<!DOCTYPE html>` through `</html>`).

### 2.2 Configuration

```rust
#[derive(Debug, Clone)]
pub struct HtmlConfig {
    /// <title> and top-level <h1> text
    pub title: String,
    /// syntect theme name for code highlighting (default "InspiredGitHub")
    pub syntax_theme: String,
    /// User-provided CSS override. When None, built-in default CSS is used.
    pub css: Option<String>,
}

impl Default for HtmlConfig {
    fn default() -> Self {
        Self {
            title: String::new(),
            syntax_theme: "InspiredGitHub".into(),
            css: None,
        }
    }
}
```

### 2.3 AST → HTML rendering

Follows the same traversal pattern as `serialize.rs` (Markdown serialization), but emits HTML tags.

**Block rendering**:

| Block | HTML output |
|-------|-------------|
| Heading(level=n) | `<h{n}>content</h{n}>` |
| Paragraph | `<p>content</p>` |
| CodeBlock(lang, code) | `<pre><code class="language-rust">highlighted spans</code></pre>` |
| List(ordered=true) | `<ol>{items}</ol>` |
| List(ordered=false) | `<ul>{items}</ul>` |
| ListItem | `<li>content{sub_items}</li>` |
| BlockQuote | `<blockquote>{blocks}</blockquote>` |
| ThematicBreak | `<hr>` |
| Table | `<table><thead>...</thead><tbody>...</tbody></table>` |
| HtmlBlock | raw output (pass-through) |

**Inline rendering**:

| Inline | HTML output |
|--------|-------------|
| Text(s) | HTML-escaped `s` |
| Emph(inlines) | `<em>content</em>` |
| Strong(inlines) | `<strong>content</strong>` |
| Code(s) | `<code>escaped(s)</code>` |
| Link{text, url, title} | `<a href="url" title="title">text</a>` |
| Image{alt, url, title} | `<img src="url" alt="alt" title="title">` |
| Html(s) | raw output |
| SoftBreak | `\n` (or `<br>` in paragraph context) |
| HardBreak | `<br>` |

### 2.4 Code highlighting

Reuses `crate::highlight::CodeHighlighter` (same as PDF). Each token becomes `<span style="color:#...;font-weight:...">token</span>`.

### 2.5 Built-in CSS

Default stylesheet (~80 lines):

- System font stack with CJK support
- Max-width 800px centered layout, responsive
- Code blocks: light gray background `#f5f5f5`, monospace font, padding, rounded corners
- Tables: full-width, collapsed borders, zebra stripes, header bold
- Blockquotes: left border `#2196F3`, light blue background
- Images: `max-width: 100%` for responsive scaling
- Headings: clear hierarchy (h1 2em, h2 1.5em, etc.)
- `@media print` — remove shadows, adjust margins
- Dark color scheme suitable for white paper / screen reading

## 3. Menu Integration

In `crates/zdown-app/src/menu.rs`, add after "导出 PDF...":

```rust
if ui.button("导出 HTML...").clicked() {
    trigger_export_html(state);
}
```

`trigger_export_html`:
1. Open file save dialog (filter: `*.html`)
2. Ensure `.html` extension
3. Call `export_engine::generate_html(doc, config)`
4. Write to file
5. Add to recent files

## 4. Error Handling

| Scenario | Behavior |
|----------|----------|
| AST traversal | No errors (pure string building) |
| Code highlighting fails | Fallback: plain `<pre><code>` without spans |
| File write fails | `tracing::error!` |

No new error variants needed — `generate_html` is infallible at the AST level. The `Result<String>` return type uses existing `Error` type only for symmetry with `generate_pdf`.

## 5. File Changes

| File | Change |
|------|--------|
| `crates/export_engine/src/html.rs` | **New** — HTML rendering + CSS + `generate_html` |
| `crates/export_engine/src/lib.rs` | Add `pub mod html;`, export `generate_html` and `HtmlConfig` |
| `crates/zdown-app/src/menu.rs` | Add "导出 HTML..." button + `trigger_export_html` |

No new dependencies required. Uses existing `syntect` and `document_model` crates.

## 6. Testing

### Unit tests (`html.rs`):
- `heading_h1_to_h6` — heading levels produce correct tags
- `paragraph_simple` — text paragraph → `<p>`
- `paragraph_with_inline_styles` — emph/strong/code/link
- `code_block_with_language` — `<pre><code class="language-rust">`
- `code_block_no_language` — `<pre><code>`
- `unordered_list` — `<ul><li>`
- `ordered_list` — `<ol><li>`
- `nested_list` — nested `<ul>`/`<ol>` in `<li>`
- `blockquote` — `<blockquote>`
- `thematic_break` — `<hr>`
- `table` — full table with header/rows
- `image` — `<img>` tag
- `hard_break` — `<br>`
- `html_block_passthrough` — raw HTML preserved
- `generate_html_contains_doctype` — full document structure
- `generate_html_with_custom_css` — user CSS appears in `<style>`
- `html_escape_special_chars` — `<>&"` are escaped
- `default_config` — HtmlConfig::default() values

### Integration tests (`html.rs` or `pdf.rs`-style):
- `generate_html_returns_non_empty` — smoke test
- `generate_html_with_syntax_highlighting` — code blocks have span styling
