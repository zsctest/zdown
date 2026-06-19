//! HTML 导出：generate_html(doc, config) -> Result<String>。
//!
//! 生成自包含的完整 HTML 文档（内嵌 CSS + 语法高亮），浏览器可直接打开。

use std::sync::LazyLock;

use document_model::ast::{
    Block, BlockQuote, CodeBlock, Document, Heading, Inline, ListItem, Paragraph, Table,
};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::Result;

// ---------------------------------------------------------------------------
// lazy static — 仅初始化一次 syntect 资源
// ---------------------------------------------------------------------------

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME: LazyLock<Option<syntect::highlighting::Theme>> = LazyLock::new(|| {
    let theme_set = ThemeSet::load_defaults();
    theme_set.themes.get("InspiredGitHub").cloned()
});

// ---------------------------------------------------------------------------
// HtmlConfig
// ---------------------------------------------------------------------------

/// HTML 导出配置。
#[derive(Debug, Clone)]
pub struct HtmlConfig {
    /// `<title>` and top-level `<h1>` text (if any heading matches).
    pub title: String,
    /// syntect theme name for code highlighting (default `"InspiredGitHub"`).
    pub syntax_theme: String,
    /// User-provided CSS appended after built-in styles.
    /// When `None`, only built-in CSS is used.
    /// When `Some(s)`, `s` is appended so cascade rules apply.
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

// ---------------------------------------------------------------------------
// built-in CSS
// ---------------------------------------------------------------------------

/// Default stylesheet — system font stack with CJK, responsive, print-friendly.
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

// ---------------------------------------------------------------------------
// HTML escaping
// ---------------------------------------------------------------------------

/// Escape `<`, `>`, `&`, `"` for HTML text content and attributes.
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

// ---------------------------------------------------------------------------
// syntax highlight → HTML spans
// ---------------------------------------------------------------------------

/// Highlight code using syntect, producing `<span style="...">` elements.
fn highlight_code_to_html(code: &str, language: Option<&str>) -> String {
    let Some(theme) = THEME.as_ref() else {
        return escape_html(code);
    };
    let syntax = language
        .and_then(|lang| SYNTAX_SET.find_syntax_by_token(lang))
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut out = String::new();

    for line in code.lines() {
        let ranges: Vec<(syntect::highlighting::Style, &str)> =
            match highlighter.highlight_line(line, &SYNTAX_SET) {
                Ok(r) => r,
                Err(_) => {
                    highlighter = HighlightLines::new(syntax, theme);
                    vec![(syntect::highlighting::Style::default(), line)]
                }
            };
        for (syn_style, text) in &ranges {
            let fg = syn_style.foreground;
            let color = format!("#{:02x}{:02x}{:02x}", fg.r, fg.g, fg.b);
            let mut style_parts = vec![format!("color:{color}")];
            if syn_style.font_style.contains(FontStyle::BOLD) {
                style_parts.push("font-weight:bold".into());
            }
            if syn_style.font_style.contains(FontStyle::ITALIC) {
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

// ---------------------------------------------------------------------------
// inline rendering
// ---------------------------------------------------------------------------

#[allow(dead_code)]
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

// ---------------------------------------------------------------------------
// block rendering
// ---------------------------------------------------------------------------

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
        Block::Heading(h) => render_heading(out, h),
        Block::Paragraph(p) => render_paragraph(out, p),
        Block::CodeBlock(cb) => render_code_block(out, cb),
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
        Block::BlockQuote(bq) => render_blockquote(out, bq),
        Block::ThematicBreak => {
            out.push_str("<hr />");
        }
        Block::Table(t) => render_table(out, t),
        Block::HtmlBlock(s) => {
            out.push_str(s);
        }
    }
}

fn render_heading(out: &mut String, h: &Heading) {
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

fn render_paragraph(out: &mut String, p: &Paragraph) {
    out.push_str("<p>");
    for i in &p.inlines {
        render_inline(out, i);
    }
    out.push_str("</p>");
}

fn render_code_block(out: &mut String, cb: &CodeBlock) {
    out.push_str("<pre><code");
    if let Some(lang) = &cb.language {
        out.push_str(" class=\"language-");
        out.push_str(&escape_html(lang));
        out.push('"');
    }
    out.push('>');
    // Highlight if possible, otherwise plain escaped text.
    let highlighted = highlight_code_to_html(&cb.content, cb.language.as_deref());
    out.push_str(&highlighted);
    out.push_str("</code></pre>");
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

fn render_blockquote(out: &mut String, bq: &BlockQuote) {
    out.push_str("<blockquote>\n");
    for bws in &bq.blocks {
        render_block(out, &bws.block);
        out.push('\n');
    }
    out.push_str("</blockquote>");
}

fn render_table(out: &mut String, t: &Table) {
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

// ---------------------------------------------------------------------------
// public API
// ---------------------------------------------------------------------------

/// 将 `Document` 导出为完整 HTML 文档字符串（`<!DOCTYPE html>` 到 `</html>`）。
///
/// 生成的 HTML 自包含：内嵌默认 CSS 和语法高亮 inline style。
pub fn generate_html(doc: &Document, config: &HtmlConfig) -> Result<String> {
    let title = escape_html(&config.title);
    let mut css = BUILTIN_CSS.to_string();
    if let Some(custom) = &config.css {
        css.push_str("\n/* === 用户自定义样式 === */\n");
        css.push_str(custom);
    }
    let body = render_body(doc);

    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"UTF-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n<title>{title}</title>\n<style>\n{css}\n</style>\n</head>\n<body>\n{body}</body>\n</html>\n"
    );

    Ok(html)
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use document_model::ast::{BlockWithSpan, List, Span, TableCell};

    // ---- helpers ----

    fn bws(block: Block) -> BlockWithSpan {
        BlockWithSpan {
            block,
            span: Span {
                start_line: 0,
                end_line: 0,
            },
        }
    }

    fn doc_with_block(block: Block) -> Document {
        Document {
            blocks: vec![bws(block)],
        }
    }

    fn sample_doc() -> Document {
        doc_with_block(Block::Paragraph(Paragraph {
            inlines: vec![Inline::Text("hello".into())],
        }))
    }

    // ---- HtmlConfig ----

    #[test]
    fn default_config() {
        let config = HtmlConfig::default();
        assert_eq!(config.title, "");
        assert_eq!(config.syntax_theme, "InspiredGitHub");
        assert!(config.css.is_none());
    }

    // ---- html_escape ----

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(escape_html("<>&\""), "&lt;&gt;&amp;&quot;");
        assert_eq!(escape_html("hello"), "hello");
        assert_eq!(escape_html(""), "");
    }

    // ---- inline rendering ----

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
        let inlines = vec![
            Inline::Text("a".into()),
            Inline::HardBreak,
            Inline::Text("b".into()),
        ];
        assert_eq!(render_inlines(&inlines), "a<br />b");
    }

    #[test]
    fn inline_soft_break() {
        let inlines = vec![
            Inline::Text("a".into()),
            Inline::SoftBreak,
            Inline::Text("b".into()),
        ];
        assert_eq!(render_inlines(&inlines), "a\nb");
    }

    #[test]
    fn inline_html_passthrough() {
        let inlines = vec![Inline::Html("<span>x</span>".into())];
        assert_eq!(render_inlines(&inlines), "<span>x</span>");
    }

    // ---- block rendering ----

    #[test]
    fn heading_h1_to_h6() {
        // h1
        let doc = doc_with_block(Block::Heading(Heading {
            level: 1,
            inlines: vec![Inline::Text("一".into())],
        }));
        assert_eq!(render_body(&doc), "<h1>一</h1>\n");

        // h6
        let doc = doc_with_block(Block::Heading(Heading {
            level: 6,
            inlines: vec![Inline::Text("六".into())],
        }));
        assert_eq!(render_body(&doc), "<h6>六</h6>\n");
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
        assert!(
            html.contains("<pre><code class=\"language-rust\">"),
            "should have class: {html}"
        );
        assert!(
            html.contains("</code></pre>"),
            "should close code tags: {html}"
        );
    }

    #[test]
    fn code_block_no_language() {
        let doc = doc_with_block(Block::CodeBlock(CodeBlock {
            language: None,
            content: "plain\n".into(),
        }));
        let html = render_body(&doc);
        assert!(
            html.contains("<pre><code>"),
            "should have <pre><code>: {html}"
        );
    }

    #[test]
    fn unordered_list() {
        let doc = doc_with_block(Block::List(List {
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
        let doc = doc_with_block(Block::List(List {
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
        let doc = doc_with_block(Block::List(List {
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
        // Check the nested ul appears
        assert!(html.matches("<ul>").count() >= 2, "{html}");
    }

    #[test]
    fn blockquote() {
        let doc = Document {
            blocks: vec![BlockWithSpan {
                block: Block::BlockQuote(BlockQuote {
                    blocks: vec![bws(Block::Paragraph(Paragraph {
                        inlines: vec![Inline::Text("引用".into())],
                    }))],
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
                TableCell {
                    inlines: vec![Inline::Text("A".into())],
                },
                TableCell {
                    inlines: vec![Inline::Text("B".into())],
                },
            ],
            rows: vec![vec![
                TableCell {
                    inlines: vec![Inline::Text("1".into())],
                },
                TableCell {
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
    fn hard_break() {
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

    // ---- integration: generate_html ----

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
        // 自定义 CSS 是追加而非替换：内置样式仍然存在
        assert!(html.contains("font-family:"));
        assert!(html.contains("用户自定义样式"));
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
        assert!(
            html.contains("<span style="),
            "highlighting missing: {html}"
        );
    }
}
