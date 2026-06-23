# HTML 内嵌渲染 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 新建 `crates/html_renderer` crate，使用 html5ever 解析 Markdown 中的内嵌 HTML，渲染为 egui 富文本/块级组件。

**架构：** html5ever 解析 HTML 片段为 DOM 树 → 转换为 `HtmlNode` AST → `inline.rs` 渲染内联标签为 `RichText` / `block.rs` 渲染块级标签为 egui `Frame`/`Grid`。CSS 内联样式由手写 `css.rs` 解析并映射到 egui API。

**技术栈：** html5ever 0.27 / markup5ever 0.12 / egui 0.34

---

## 文件结构

| 操作 | 文件路径 | 职责 |
|---|---|---|
| 创建 | `crates/html_renderer/Cargo.toml` | 依赖声明 |
| 创建 | `crates/html_renderer/src/lib.rs` | 公共 API |
| 创建 | `crates/html_renderer/src/css.rs` | 内联 style 解析 |
| 创建 | `crates/html_renderer/src/parser.rs` | html5ever → HtmlNode 树 |
| 创建 | `crates/html_renderer/src/inline.rs` | 内联标签 → egui |
| 创建 | `crates/html_renderer/src/block.rs` | 块级标签 → egui |
| 修改 | `Cargo.toml` (root) | 添加 workspace deps + member |
| 修改 | `crates/markdown_renderer/Cargo.toml` | 加 html_renderer 依赖 |
| 修改 | `crates/markdown_renderer/src/render.rs` | 替换 HtmlBlock/Inline::Html |

---

### 任务 1：工作区配置 + html_renderer crate 骨架

**文件：**
- 修改：`Cargo.toml`（根）
- 创建：`crates/html_renderer/Cargo.toml`
- 创建：`crates/html_renderer/src/lib.rs`（stub）

- [ ] **步骤 1：添加 workspace 依赖声明和 member**

编辑根 `Cargo.toml`：

在 `members` 数组中新增 `"crates/html_renderer"`：

```toml
members = [
    # ... 现有 members ...
    "crates/html_renderer",
]
```

在 `[workspace.dependencies]` 末尾添加 html5ever 系列和 html_renderer path 依赖：

```toml
# ---------- html_renderer ----------
html5ever = "0.27"
markup5ever = "0.12"
html_renderer = { path = "crates/html_renderer" }
```

- [ ] **步骤 2：创建 html_renderer/Cargo.toml**

```toml
[package]
name = "html_renderer"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lints]
workspace = true

[dependencies]
html5ever.workspace = true
markup5ever.workspace = true
egui.workspace = true
```

- [ ] **步骤 3：创建 lib.rs stub**

```rust
//! HTML 内嵌渲染器。
//!
//! 将 Markdown 中的内嵌 HTML 解析为 DOM 树并渲染为 egui 富文本/块级组件。

pub mod block;
pub mod css;
pub mod inline;
pub mod parser;

use egui::{FontId, Ui};

/// 渲染内联 HTML 字符串（在段落内调用）。
///
/// 解析失败或标签未识别时回退为源码弱化文本显示。
pub fn render_inline_html(ui: &mut Ui, html: &str, base_font: &FontId) {
    let _ = ui;
    let _ = html;
    let _ = base_font;
}

/// 渲染块级 HTML 字符串（在 Block context 调用）。
pub fn render_block_html(ui: &mut Ui, html: &str) {
    let _ = ui;
    let _ = html;
}
```

- [ ] **步骤 4：编译验证**

```bash
cargo check -p html_renderer
```

预期：编译通过。

- [ ] **步骤 5：Commit**

```bash
git add Cargo.toml crates/html_renderer/
git commit -m "chore(html_renderer): add crate skeleton and workspace config

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 2：CSS 解析器 (css.rs)

**文件：**
- 修改：`crates/html_renderer/src/css.rs`

- [ ] **步骤 1：编写颜色解析测试**

在 `crates/html_renderer/src/css.rs` 编写完整文件：

```rust
//! 内联 style 属性解析。
//!
//! 支持 9 个 CSS 属性到 egui 的映射。

use egui::Color32;

/// 解析后的 CSS 样式属性集合。
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CssStyle {
    pub color: Option<Color32>,
    pub background_color: Option<Color32>,
    pub font_size: Option<f32>,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub text_decoration: Option<TextDecoration>,
    pub text_align: Option<TextAlign>,
    pub margin: Option<Spacing>,
    pub padding: Option<Spacing>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FontWeight {
    Bold,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FontStyle {
    Italic,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TextDecoration {
    Underline,
    LineThrough,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TextAlign {
    Left,
    Center,
    Right,
}

/// 四边间距。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Spacing {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Spacing {
    pub fn uniform(v: f32) -> Self {
        Self { top: v, right: v, bottom: v, left: v }
    }
}

/// 解析颜色字符串：支持十六进制 `#rgb` / `#rrggbb`、`rgb(r,g,b)` 和 16 个命名颜色。
fn parse_color(value: &str) -> Option<Color32> {
    let value = value.trim();
    if value.starts_with('#') {
        parse_hex_color(value)
    } else if value.starts_with("rgb(") {
        parse_rgb_color(value)
    } else {
        parse_named_color(value)
    }
}

fn parse_hex_color(value: &str) -> Option<Color32> {
    let hex = value.trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some(Color32::from_rgb(r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color32::from_rgb(r, g, b))
        }
        _ => None,
    }
}

fn parse_rgb_color(value: &str) -> Option<Color32> {
    let inner = value.trim_start_matches("rgb(").trim_end_matches(')');
    let mut parts = inner.split(',');
    let r: u8 = parts.next()?.trim().parse().ok()?;
    let g: u8 = parts.next()?.trim().parse().ok()?;
    let b: u8 = parts.next()?.trim().parse().ok()?;
    Some(Color32::from_rgb(r, g, b))
}

fn parse_named_color(value: &str) -> Option<Color32> {
    match value.to_lowercase().as_str() {
        "red" => Some(Color32::RED),
        "blue" => Some(Color32::BLUE),
        "green" => Some(Color32::GREEN),
        "yellow" => Some(Color32::YELLOW),
        "white" => Some(Color32::WHITE),
        "black" => Some(Color32::BLACK),
        "gray" | "grey" => Some(Color32::GRAY),
        "orange" => Some(Color32::from_rgb(255, 165, 0)),
        "purple" => Some(Color32::from_rgb(128, 0, 128)),
        "pink" => Some(Color32::from_rgb(255, 192, 203)),
        "cyan" => Some(Color32::from_rgb(0, 255, 255)),
        "magenta" => Some(Color32::from_rgb(255, 0, 255)),
        "lime" => Some(Color32::from_rgb(0, 255, 0)),
        "navy" => Some(Color32::from_rgb(0, 0, 128)),
        "teal" => Some(Color32::from_rgb(0, 128, 128)),
        "maroon" => Some(Color32::from_rgb(128, 0, 0)),
        _ => None,
    }
}

/// 解析 `style="..."` 属性字符串为 `CssStyle`。
pub(crate) fn parse_style(style_str: &str) -> CssStyle {
    let mut style = CssStyle::default();
    for decl in style_str.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let mut parts = decl.splitn(2, ':');
        let key = parts.next().map(|s| s.trim().to_lowercase());
        let val = parts.next().map(|s| s.trim());
        match (key.as_deref(), val) {
            (Some("color"), Some(v)) => style.color = parse_color(v),
            (Some("background-color"), Some(v)) => style.background_color = parse_color(v),
            (Some("font-size"), Some(v)) => style.font_size = parse_px(v),
            (Some("font-weight"), Some(v)) => style.font_weight = parse_font_weight(v),
            (Some("font-style"), Some(v)) => style.font_style = parse_font_style(v),
            (Some("text-decoration"), Some(v)) => style.text_decoration = parse_text_decoration(v),
            (Some("text-align"), Some(v)) => style.text_align = parse_text_align(v),
            (Some("margin"), Some(v)) => style.margin = parse_spacing(v),
            (Some("padding"), Some(v)) => style.padding = parse_spacing(v),
            _ => {}
        }
    }
    style
}

fn parse_px(value: &str) -> Option<f32> {
    let v = value.trim_end_matches("px").trim();
    v.parse::<f32>().ok()
}

fn parse_font_weight(value: &str) -> Option<FontWeight> {
    match value.trim().to_lowercase().as_str() {
        "bold" | "700" | "800" | "900" | "bolder" => Some(FontWeight::Bold),
        "normal" | "400" | "lighter" => Some(FontWeight::Normal),
        _ => None,
    }
}

fn parse_font_style(value: &str) -> Option<FontStyle> {
    match value.trim().to_lowercase().as_str() {
        "italic" | "oblique" => Some(FontStyle::Italic),
        "normal" => Some(FontStyle::Normal),
        _ => None,
    }
}

fn parse_text_decoration(value: &str) -> Option<TextDecoration> {
    match value.trim().to_lowercase().as_str() {
        "underline" => Some(TextDecoration::Underline),
        "line-through" => Some(TextDecoration::LineThrough),
        _ => None,
    }
}

fn parse_text_align(value: &str) -> Option<TextAlign> {
    match value.trim().to_lowercase().as_str() {
        "left" | "start" => Some(TextAlign::Left),
        "center" => Some(TextAlign::Center),
        "right" | "end" => Some(TextAlign::Right),
        _ => None,
    }
}

fn parse_spacing(value: &str) -> Option<Spacing> {
    let parts: Vec<f32> = value
        .split_whitespace()
        .filter_map(|s| s.trim_end_matches("px").parse::<f32>().ok())
        .collect();
    match parts.len() {
        1 => Some(Spacing::uniform(parts[0])),
        2 => Some(Spacing {
            top: parts[0],
            right: parts[1],
            bottom: parts[0],
            left: parts[1],
        }),
        3 => Some(Spacing {
            top: parts[0],
            right: parts[1],
            bottom: parts[2],
            left: parts[1],
        }),
        4 => Some(Spacing {
            top: parts[0],
            right: parts[1],
            bottom: parts[2],
            left: parts[3],
        }),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ---- color parsing ----

    #[test]
    fn parse_color_hex_3() {
        assert_eq!(parse_color("#f00"), Some(Color32::RED));
        assert_eq!(parse_color("#0f0"), Some(Color32::GREEN));
        assert_eq!(parse_color("#00f"), Some(Color32::BLUE));
    }

    #[test]
    fn parse_color_hex_6() {
        assert_eq!(parse_color("#ff0000"), Some(Color32::RED));
        assert_eq!(parse_color("#00ff00"), Some(Color32::GREEN));
    }

    #[test]
    fn parse_color_rgb() {
        assert_eq!(parse_color("rgb(255, 0, 0)"), Some(Color32::RED));
        assert_eq!(parse_color("rgb(0,0,255)"), Some(Color32::BLUE));
    }

    #[test]
    fn parse_color_named() {
        assert_eq!(parse_color("red"), Some(Color32::RED));
        assert_eq!(parse_color("blue"), Some(Color32::BLUE));
        assert_eq!(parse_color("orange"), Some(Color32::from_rgb(255, 165, 0)));
    }

    #[test]
    fn parse_color_invalid() {
        assert_eq!(parse_color("notacolor"), None);
        assert_eq!(parse_color(""), None);
    }

    // ---- CSS style parsing ----

    #[test]
    fn parse_style_color() {
        let s = parse_style("color: red");
        assert_eq!(s.color, Some(Color32::RED));
    }

    #[test]
    fn parse_style_bg_color() {
        let s = parse_style("background-color: #ff0");
        assert_eq!(s.background_color, Some(Color32::YELLOW));
    }

    #[test]
    fn parse_style_font_size() {
        let s = parse_style("font-size: 16px");
        assert_eq!(s.font_size, Some(16.0));
    }

    #[test]
    fn parse_style_font_weight() {
        let s = parse_style("font-weight: bold");
        assert_eq!(s.font_weight, Some(FontWeight::Bold));
    }

    #[test]
    fn parse_style_font_style() {
        let s = parse_style("font-style: italic");
        assert_eq!(s.font_style, Some(FontStyle::Italic));
    }

    #[test]
    fn parse_style_text_decoration() {
        let s = parse_style("text-decoration: underline");
        assert_eq!(s.text_decoration, Some(TextDecoration::Underline));

        let s = parse_style("text-decoration: line-through");
        assert_eq!(s.text_decoration, Some(TextDecoration::LineThrough));
    }

    #[test]
    fn parse_style_text_align() {
        let s = parse_style("text-align: center");
        assert_eq!(s.text_align, Some(TextAlign::Center));
    }

    #[test]
    fn parse_style_margin_uniform() {
        let s = parse_style("margin: 8px");
        assert_eq!(s.margin, Some(Spacing::uniform(8.0)));
    }

    #[test]
    fn parse_style_margin_two_values() {
        let s = parse_style("margin: 4px 8px");
        assert_eq!(
            s.margin,
            Some(Spacing {
                top: 4.0,
                right: 8.0,
                bottom: 4.0,
                left: 8.0,
            })
        );
    }

    #[test]
    fn parse_style_padding() {
        let s = parse_style("padding: 4px");
        assert_eq!(s.padding, Some(Spacing::uniform(4.0)));
    }

    #[test]
    fn parse_style_multiple_properties() {
        let s = parse_style("color: red; font-weight: bold; padding: 8px");
        assert_eq!(s.color, Some(Color32::RED));
        assert_eq!(s.font_weight, Some(FontWeight::Bold));
        assert_eq!(s.padding, Some(Spacing::uniform(8.0)));
    }

    #[test]
    fn parse_style_empty() {
        let s = parse_style("");
        assert_eq!(s, CssStyle::default());
    }

    #[test]
    fn parse_style_unknown_property_ignored() {
        let s = parse_style("display: flex; color: red");
        assert_eq!(s.color, Some(Color32::RED));
    }
}
```

- [ ] **步骤 2：运行 CSS 测试**

```bash
cargo test -p html_renderer
```

预期：全部 17 个 CSS 测试通过。

- [ ] **步骤 3：Commit**

```bash
git add crates/html_renderer/src/css.rs
git commit -m "feat(html_renderer): implement CSS inline style parser

- Parse 9 CSS properties: color, background-color, font-size,
  font-weight, font-style, text-decoration, text-align, margin, padding
- Color parsing: hex #rgb/#rrggbb, rgb(), 16 named colors
- Spacing parsing: 1-4 value shorthand
- 17 unit tests covering all properties and edge cases

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 3：HTML 解析器 (parser.rs)

**文件：**
- 修改：`crates/html_renderer/src/parser.rs`

- [ ] **步骤 1：编写 AST 类型定义 + 标签分类 + 解析实现**

```rust
//! HTML → HtmlNode 树：使用 html5ever 解析 HTML 片段。

use std::collections::HashMap;

use html5ever::parse_document;
use html5ever::rcdom::{Handle, NodeData, RcDom};
use html5ever::tendril::TendrilSink;

use crate::css::{self, CssStyle};

// ---- 标签枚举 ----

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum InlineTag {
    B,
    I,
    U,
    Code,
    Del,
    Mark,
    Sub,
    Sup,
    A,
    Span,
    Br,
    Small,
    Big,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum BlockTag {
    Div,
    P,
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
    Pre,
    Hr,
    Table,
    Blockquote,
    Ul,
    Ol,
    Li,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TagKind {
    Inline(InlineTag),
    Block(BlockTag),
    Unknown,
}

// ---- HtmlNode ----

/// DOM 节点。
#[derive(Debug, Clone)]
pub(crate) enum HtmlNode {
    Inline {
        tag: InlineTag,
        attrs: HashMap<String, String>,
        style: CssStyle,
        children: Vec<HtmlNode>,
    },
    Block {
        tag: BlockTag,
        attrs: HashMap<String, String>,
        style: CssStyle,
        children: Vec<HtmlNode>,
    },
    Text(String),
}

// ---- 标签分类 ----

fn classify(tag: &str) -> TagKind {
    match tag.to_lowercase().as_str() {
        "b" | "strong" => TagKind::Inline(InlineTag::B),
        "i" | "em" => TagKind::Inline(InlineTag::I),
        "u" | "ins" => TagKind::Inline(InlineTag::U),
        "code" => TagKind::Inline(InlineTag::Code),
        "del" | "s" | "strike" => TagKind::Inline(InlineTag::Del),
        "mark" => TagKind::Inline(InlineTag::Mark),
        "sub" => TagKind::Inline(InlineTag::Sub),
        "sup" => TagKind::Inline(InlineTag::Sup),
        "a" => TagKind::Inline(InlineTag::A),
        "span" | "font" => TagKind::Inline(InlineTag::Span),
        "br" => TagKind::Inline(InlineTag::Br),
        "small" => TagKind::Inline(InlineTag::Small),
        "big" => TagKind::Inline(InlineTag::Big),

        "div" => TagKind::Block(BlockTag::Div),
        "p" => TagKind::Block(BlockTag::P),
        "h1" => TagKind::Block(BlockTag::H1),
        "h2" => TagKind::Block(BlockTag::H2),
        "h3" => TagKind::Block(BlockTag::H3),
        "h4" => TagKind::Block(BlockTag::H4),
        "h5" => TagKind::Block(BlockTag::H5),
        "h6" => TagKind::Block(BlockTag::H6),
        "pre" => TagKind::Block(BlockTag::Pre),
        "hr" => TagKind::Block(BlockTag::Hr),
        "table" => TagKind::Block(BlockTag::Table),
        "blockquote" => TagKind::Block(BlockTag::Blockquote),
        "ul" => TagKind::Block(BlockTag::Ul),
        "ol" => TagKind::Block(BlockTag::Ol),
        "li" => TagKind::Block(BlockTag::Li),

        "html" | "head" | "body" | "meta" | "title" | "link"
        | "script" | "style" | "noscript" => TagKind::Unknown,
        _ => TagKind::Unknown,
    }
}

// ---- 入口 ----

/// 解析 HTML 片段为 HtmlNode 列表（内联上下文，不包裹 body）。
pub(crate) fn parse_inline(html: &str) -> Vec<HtmlNode> {
    let wrapped = format!("<html><body>{html}</body></html>");
    parse_fragment(&wrapped)
}

/// 解析 HTML 片段为 HtmlNode 列表（块级上下文）。
pub(crate) fn parse_block(html: &str) -> Vec<HtmlNode> {
    let wrapped = format!("<html><body>{html}</body></html>");
    parse_fragment(&wrapped)
}

fn parse_fragment(html: &str) -> Vec<HtmlNode> {
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes());

    match dom {
        Ok(dom) => {
            let body = find_body(&dom.document);
            let mut nodes = vec![];
            walk_children(&body, &mut nodes);
            nodes
        }
        Err(_) => vec![HtmlNode::Text(html.to_string())],
    }
}

fn find_body(handle: &Handle) -> Handle {
    for child in handle.children.borrow().iter() {
        if let NodeData::Element { ref name, .. } = child.data {
            if name.local.as_ref() == "body" {
                return child.clone();
            }
        }
        let found = find_body(child);
        if !is_document_root(&found) {
            return found;
        }
    }
    handle.clone()
}

fn is_document_root(handle: &Handle) -> bool {
    matches!(
        handle.data,
        NodeData::Document | NodeData::Doctype { .. }
    )
}

fn walk_children(handle: &Handle, out: &mut Vec<HtmlNode>) {
    for child in handle.children.borrow().iter() {
        match &child.data {
            NodeData::Text { contents } => {
                let text = contents.borrow().to_string();
                if !text.trim().is_empty() {
                    out.push(HtmlNode::Text(text));
                }
            }
            NodeData::Element { name, attrs, .. } => {
                let tag_name = name.local.as_ref();
                let kind = classify(tag_name);
                let attr_map = attrs_to_map(&attrs.borrow());

                let style_str = attr_map.get("style").cloned().unwrap_or_default();
                let style = css::parse_style(&style_str);

                match kind {
                    TagKind::Inline(tag) => {
                        let mut children = vec![];
                        walk_children(child, &mut children);
                        out.push(HtmlNode::Inline {
                            tag,
                            attrs: attr_map,
                            style,
                            children,
                        });
                    }
                    TagKind::Block(tag) => {
                        let mut children = vec![];
                        walk_children(child, &mut children);
                        out.push(HtmlNode::Block {
                            tag,
                            attrs: attr_map,
                            style,
                            children,
                        });
                    }
                    TagKind::Unknown => {
                        // 未知标签：透传子节点
                        walk_children(child, out);
                    }
                }
            }
            _ => {
                // Comment, ProcessingInstruction, etc. — 忽略
                walk_children(child, out);
            }
        }
    }
}

fn attrs_to_map(attrs: &[html5ever::Attribute]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for attr in attrs {
        map.insert(attr.name.local.to_string(), attr.value.to_string());
    }
    map
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn classify_inline_tags() {
        assert_eq!(classify("b"), TagKind::Inline(InlineTag::B));
        assert_eq!(classify("strong"), TagKind::Inline(InlineTag::B));
        assert_eq!(classify("i"), TagKind::Inline(InlineTag::I));
        assert_eq!(classify("em"), TagKind::Inline(InlineTag::I));
        assert_eq!(classify("u"), TagKind::Inline(InlineTag::U));
        assert_eq!(classify("code"), TagKind::Inline(InlineTag::Code));
        assert_eq!(classify("del"), TagKind::Inline(InlineTag::Del));
        assert_eq!(classify("mark"), TagKind::Inline(InlineTag::Mark));
        assert_eq!(classify("sub"), TagKind::Inline(InlineTag::Sub));
        assert_eq!(classify("sup"), TagKind::Inline(InlineTag::Sup));
        assert_eq!(classify("a"), TagKind::Inline(InlineTag::A));
        assert_eq!(classify("span"), TagKind::Inline(InlineTag::Span));
        assert_eq!(classify("br"), TagKind::Inline(InlineTag::Br));
    }

    #[test]
    fn classify_block_tags() {
        assert_eq!(classify("div"), TagKind::Block(BlockTag::Div));
        assert_eq!(classify("p"), TagKind::Block(BlockTag::P));
        assert_eq!(classify("h1"), TagKind::Block(BlockTag::H1));
        assert_eq!(classify("h6"), TagKind::Block(BlockTag::H6));
        assert_eq!(classify("pre"), TagKind::Block(BlockTag::Pre));
        assert_eq!(classify("hr"), TagKind::Block(BlockTag::Hr));
        assert_eq!(classify("table"), TagKind::Block(BlockTag::Table));
        assert_eq!(classify("blockquote"), TagKind::Block(BlockTag::Blockquote));
        assert_eq!(classify("ul"), TagKind::Block(BlockTag::Ul));
        assert_eq!(classify("ol"), TagKind::Block(BlockTag::Ol));
        assert_eq!(classify("li"), TagKind::Block(BlockTag::Li));
    }

    #[test]
    fn classify_case_insensitive() {
        assert_eq!(classify("DIV"), TagKind::Block(BlockTag::Div));
        assert_eq!(classify("Strong"), TagKind::Inline(InlineTag::B));
    }

    #[test]
    fn classify_unknown() {
        assert_eq!(classify("custom-element"), TagKind::Unknown);
        assert_eq!(classify("script"), TagKind::Unknown);
        assert_eq!(classify("style"), TagKind::Unknown);
    }

    #[test]
    fn parse_simple_text() {
        let nodes = parse_inline("hello");
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            HtmlNode::Text(s) => assert_eq!(s, "hello"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn parse_bold() {
        let nodes = parse_inline("<b>bold</b>");
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            HtmlNode::Inline { tag, children, .. } => {
                assert_eq!(*tag, InlineTag::B);
                match &children[0] {
                    HtmlNode::Text(s) => assert_eq!(s, "bold"),
                    _ => panic!("expected Text"),
                }
            }
            _ => panic!("expected Inline"),
        }
    }

    #[test]
    fn parse_nested() {
        let nodes = parse_inline("<b><i>bold italic</i></b>");
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            HtmlNode::Inline { tag, children, .. } => {
                assert_eq!(*tag, InlineTag::B);
                match &children[0] {
                    HtmlNode::Inline { tag, children, .. } => {
                        assert_eq!(*tag, InlineTag::I);
                        match &children[0] {
                            HtmlNode::Text(s) => assert_eq!(s, "bold italic"),
                            _ => panic!("expected Text"),
                        }
                    }
                    _ => panic!("expected Inline"),
                }
            }
            _ => panic!("expected Inline"),
        }
    }

    #[test]
    fn parse_block_div() {
        let nodes = parse_block("<div>content</div>");
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            HtmlNode::Block { tag, .. } => assert_eq!(*tag, BlockTag::Div),
            _ => panic!("expected Block"),
        }
    }

    #[test]
    fn parse_style_attribute() {
        let nodes = parse_inline("<span style=\"color: red\">text</span>");
        match &nodes[0] {
            HtmlNode::Inline { tag, style, .. } => {
                assert_eq!(*tag, InlineTag::Span);
                assert_eq!(style.color, Some(egui::Color32::RED));
            }
            _ => panic!("expected Inline"),
        }
    }

    #[test]
    fn parse_unknown_tag_passthrough() {
        let nodes = parse_inline("<custom>text</custom>");
        // custom tag is unknown, children are passed through
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            HtmlNode::Text(s) => assert_eq!(s, "text"),
            _ => panic!("expected Text from passthrough"),
        }
    }
}
```

- [ ] **步骤 2：运行解析器测试**

```bash
cargo test -p html_renderer
```

预期：全部测试通过（17 CSS + 11 解析器 = 28 个测试）。

- [ ] **步骤 3：Commit**

```bash
git add crates/html_renderer/src/parser.rs
git commit -m "feat(html_renderer): implement HTML parser with html5ever

- html5ever parse_document + RcDom for DOM tree construction
- Classify 30+ HTML tags into Inline/Block/Unknown
- Walk RcDom tree to build HtmlNode AST
- Support style attribute extraction and CSS parsing
- Unknown tags: passthrough children, skip tag itself
- 11 unit tests covering classification, parsing, nesting, style

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 4：内联标签渲染 (inline.rs)

**文件：**
- 修改：`crates/html_renderer/src/inline.rs`

- [ ] **步骤 1：实现内联渲染**

```rust
//! 内联 HTML 标签 → egui RichText 渲染。

use egui::{FontId, RichText, Ui};

use crate::css::{CssStyle, FontStyle, FontWeight, TextDecoration};
use crate::parser::{HtmlNode, InlineTag};

/// 渲染内联 HtmlNode 列表。
pub(crate) fn render_inline_nodes(ui: &mut Ui, nodes: &[HtmlNode], base_font: &FontId) {
    ui.horizontal_wrapped(|ui| {
        for node in nodes {
            render_inline_node(ui, node, base_font);
        }
    });
}

fn render_inline_node(ui: &mut Ui, node: &HtmlNode, base_font: &FontId) {
    match node {
        HtmlNode::Text(s) => {
            ui.label(RichText::new(s.as_str()).font(base_font.clone()));
        }
        HtmlNode::Inline { tag, attrs, style, children } => {
            let mut rt = build_richtext_base(ui, tag, attrs, style, base_font);
            // 嵌套子节点：累积子文本
            for child in children {
                append_child_text(&mut rt, child, base_font);
            }
            // 根据标签类型渲染
            match tag {
                InlineTag::A => {
                    let url = attrs.get("href").map(|s| s.as_str()).unwrap_or("#");
                    ui.hyperlink_to(rt, url);
                }
                InlineTag::Br => {
                    ui.label(RichText::new("\n").font(base_font.clone()));
                }
                _ => {
                    ui.label(rt);
                }
            }
        }
        HtmlNode::Block { .. } => {
            // 块级节点不应出现在内联上下文中，忽略
        }
    }
}

fn build_richtext_base(
    ui: &mut Ui,
    tag: &InlineTag,
    _attrs: &std::collections::HashMap<String, String>,
    style: &CssStyle,
    base_font: &FontId,
) -> RichText {
    let _ = ui;
    let mut rt = RichText::new(String::new());

    // 应用 CSS 样式
    if let Some(c) = style.color {
        rt = rt.color(c);
    }
    if let Some(c) = style.background_color {
        rt = rt.background_color(c);
    }
    if let Some(size) = style.font_size {
        rt = rt.font(FontId::new(size, base_font.family.clone()));
    } else {
        rt = rt.font(base_font.clone());
    }
    match style.font_weight {
        Some(FontWeight::Bold) => {
            rt = rt.strong();
        }
        Some(FontWeight::Normal) => {}
        None => {}
    }
    match style.font_style {
        Some(FontStyle::Italic) => {
            rt = rt.italics();
        }
        Some(FontStyle::Normal) => {}
        None => {}
    }
    match style.text_decoration {
        Some(TextDecoration::Underline) => {
            rt = rt.underline();
        }
        Some(TextDecoration::LineThrough) => {
            rt = rt.strikethrough();
        }
        None => {}
    }

    // 应用标签语义（如果 CSS 未覆盖）
    match tag {
        InlineTag::B => {
            if style.font_weight.is_none() {
                rt = rt.strong();
            }
        }
        InlineTag::I => {
            if style.font_style.is_none() {
                rt = rt.italics();
            }
        }
        InlineTag::U => {
            if style.text_decoration.is_none() {
                rt = rt.underline();
            }
        }
        InlineTag::Del => {
            if style.text_decoration.is_none() {
                rt = rt.strikethrough();
            }
        }
        InlineTag::Code => {
            rt = rt.code();
        }
        InlineTag::Mark => {
            if style.background_color.is_none() {
                rt = rt.background_color(egui::Color32::from_rgb(255, 255, 0));
            }
        }
        InlineTag::Sub => {
            let small_size = base_font.size * 0.75;
            rt = rt.font(FontId::new(small_size, base_font.family.clone()));
            // Note: egui doesn't natively support vertical offset for sub/sup.
            // The size reduction is the best approximation.
        }
        InlineTag::Sup => {
            let small_size = base_font.size * 0.75;
            rt = rt.font(FontId::new(small_size, base_font.family.clone()));
        }
        InlineTag::Small => {
            let small_size = base_font.size * 0.85;
            rt = rt.font(FontId::new(small_size, base_font.family.clone()));
        }
        InlineTag::Big => {
            let big_size = base_font.size * 1.2;
            rt = rt.font(FontId::new(big_size, base_font.family.clone()));
        }
        InlineTag::Span | InlineTag::Br | InlineTag::A => {}
    }

    rt
}

fn append_child_text(rt: &mut RichText, node: &HtmlNode, base_font: &FontId) {
    match node {
        HtmlNode::Text(s) => {
            *rt = rt.clone().text(s.as_str());
        }
        HtmlNode::Inline { .. } => {
            // 嵌套内联标签：当前简化处理，提取纯文本
            let text = collect_text(node);
            *rt = rt.clone().text(text);
        }
        HtmlNode::Block { .. } => {}
    }
}

fn collect_text(node: &HtmlNode) -> String {
    match node {
        HtmlNode::Text(s) => s.clone(),
        HtmlNode::Inline { children, .. } => {
            children.iter().map(collect_text).collect()
        }
        HtmlNode::Block { children, .. } => {
            children.iter().map(collect_text).collect()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn collect_text_simple() {
        let node = HtmlNode::Text("hello".into());
        assert_eq!(collect_text(&node), "hello");
    }

    #[test]
    fn collect_text_nested() {
        let node = HtmlNode::Inline {
            tag: InlineTag::B,
            attrs: Default::default(),
            style: Default::default(),
            children: vec![
                HtmlNode::Text("bold ".into()),
                HtmlNode::Inline {
                    tag: InlineTag::I,
                    attrs: Default::default(),
                    style: Default::default(),
                    children: vec![HtmlNode::Text("italic".into())],
                },
            ],
        };
        assert_eq!(collect_text(&node), "bold italic");
    }
}
```

- [ ] **步骤 2：运行测试**

```bash
cargo test -p html_renderer
```

预期：全部测试通过（28 + 2 = 30 个测试）。

- [ ] **步骤 3：Commit**

```bash
git add crates/html_renderer/src/inline.rs
git commit -m "feat(html_renderer): implement inline HTML tag rendering

- Tag → RichText mapping: b/strong/em/i/u/ins/del/s/code/mark/sub/sup/a/br/span/font/small/big
- CSS style application: color, background-color, font-size, font-weight,
  font-style, text-decoration
- <a href> rendered as ui.hyperlink_to()
- Tag semantics override CSS defaults when CSS not specified
- Nested child text collection for RichText
- 2 unit tests for text collection

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 5：块级标签渲染 (block.rs)

**文件：**
- 修改：`crates/html_renderer/src/block.rs`

- [ ] **步骤 1：实现块级渲染**

```rust
//! 块级 HTML 标签 → egui Frame/Grid/Layout 渲染。

use egui::{Align, Color32, Direction, Layout, Ui};

use crate::css::{CssStyle, TextAlign};
use crate::inline;
use crate::parser::{BlockTag, HtmlNode};

/// 渲染块级 HtmlNode 列表。
pub(crate) fn render_block_nodes(ui: &mut Ui, nodes: &[HtmlNode]) {
    for node in nodes {
        render_block_node(ui, node);
    }
}

fn render_block_node(ui: &mut Ui, node: &HtmlNode) {
    match node {
        HtmlNode::Block { tag, style, children, .. } => match tag {
            BlockTag::Div | BlockTag::P | BlockTag::Blockquote => {
                render_div_like(ui, tag, style, children);
            }
            BlockTag::H1
            | BlockTag::H2
            | BlockTag::H3
            | BlockTag::H4
            | BlockTag::H5
            | BlockTag::H6 => {
                render_heading(ui, tag, style, children);
            }
            BlockTag::Pre => {
                render_pre(ui, style, children);
            }
            BlockTag::Hr => {
                ui.separator();
            }
            BlockTag::Table => {
                render_table(ui, style, children);
            }
            BlockTag::Ul => {
                render_list(ui, false, style, children);
            }
            BlockTag::Ol => {
                render_list(ui, true, style, children);
            }
            BlockTag::Li => {
                // <li> 出现在顶部时应渲染为段落
                render_div_like(ui, &BlockTag::Div, style, children);
            }
        },
        HtmlNode::Text(s) => {
            ui.label(s.as_str());
        }
        HtmlNode::Inline { .. } => {
            // 内联节点出现在块级上下文：用默认字体渲染
            let font_id = egui::FontId::default();
            inline::render_inline_nodes(ui, &[node.clone()], &font_id);
        }
    }
}

fn render_div_like(
    ui: &mut Ui,
    _tag: &BlockTag,
    style: &CssStyle,
    children: &[HtmlNode],
) {
    let mut frame = egui::Frame::group(ui.style());

    // 应用 padding
    if let Some(pad) = style.padding {
        frame = frame.inner_margin(egui::Margin {
            left: pad.left as i32,
            right: pad.right as i32,
            top: pad.top as i32,
            bottom: pad.bottom as i32,
        });
    }

    // 应用 margin
    if let Some(m) = style.margin {
        frame = frame.outer_margin(egui::Margin {
            left: m.left as i32,
            right: m.right as i32,
            top: m.top as i32,
            bottom: m.bottom as i32,
        });
    }

    // 应用背景色
    if let Some(bg) = style.background_color {
        frame = frame.fill(bg);
    }

    let align_layout = text_align_to_layout(style.text_align);

    frame.show(ui, |ui| {
        ui.with_layout(align_layout, |ui| {
            for child in children {
                match child {
                    HtmlNode::Block { .. } => render_block_node(ui, child),
                    HtmlNode::Text(s) => {
                        ui.label(s.as_str());
                    }
                    HtmlNode::Inline { .. } => {
                        let font_id = egui::FontId::default();
                        inline::render_inline_nodes(ui, &[child.clone()], &font_id);
                    }
                }
            }
        });
    });
}

fn text_align_to_layout(align: Option<TextAlign>) -> Layout {
    match align {
        Some(TextAlign::Center) => {
            Layout::top_down_justified(Align::Center)
        }
        Some(TextAlign::Right) => {
            Layout::right_to_left(Align::Min)
        }
        _ => Layout::left_to_right(Align::Min),
    }
}

fn render_heading(
    ui: &mut Ui,
    tag: &BlockTag,
    style: &CssStyle,
    children: &[HtmlNode],
) {
    let level = match tag {
        BlockTag::H1 => 1,
        BlockTag::H2 => 2,
        BlockTag::H3 => 3,
        BlockTag::H4 => 4,
        BlockTag::H5 => 5,
        BlockTag::H6 => 6,
        _ => 3,
    };

    let font_size = match level {
        1 => 28.0,
        2 => 24.0,
        3 => 20.0,
        4 => 18.0,
        5 => 16.0,
        _ => 14.0,
    };

    let heading_font = egui::FontId::new(font_size, egui::FontFamily::Proportional);
    let text = children
        .iter()
        .map(|c| match c {
            HtmlNode::Text(s) => s.clone(),
            _ => String::new(),
        })
        .collect::<String>();

    let mut rt = egui::RichText::new(text)
        .strong()
        .font(heading_font);

    if let Some(c) = style.color {
        rt = rt.color(c);
    }

    ui.label(rt);
}

fn render_pre(
    ui: &mut Ui,
    style: &CssStyle,
    children: &[HtmlNode],
) {
    let text = children
        .iter()
        .map(|c| match c {
            HtmlNode::Text(s) => s.clone(),
            _ => String::new(),
        })
        .collect::<String>();

    let mut frame = egui::Frame::group(ui.style());

    if let Some(bg) = style.background_color {
        frame = frame.fill(bg);
    }

    frame.show(ui, |ui| {
        ui.label(
            egui::RichText::new(text)
                .monospace()
                .font(egui::FontId::monospace(13.0)),
        );
    });
}

fn render_table(
    ui: &mut Ui,
    _style: &CssStyle,
    children: &[HtmlNode],
) {
    let rows = extract_table_rows(children);

    if rows.is_empty() {
        return;
    }

    let table_id = egui::Id::new(format!("html_table_{:p}", children.as_ptr()));
    egui::Grid::new(table_id)
        .striped(true)
        .show(ui, |ui| {
            for row in &rows {
                for cell in row {
                    ui.label(cell);
                }
                ui.end_row();
            }
        });
}

fn extract_table_rows(nodes: &[HtmlNode]) -> Vec<Vec<String>> {
    let mut rows = vec![];

    for node in nodes {
        match node {
            HtmlNode::Block { tag, children, .. } => match tag {
                BlockTag::Tr => {
                    let mut row = vec![];
                    for child in children {
                        match child {
                            HtmlNode::Block { tag, children, .. }
                                if *tag == BlockTag::Td || *tag == BlockTag::Th =>
                            {
                                let text: String =
                                    children.iter().map(|c| match c {
                                        HtmlNode::Text(s) => s.clone(),
                                        _ => String::new(),
                                    }).collect();
                                row.push(text);
                            }
                            HtmlNode::Text(s) => {
                                row.push(s.clone());
                            }
                            _ => {}
                        }
                    }
                    if !row.is_empty() {
                        rows.push(row);
                    }
                }
                BlockTag::Thead | BlockTag::Tbody => {
                    rows.extend(extract_table_rows(children));
                }
                _ => {}
            },
            _ => {}
        }
    }

    rows
}

fn render_list(
    ui: &mut Ui,
    ordered: bool,
    _style: &CssStyle,
    children: &[HtmlNode],
) {
    ui.vertical(|ui| {
        let mut idx = 1usize;
        for node in children {
            match node {
                HtmlNode::Block { tag, children, .. } if *tag == BlockTag::Li => {
                    let marker = if ordered {
                        let m = format!("{idx}. ");
                        idx += 1;
                        m
                    } else {
                        "• ".to_owned()
                    };
                    let text: String = children
                        .iter()
                        .map(|c| match c {
                            HtmlNode::Text(s) => s.clone(),
                            _ => String::new(),
                        })
                        .collect();
                    ui.label(format!("{marker}{text}"));
                }
                HtmlNode::Text(s) => {
                    ui.label(s.as_str());
                }
                _ => {
                    render_block_node(ui, node);
                }
            }
        }
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn extract_table_rows_basic() {
        let nodes = vec![HtmlNode::Block {
            tag: BlockTag::Tr,
            attrs: Default::default(),
            style: Default::default(),
            children: vec![
                HtmlNode::Block {
                    tag: BlockTag::Td,
                    attrs: Default::default(),
                    style: Default::default(),
                    children: vec![HtmlNode::Text("A".into())],
                },
                HtmlNode::Block {
                    tag: BlockTag::Td,
                    attrs: Default::default(),
                    style: Default::default(),
                    children: vec![HtmlNode::Text("B".into())],
                },
            ],
        }];
        let rows = extract_table_rows(&nodes);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 2);
        assert_eq!(rows[0][0], "A");
        assert_eq!(rows[0][1], "B");
    }

    #[test]
    fn extract_table_rows_with_thead() {
        let nodes = vec![HtmlNode::Block {
            tag: BlockTag::Thead,
            attrs: Default::default(),
            style: Default::default(),
            children: vec![HtmlNode::Block {
                tag: BlockTag::Tr,
                attrs: Default::default(),
                style: Default::default(),
                children: vec![HtmlNode::Block {
                    tag: BlockTag::Th,
                    attrs: Default::default(),
                    style: Default::default(),
                    children: vec![HtmlNode::Text("Header".into())],
                }],
            }],
        }];
        let rows = extract_table_rows(&nodes);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], "Header");
    }

    #[test]
    fn extract_table_rows_empty() {
        let rows = extract_table_rows(&[]);
        assert!(rows.is_empty());
    }
}
```

- [ ] **步骤 2：运行测试**

```bash
cargo test -p html_renderer
```

预期：全部测试通过（30 + 3 = 33 个测试）。

- [ ] **步骤 3：Commit**

```bash
git add crates/html_renderer/src/block.rs
git commit -m "feat(html_renderer): implement block HTML tag rendering

- div/p/blockquote: egui::Frame::group() with padding/margin/bg
- h1-h6: scaled RichText with strong weight
- pre: monospace text in Frame group
- hr: ui.separator()
- table: egui::Grid::striped() from tr/td/th/thead/tbody
- ul/ol/li: bulleted/numbered list rendering
- text-align: left/center/right via Layout
- 3 unit tests for table row extraction

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 6：公共 API 完善 + 集成测试 (lib.rs)

**文件：**
- 修改：`crates/html_renderer/src/lib.rs`

- [ ] **步骤 1：完善 lib.rs 公共 API + 降级逻辑**

```rust
//! HTML 内嵌渲染器。
//!
//! 将 Markdown 中的内嵌 HTML 解析为 DOM 树并渲染为 egui 富文本/块级组件。

pub mod block;
pub mod css;
pub mod inline;
pub mod parser;

use egui::{FontId, RichText, Ui};

/// 渲染内联 HTML 字符串（在段落内调用）。
///
/// 解析失败或标签未识别时回退为源码弱化文本显示。
pub fn render_inline_html(ui: &mut Ui, html: &str, base_font: &FontId) {
    let nodes = parser::parse_inline(html);
    if nodes.is_empty() {
        return;
    }
    // 如果解析结果只有 Text 节点且内容与原始 HTML 相同（解析失败回退），
    // 则显示为弱文本
    if nodes.len() == 1 {
        if let parser::HtmlNode::Text(ref s) = nodes[0] {
            if s == html {
                ui.label(RichText::new(html).weak().font(base_font.clone()));
                return;
            }
        }
    }
    inline::render_inline_nodes(ui, &nodes, base_font);
}

/// 渲染块级 HTML 字符串（在 Block context 调用）。
pub fn render_block_html(ui: &mut Ui, html: &str) {
    let nodes = parser::parse_block(html);
    if nodes.is_empty() {
        return;
    }
    block::render_block_nodes(ui, &nodes);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "html_renderer");
    }

    #[test]
    fn render_inline_html_empty() {
        // 空字符串不 panic
        let _ = parser::parse_inline("");
    }

    #[test]
    fn render_inline_html_bold() {
        let nodes = parser::parse_inline("<b>hello</b>");
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn render_block_html_div() {
        let nodes = parser::parse_block("<div>content</div>");
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn render_inline_html_nested_mixed() {
        let nodes = parser::parse_inline("<b>bold <i>and italic</i></b> text");
        assert!(!nodes.is_empty());
    }

    #[test]
    fn render_block_html_table() {
        let html = "<table><tr><td>A</td><td>B</td></tr></table>";
        let nodes = parser::parse_block(html);
        assert!(!nodes.is_empty());
    }

    #[test]
    fn render_inline_html_br() {
        let nodes = parser::parse_inline("line1<br>line2");
        assert_eq!(nodes.len(), 2); // Text + Br
    }
}
```

- [ ] **步骤 2：运行全部测试**

```bash
cargo test -p html_renderer
```

预期：全部测试通过（33 + 7 = 40 个测试）。

- [ ] **步骤 3：Commit**

```bash
git add crates/html_renderer/src/lib.rs
git commit -m "feat(html_renderer): finalize public API with fallback behavior

- render_inline_html(): parse and render, fallback to weak text on error
- render_block_html(): parse and render block nodes
- 7 integration tests covering empty, bold, div, nested, table, br

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 7：集成到 markdown_renderer

**文件：**
- 修改：`crates/markdown_renderer/Cargo.toml`
- 修改：`crates/markdown_renderer/src/render.rs`

- [ ] **步骤 1：添加 html_renderer 依赖**

在 `crates/markdown_renderer/Cargo.toml` 的 `[dependencies]` 中添加：

```toml
html_renderer.workspace = true
```

- [ ] **步骤 2：修改 render.rs 中的 HtmlBlock 和 Inline::Html 处理**

编辑 `crates/markdown_renderer/src/render.rs`：

找到 `render_block` 函数中的 `Block::HtmlBlock` 分支（约第 31-33 行）：

```rust
// 原来：
Block::HtmlBlock(s) => {
    ui.label(egui::RichText::new(s).code().weak());
}
```

替换为：

```rust
Block::HtmlBlock(s) => {
    html_renderer::render_block_html(ui, s);
}
```

找到 `render_inlines` 函数中的 `Inline::Html` 分支（约第 109-111 行）：

```rust
// 原来：
Inline::Html(s) => {
    ui.label(egui::RichText::new(s).weak().font(font_id.clone()));
}
```

替换为：

```rust
Inline::Html(s) => {
    html_renderer::render_inline_html(ui, s, font_id);
}
```

- [ ] **步骤 3：编译验证**

```bash
cargo check -p markdown_renderer
```

预期：编译通过。

- [ ] **步骤 4：运行完整工作区编译**

```bash
cargo check --workspace
```

预期：编译通过，无错误。

- [ ] **步骤 5：Commit**

```bash
git add crates/markdown_renderer/
git commit -m "feat(markdown_renderer): integrate html_renderer for HtmlBlock/Inline::Html

- Replace HtmlBlock passthrough with html_renderer::render_block_html()
- Replace Inline::Html weak text with html_renderer::render_inline_html()
- Add html_renderer dependency to markdown_renderer

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 8：最终验证

**文件：** 无特定文件，验证整体一致性。

- [ ] **步骤 1：编译整个工作区**

```bash
cargo check --workspace
```

预期：编译通过，无错误无警告。

- [ ] **步骤 2：运行全部测试**

```bash
cargo test --workspace
```

预期：全部测试通过（包括 html_renderer 40 个测试 + 所有现有测试）。

- [ ] **步骤 3：运行 clippy**

```bash
cargo clippy --workspace -- -D warnings
```

预期：clippy clean。

- [ ] **步骤 4：运行 fmt**

```bash
cargo fmt -- --check
```

预期：格式化干净。

- [ ] **步骤 5：修复任何编译错误或测试失败**

逐个修复问题，确保全部通过。

- [ ] **步骤 6：Commit**

```bash
git add -A
git commit -m "chore: final verification — clippy, fmt, tests all pass

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## 验证清单

实现完成后，逐项确认：

- [ ] `cargo check --workspace` 通过
- [ ] `cargo test --workspace` 全部通过（html_renderer ≥ 40 个测试 + 所有现有测试）
- [ ] `cargo clippy --workspace -- -D warnings` 干净
- [ ] `cargo fmt -- --check` 干净
- [ ] 简单 HTML 标签如 `<b>bold</b>` 渲染为粗体而非源码文本
- [ ] 带 CSS 的 `<span style="color: red">text</span>` 渲染为红色文本
- [ ] `<div style="background: #eee; padding: 8px">` 渲染为带背景和内边距的块
- [ ] `<table>` 渲染为 Striped Grid
- [ ] 未识别或无效 HTML 回退为弱化文本
- [ ] 现有 Markdown 渲染功能不受影响
