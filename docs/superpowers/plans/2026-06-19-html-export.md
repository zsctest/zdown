# HTML 导出 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 实现 Markdown → 自包含 HTML 文件导出（内嵌 CSS + 语法高亮）

**架构：** 在 `export_engine` crate 新增 `html.rs` 模块，遵循与 `serialize.rs` 相同的 AST 遍历模式产生 HTML 标签，复用 `highlight::CodeHighlighter` 做代码着色

**技术栈：** Rust 2024, syntect, document_model, workspace (rfd)

---

### 任务 1：添加文件对话框

**文件：**
- 修改：`crates/workspace/src/dialog.rs`

- [ ] **步骤 1：添加 pick_save_file_html 函数**

```rust
/// 弹出 HTML 导出保存对话框。用户取消或环境不支持时返回 `None`。
pub fn pick_save_file_html() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("HTML", &["html", "htm"])
        .set_title("导出 HTML")
        .set_file_name("untitled.html")
        .save_file()
}
```

- [ ] **步骤 2：在 lib.rs 导出**

在 `crates/workspace/src/lib.rs` 的 `pub use dialog::{...}` 行添加 `pick_save_file_html`。

- [ ] **步骤 3：Commit**

```bash
git add crates/workspace/src/dialog.rs crates/workspace/src/lib.rs
git commit -m "feat(workspace): add pick_save_file_html dialog"
```

---

### 任务 2：实现 HtmlConfig

**文件：**
- 创建：`crates/export_engine/src/html.rs`

- [ ] **步骤 1：编写 HtmlConfig 及其 Default impl**

```rust
//! HTML 导出：generate_html(doc, config) -> Result<String>。
//!
//! 生成自包含的完整 HTML 文档，浏览器可直接打开。

use crate::highlight::CodeHighlighter;
use crate::Result;
use document_model::ast::{
    Block, BlockQuote, CodeBlock, Document, Heading, Inline, ListItem, Paragraph, Table,
};

/// HTML 导出配置。
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

- [ ] **步骤 2：编写 default_config 测试**

```rust
#[test]
fn default_config() {
    let config = HtmlConfig::default();
    assert_eq!(config.title, "");
    assert_eq!(config.syntax_theme, "InspiredGitHub");
    assert!(config.css.is_none());
}
```

- [ ] **步骤 3：运行测试验证通过**

```bash
cargo test -p export_engine -- html::tests::default_config
```

- [ ] **步骤 4：Commit**

```bash
git add crates/export_engine/src/html.rs
git commit -m "test(export_engine): add HtmlConfig with default test"
```

---

### 任务 3：实现 HTML 转义和内联渲染

**文件：**
- 修改：`crates/export_engine/src/html.rs`

- [ ] **步骤 1：编写 html_escape 函数测试**

```rust
#[test]
fn html_escape_special_chars() {
    assert_eq!(escape_html("<>&\""), "&lt;&gt;&amp;&quot;");
    assert_eq!(escape_html("hello"), "hello");
    assert_eq!(escape_html(""), "");
}
```

- [ ] **步骤 2：实现 html_escape**

```rust
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}
```

- [ ] **步骤 3：编写内联渲染测试**

```rust
#[test]
fn inline_text() {
    let inlines = vec![Inline::Text("hello".into())];
    assert_eq!(render_inlines(&inlines), "hello");
}

#[test]
fn inline_emph() {
    let inlines = vec![Inline::Emph(vec![Inline::Text("e".into())])];
    assert_eq!(render_inlines(&inlines), "<em>e</em>");
}

#[test]
fn inline_strong() {
    let inlines = vec![Inline::Strong(vec![Inline::Text("s".into())])];
    assert_eq!(render_inlines(&inlines), "<strong>s</strong>");
}

#[test]
fn inline_code() {
    let inlines = vec![Inline::Code("c".into())];
    assert_eq!(render_inlines(&inlines), "<code>c</code>");
}

#[test]
fn inline_link() {
    let inlines = vec![Inline::Link {
        text: vec![Inline::Text("click".into())],
        url: "https://example.com".into(),
        title: None,
    }];
    assert_eq!(
        render_inlines(&inlines),
        "<a href=\"https://example.com\">click</a>"
    );
}

#[test]
fn inline_link_with_title() {
    let inlines = vec![Inline::Link {
        text: vec![Inline::Text("t".into())],
        url: "/".into(),
        title: Some("tip".into()),
    }];
    assert_eq!(
        render_inlines(&inlines),
        "<a href=\"/\" title=\"tip\">t</a>"
    );
}

#[test]
fn inline_image() {
    let inlines = vec![Inline::Image {
        alt: "logo".into(),
        url: "logo.png".into(),
        title: None,
    }];
    assert_eq!(
        render_inlines(&inlines),
        "<img src=\"logo.png\" alt=\"logo\" />"
    );
}

#[test]
fn inline_hard_break() {
    let inlines = vec![Inline::Text("a".into()), Inline::HardBreak, Inline::Text("b".into())];
    assert_eq!(render_inlines(&inlines), "a<br />b");
}

#[test]
fn inline_soft_break() {
    let inlines = vec![Inline::Text("a".into()), Inline::SoftBreak, Inline::Text("b".into())];
    assert_eq!(render_inlines(&inlines), "a\nb");
}

#[test]
fn inline_html_passthrough() {
    let inlines = vec![Inline::Html("<span>x</span>".into())];
    assert_eq!(render_inlines(&inlines), "<span>x</span>");
}
```

- [ ] **步骤 4：实现 render_inlines 和 render_inline**

```rust
fn render_inlines(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        render_inline(&mut out, inline);
    }
    out
}

fn render_inline(out: &mut String, inline: &Inline) {
    match inline {
        Inline::Text(s) => {
            out.push_str(&escape_html(s));
        }
        Inline::Emph(inner) => {
            out.push_str("<em>");
            for i in inner {
                render_inline(out, i);
            }
            out.push_str("</em>");
        }
        Inline::Strong(inner) => {
            out.push_str("<strong>");
            for i in inner {
                render_inline(out, i);
            }
            out.push_str("</strong>");
        }
        Inline::Code(s) => {
            out.push_str("<code>");
            out.push_str(&escape_html(s));
            out.push_str("</code>");
        }
        Inline::Link { text, url, title } => {
            out.push_str("<a href=\"");
            out.push_str(&escape_html(url));
            out.push('"');
            if let Some(t) = title {
                out.push_str(" title=\"");
                out.push_str(&escape_html(t));
                out.push('"');
            }
            out.push('>');
            for i in text {
                render_inline(out, i);
            }
            out.push_str("</a>");
        }
        Inline::Image { alt, url, title } => {
            out.push_str("<img src=\"");
            out.push_str(&escape_html(url));
            out.push_str("\" alt=\"");
            out.push_str(&escape_html(alt));
            out.push('"');
            if let Some(t) = title {
                out.push_str(" title=\"");
                out.push_str(&escape_html(t));
                out.push('"');
            }
            out.push_str(" />");
        }
        Inline::Html(s) => out.push_str(s),
        Inline::SoftBreak => out.push('\n'),
        Inline::HardBreak => out.push_str("<br />"),
    }
}
```

- [ ] **步骤 5：运行测试验证通过**

```bash
cargo test -p export_engine -- html::tests::
```

- [ ] **步骤 6：Commit**

```bash
git add crates/export_engine/src/html.rs
git commit -m "feat(export_engine): add inline HTML rendering with html_escape"
```

---

### 任务 4：实现块级渲染

**文件：**
- 修改：`crates/export_engine/src/html.rs`

- [ ] **步骤 1：编写块级渲染测试**

```rust
#[test]
fn heading_h1() {
    let doc = doc_with_block(Block::Heading(Heading {
        level: 1,
        inlines: vec![Inline::Text("标题".into())],
    }));
    assert_eq!(render_body(&doc), "<h1>标题</h1>\n");
}

#[test]
fn heading_h3() {
    let doc = doc_with_block(Block::Heading(Heading {
        level: 3,
        inlines: vec![Inline::Text("三".into())],
    }));
    assert_eq!(render_body(&doc), "<h3>三</h3>\n");
}

#[test]
fn paragraph_simple() {
    let doc = doc_with_block(Block::Paragraph(Paragraph {
        inlines: vec![Inline::Text("hello".into())],
    }));
    assert_eq!(render_body(&doc), "<p>hello</p>\n");
}

#[test]
fn paragraph_with_inline_styles() {
    let doc = doc_with_block(Block::Paragraph(Paragraph {
        inlines: vec![
            Inline::Strong(vec![Inline::Text("bold".into())]),
            Inline::Text(" and ".into()),
            Inline::Emph(vec![Inline::Text("em".into())]),
        ],
    }));
    assert_eq!(
        render_body(&doc),
        "<p><strong>bold</strong> and <em>em</em></p>\n"
    );
}

#[test]
fn code_block_with_language() {
    let doc = doc_with_block(Block::CodeBlock(CodeBlock {
        language: Some("rust".into()),
        content: "fn x() {}\n".into(),
    }));
    let html = render_body(&doc);
    assert!(html.contains("<pre>"), "should have <pre>: {html}");
}

#[test]
fn code_block_no_language() {
    let doc = doc_with_block(Block::CodeBlock(CodeBlock {
        language: None,
        content: "plain\n".into(),
    }));
    let html = render_body(&doc);
    assert!(html.contains("<pre>"), "should have <pre>");
}

#[test]
fn unordered_list() {
    let doc = doc_with_block(Block::List(document_model::ast::List {
        ordered: false,
        start: 0,
        items: vec![
            ListItem {
                inlines: vec![Inline::Text("a".into())],
                sub_items: vec![],
            },
            ListItem {
                inlines: vec![Inline::Text("b".into())],
                sub_items: vec![],
            },
        ],
    }));
    assert_eq!(render_body(&doc), "<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n");
}

#[test]
fn ordered_list() {
    let doc = doc_with_block(Block::List(document_model::ast::List {
        ordered: true,
        start: 1,
        items: vec![ListItem {
            inlines: vec![Inline::Text("一".into())],
            sub_items: vec![],
        }],
    }));
    assert_eq!(render_body(&doc), "<ol>\n<li>一</li>\n</ol>\n");
}

#[test]
fn nested_list() {
    let doc = doc_with_block(Block::List(document_model::ast::List {
        ordered: false,
        start: 0,
        items: vec![ListItem {
            inlines: vec![Inline::Text("顶".into())],
            sub_items: vec![ListItem {
                inlines: vec![Inline::Text("嵌".into())],
                sub_items: vec![],
            }],
        }],
    }));
    let html = render_body(&doc);
    assert!(html.contains("<li>顶"));
    assert!(html.contains("<li>嵌"));
    assert!(html.contains("<ul>"));
}

#[test]
fn blockquote() {
    let doc = Document {
        blocks: vec![BlockWithSpan {
            block: Block::BlockQuote(BlockQuote {
                blocks: vec![BlockWithSpan {
                    block: Block::Paragraph(Paragraph {
                        inlines: vec![Inline::Text("引用".into())],
                    }),
                    span: Span {
                        start_line: 0,
                        end_line: 0,
                    },
                }],
            }),
            span: Span {
                start_line: 0,
                end_line: 0,
            },
        }],
    };
    let html = render_body(&doc);
    assert!(html.contains("<blockquote>"));
    assert!(html.contains("<p>引用</p>"));
}

#[test]
fn thematic_break() {
    let doc = doc_with_block(Block::ThematicBreak);
    assert_eq!(render_body(&doc), "<hr />\n");
}

#[test]
fn table_basic() {
    let doc = doc_with_block(Block::Table(Table {
        header: vec![
            document_model::ast::TableCell {
                inlines: vec![Inline::Text("A".into())],
            },
            document_model::ast::TableCell {
                inlines: vec![Inline::Text("B".into())],
            },
        ],
        rows: vec![vec![
            document_model::ast::TableCell {
                inlines: vec![Inline::Text("1".into())],
            },
            document_model::ast::TableCell {
                inlines: vec![Inline::Text("2".into())],
            },
        ]],
        alignments: vec![None, None],
    }));
    let html = render_body(&doc);
    assert!(html.contains("<table>"));
    assert!(html.contains("<thead>"));
    assert!(html.contains("<tbody>"));
    assert!(html.contains("<th>A</th>"));
    assert!(html.contains("<td>1</td>"));
}

#[test]
fn image_block() {
    let doc = doc_with_block(Block::Paragraph(Paragraph {
        inlines: vec![Inline::Image {
            alt: "pic".into(),
            url: "pic.png".into(),
            title: None,
        }],
    }));
    let html = render_body(&doc);
    assert!(html.contains("<img"));
    assert!(html.contains("src=\"pic.png\""));
}

#[test]
fn hard_break_in_paragraph() {
    let doc = doc_with_block(Block::Paragraph(Paragraph {
        inlines: vec![
            Inline::Text("line1".into()),
            Inline::HardBreak,
            Inline::Text("line2".into()),
        ],
    }));
    let html = render_body(&doc);
    assert!(html.contains("<br />"));
}

#[test]
fn html_block_passthrough() {
    let doc = doc_with_block(Block::HtmlBlock("<div class=\"x\">raw</div>".into()));
    let html = render_body(&doc);
    assert_eq!(html, "<div class=\"x\">raw</div>\n");
}
```

- [ ] **步骤 2：实现 render_block_body / render_body / render_list_items**

```rust
fn render_body(doc: &Document) -> String {
    let mut out = String::new();
    for bws in &doc.blocks {
        render_block(&mut out, &bws.block);
        out.push('\n');
    }
    out
}

fn render_block(out: &mut String, block: &Block) {
    match block {
        Block::Heading(h) => {
            let tag = format!("h{}", h.level);
            out.push('<');
            out.push_str(&tag);
            out.push('>');
            for i in &h.inlines {
                render_inline(out, i);
            }
            out.push_str("</");
            out.push_str(&tag);
            out.push('>');
        }
        Block::Paragraph(p) => {
            out.push_str("<p>");
            for i in &p.inlines {
                render_inline(out, i);
            }
            out.push_str("</p>");
        }
        Block::CodeBlock(cb) => {
            out.push_str("<pre><code");
            if let Some(lang) = &cb.language {
                out.push_str(" class=\"language-");
                out.push_str(&escape_html(lang));
                out.push('"');
            }
            out.push('>');
            if let Some(highlighter) = HIGHLIGHTER.as_ref() {
                let lang = cb.language.as_deref();
                let lines = highlighter.highlight(&cb.content, lang);
                for line in &lines {
                    for (_syn_style, text) in line {
                        // TODO: span styling in task 5
                        out.push_str(&escape_html(text));
                    }
                    out.push('\n');
                }
            } else {
                out.push_str(&escape_html(&cb.content));
            }
            out.push_str("</code></pre>");
        }
        Block::List(l) => {
            let tag = if l.ordered { "ol" } else { "ul" };
            out.push('<');
            out.push_str(tag);
            out.push('>');
            out.push('\n');
            render_list_items(out, &l.items);
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
        Block::BlockQuote(bq) => {
            out.push_str("<blockquote>\n");
            for bws in &bq.blocks {
                render_block(out, &bws.block);
                out.push('\n');
            }
            out.push_str("</blockquote>");
        }
        Block::ThematicBreak => {
            out.push_str("<hr />");
        }
        Block::Table(t) => {
            out.push_str("<table>\n<thead>\n<tr>");
            for cell in &t.header {
                out.push_str("<th>");
                for i in &cell.inlines {
                    render_inline(out, i);
                }
                out.push_str("</th>");
            }
            out.push_str("</tr>\n</thead>\n<tbody>\n");
            for row in &t.rows {
                out.push_str("<tr>");
                for cell in row {
                    out.push_str("<td>");
                    for i in &cell.inlines {
                        render_inline(out, i);
                    }
                    out.push_str("</td>");
                }
                out.push_str("</tr>\n");
            }
            out.push_str("</tbody>\n</table>");
        }
        Block::HtmlBlock(s) => {
            out.push_str(s);
        }
    }
}

fn render_list_items(out: &mut String, items: &[ListItem]) {
    for item in items {
        out.push_str("<li>");
        for i in &item.inlines {
            render_inline(out, i);
        }
        if !item.sub_items.is_empty() {
            out.push('\n');
            out.push_str("<ul>\n");
            render_list_items(out, &item.sub_items);
            out.push_str("</ul>\n");
        }
        out.push_str("</li>");
        out.push('\n');
    }
}
```

- [ ] **步骤 3：运行测试验证通过**

```bash
cargo test -p export_engine -- html::tests::
```

- [ ] **步骤 4：Commit**

```bash
git add crates/export_engine/src/html.rs
git commit -m "feat(export_engine): add block-level HTML rendering"
```

---

### 任务 5：内联语法高亮 span 和 generate_html

**文件：**
- 修改：`crates/export_engine/src/html.rs`

- [ ] **步骤 1：实现 syntax style → CSS color 映射**

高亮时将 syntect style 映射为 `<span style="color:#...">`：

```rust
use std::sync::LazyLock;

static HIGHLIGHTER: LazyLock<Option<CodeHighlighter>> =
    LazyLock::new(|| CodeHighlighter::new("InspiredGitHub"));

fn highlight_code_to_html(code: &str, language: Option<&str>) -> String {
    let Some(highlighter) = HIGHLIGHTER.as_ref() else {
        return escape_html(code);
    };
    let lines = highlighter.highlight(code, language);
    let mut out = String::new();
    for line in &lines {
        for (syn_style, text) in line {
            let color = format!("#{:02x}{:02x}{:02x}", syn_style.foreground.r, syn_style.foreground.g, syn_style.foreground.b);
            let mut style_parts = vec![format!("color:{}", color)];
            if syn_style.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
                style_parts.push("font-weight:bold".into());
            }
            if syn_style.font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
                style_parts.push("font-style:italic".into());
            }
            out.push_str("<span style=\"");
            out.push_str(&style_parts.join(";"));
            out.push_str("\">");
            out.push_str(&escape_html(text));
            out.push_str("</span>");
        }
        out.push('\n');
    }
    out
}
```

- [ ] **步骤 2：编写高亮测试**

```rust
#[test]
fn code_block_highlight_has_spans() {
    let doc = doc_with_block(Block::CodeBlock(CodeBlock {
        language: Some("rust".into()),
        content: "fn main() {}\n".into(),
    }));
    let html = render_body(&doc);
    assert!(html.contains("<span style="), "highlight should produce spans: {html}");
}
```

- [ ] **步骤 3：实现 generate_html**

```rust
/// CSS for the built-in default stylesheet.
const BUILTIN_CSS: &str = r#"*{margin:0;padding:0;box-sizing:border-box}
body{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI","PingFang SC","Hiragino Sans GB","Microsoft YaHei","Noto Sans CJK SC",sans-serif;font-size:17px;line-height:1.65;color:#333;max-width:800px;margin:0 auto;padding:2rem 1.5rem}
h1{font-size:2em;font-weight:700;margin:0.7em 0 0.3em;border-bottom:2px solid #eee;padding-bottom:0.2em}
h2{font-size:1.5em;font-weight:700;margin:0.75em 0 0.3em}
h3{font-size:1.25em;font-weight:600;margin:0.7em 0 0.25em}
h4{font-size:1.1em;font-weight:600;margin:0.6em 0 0.2em}
h5{font-size:1em;font-weight:600;margin:0.5em 0 0.15em}
h6{font-size:0.9em;font-weight:600;color:#666;margin:0.5em 0 0.15em}
p{margin:0.6em 0}
pre{background:#f5f5f5;border-radius:6px;padding:1em;overflow-x:auto;margin:0.8em 0;font-size:0.85em;line-height:1.5}
pre code{font-family:"Fira Code","Cascadia Code","JetBrains Mono","Source Code Pro",Consolas,monospace;font-size:0.95em}
code{font-family:"Fira Code","Cascadia Code","JetBrains Mono","Source Code Pro",Consolas,monospace;background:#f5f5f5;padding:0.15em 0.35em;border-radius:3px;font-size:0.9em}
pre code{background:transparent;padding:0;border-radius:0;font-size:inherit}
table{width:100%;border-collapse:collapse;margin:0.8em 0}
th,td{border:1px solid #ddd;padding:0.5em 0.75em;text-align:left}
th{background:#f5f5f5;font-weight:700}
tr:nth-child(even){background:#fafafa}
blockquote{border-left:4px solid #2196F3;background:#e3f2fd;padding:0.6em 1em;margin:0.8em 0;color:#333}
blockquote p{margin:0.3em 0}
img{max-width:100%;height:auto}
hr{border:none;border-top:1px solid #ddd;margin:1.5em 0}
ul,ol{padding-left:2em;margin:0.6em 0}
li{margin:0.25em 0}
a{color:#1976D2;text-decoration:none}
a:hover{text-decoration:underline}
@media print{body{max-width:none;padding:1cm;font-size:12pt}pre{background:#f9f9f9}blockquote{background:#f9f9f9}}"#;

/// 将 Document 导出为完整 HTML 文档字符串。
pub fn generate_html(doc: &Document, config: &HtmlConfig) -> Result<String> {
    let title = escape_html(&config.title);
    let css = config.css.as_deref().unwrap_or(BUILTIN_CSS);
    let body = render_body(doc);

    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"UTF-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n<title>{title}</title>\n<style>\n{css}\n</style>\n</head>\n<body>\n{body}</body>\n</html>\n"
    );

    Ok(html)
}
```

- [ ] **步骤 4：编写 generate_html 集成测试**

```rust
#[test]
fn generate_html_contains_doctype() {
    let doc = sample_doc();
    let config = HtmlConfig::default();
    let html = generate_html(&doc, &config).expect("generate_html");
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("</html>"));
}

#[test]
fn generate_html_with_custom_css() {
    let doc = sample_doc();
    let config = HtmlConfig {
        css: Some("body{color:red}".into()),
        ..Default::default()
    };
    let html = generate_html(&doc, &config).expect("generate_html");
    assert!(html.contains("body{color:red}"));
    assert!(!html.contains(BUILTIN_CSS)); // custom CSS replaces default
}

#[test]
fn generate_html_contains_default_css() {
    let doc = sample_doc();
    let config = HtmlConfig::default();
    let html = generate_html(&doc, &config).expect("generate_html");
    assert!(html.contains("font-family:"));
}

#[test]
fn generate_html_returns_non_empty() {
    let doc = sample_doc();
    let config = HtmlConfig::default();
    let html = generate_html(&doc, &config).expect("generate_html");
    assert!(!html.is_empty());
}

#[test]
fn generate_html_with_syntax_highlighting() {
    let doc = doc_with_block(Block::CodeBlock(CodeBlock {
        language: Some("rust".into()),
        content: "let x = 1;\n".into(),
    }));
    let config = HtmlConfig::default();
    let html = generate_html(&doc, &config).expect("generate_html");
    assert!(html.contains("<span style="), "highlighting missing: {html}");
}
```

- [ ] **步骤 5：运行全部测试验证通过**

```bash
cargo test -p export_engine -- html::
```

- [ ] **步骤 6：Commit**

```bash
git add crates/export_engine/src/html.rs
git commit -m "feat(export_engine): add generate_html with syntax highlighting and built-in CSS"
```

---

### 任务 6：注册模块并导出

**文件：**
- 修改：`crates/export_engine/src/lib.rs`

- [ ] **步骤 1：添加 pub mod html; 和 pub use**

```rust
pub mod html;

pub use html::{generate_html, HtmlConfig};
```

- [ ] **步骤 2：确认编译通过**

```bash
cargo build -p export_engine
```

- [ ] **步骤 3：Commit**

```bash
git add crates/export_engine/src/lib.rs
git commit -m "feat(export_engine): export html module and HtmlConfig"
```

---

### 任务 7：菜单集成

**文件：**
- 修改：`crates/zdown-app/src/menu.rs`

- [ ] **步骤 1：添加 trigger_export_html 函数（在 trigger_export_pdf 旁边）**

```rust
fn trigger_export_html(state: &mut EditorState) {
    if let Some(mut path) = workspace::pick_save_file_html() {
        if path.extension().is_none_or(|e| e != "html" && e != "htm") {
            path.set_extension("html");
        }
        let config = export_engine::HtmlConfig {
            title: state
                .current_path
                .as_ref()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            ..Default::default()
        };
        let doc = state.current_doc();
        match export_engine::generate_html(&doc, &config) {
            Ok(html_str) => {
                if let Err(e) = std::fs::write(&path, &html_str) {
                    tracing::error!("HTML 写入失败: {e}");
                } else {
                    tracing::info!("HTML 导出成功: {}", path.display());
                    state.recent.add(path);
                }
            }
            Err(e) => {
                tracing::error!("HTML 生成失败: {e}");
            }
        }
    }
}
```

- [ ] **步骤 2：在菜单中添加按钮（在 PDF 导出行之后）**

```rust
// 在 "if ui.button("导出 PDF...").clicked()" 之后添加：
if ui.button("导出 HTML...").clicked() {
    trigger_export_html(state);
}
```

- [ ] **步骤 3：确认编译通过**

```bash
cargo build -p zdown-app
```

- [ ] **步骤 4：Commit**

```bash
git add crates/zdown-app/src/menu.rs
git commit -m "feat(zdown-app): add HTML export menu integration"
```

---

### 任务 8：全量测试和 clippy

- [ ] **步骤 1：运行全量测试**

```bash
cargo test --workspace
```

- [ ] **步骤 2：运行 clippy**

```bash
cargo clippy --workspace -- -D warnings
```

- [ ] **步骤 3：运行 fmt**

```bash
cargo fmt -- --check
```

- [ ] **步骤 4：Commit（如有修复）**

---
