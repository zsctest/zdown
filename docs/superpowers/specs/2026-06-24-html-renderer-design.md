# HTML 内嵌渲染 设计规格

**日期**: 2026-06-24
**状态**: 已确认
**范围**: 阶段 4.4 — 渲染 Markdown 中的内嵌 HTML

---

## 1. 目标

为 zdown Markdown 编辑器实现 Markdown 内嵌 HTML 的真正渲染。当前 AST 已支持 `HtmlBlock` 和 `Inline::Html`，但渲染层仅将 HTML 源码显示为弱化文本。本功能将 HTML 源码解析为 DOM 树并渲染为对应的 egui 富文本样式。

### 覆盖范围

- 内联标签：`b`, `strong`, `i`, `em`, `u`, `ins`, `del`, `s`, `code`, `mark`, `sub`, `sup`, `a`, `br`, `span`, `font`, `small`, `big`
- 块级标签：`div`, `p`, `h1`-`h6`, `pre`, `hr`, `table` (+ `tr`/`td`/`th`/`thead`/`tbody`), `blockquote`, `ul`, `ol`, `li`
- CSS 内联样式：`color`, `background-color`, `font-size`, `font-weight`, `font-style`, `text-decoration`, `text-align`, `margin`, `padding`
- 嵌套标签支持
- `<a href>` 渲染为 egui hyperlink

### 非覆盖范围

- 外部样式表（`<link>` / `<style>` 块）
- JavaScript
- CSS `border` / `width` / `height` / `line-height` / `position` / `display` / `float` / `flex`
- `<img>` 标签（Markdown 已有图片语法）
- `<input>` / `<form>` / `<button>` / `<select>` 等交互标签
- PDF 导出中渲染 HTML（当前 PDF 导出跳过 HtmlBlock，保持不变）

---

## 2. 架构

### 2.1 新增 crate: `crates/html_renderer`

```
crates/html_renderer/
├── Cargo.toml
└── src/
    ├── lib.rs          # 公共 API + 测试
    ├── parser.rs       # html5ever 封装，Markdown 片段 → DOM
    ├── css.rs          # 内联 style 解析
    ├── inline.rs       # 内联标签 → egui RichText/Label
    └── block.rs        # 块级标签 → egui Frame/Grid/Layout
```

### 2.2 依赖

```toml
[dependencies]
html5ever = "0.27"
markup5ever = "0.12"
egui.workspace = true
```

### 2.3 数据模型

```rust
/// DOM 节点
enum HtmlNode {
    Inline {
        tag: InlineTag,
        attrs: HashMap<String, String>,  // href, title, etc.
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

/// CSS 属性集合（解析后的结果）
struct CssStyle {
    color: Option<Color32>,
    background_color: Option<Color32>,
    font_size: Option<f32>,
    font_weight: Option<FontWeight>,
    font_style: Option<FontStyle>,
    text_decoration: Option<TextDecoration>,
    text_align: Option<Align>,
    margin: Option<Margin>,
    padding: Option<Margin>,
}
```

### 2.4 公共 API

```rust
/// 渲染内联 HTML 字符串（在段落内调用）
pub fn render_inline_html(ui: &mut Ui, html: &str, base_font: &FontId);

/// 渲染块级 HTML 字符串（在 Block context 调用）
pub fn render_block_html(ui: &mut Ui, html: &str);
```

---

## 3. CSS Parser (css.rs)

### 支持的属性映射

| CSS 属性 | 值格式 | egui 映射 |
|---|---|---|
| `color` | `#rgb`, `#rrggbb`, `r,g,b`, 颜色名 | `Color32` |
| `background-color` | 同上 | `Color32` → painter |
| `font-size` | `12px`, `14px`, `1.2em` | `f32` 字号 |
| `font-weight` | `bold`/`normal`/`700` | `RichText::strong()` |
| `font-style` | `italic`/`normal` | `RichText::italics()` |
| `text-decoration` | `underline`/`line-through` | `underline()`/`strikethrough()` |
| `text-align` | `left`/`center`/`right` | egui layout |
| `margin` | `4px`, `4px 8px` (1-4 值) | egui spacing |
| `padding` | 同上 | egui Frame inner_margin |

### 颜色名支持

16 个：`red`, `blue`, `green`, `yellow`, `white`, `black`, `gray`, `orange`, `purple`, `pink`, `cyan`, `magenta`, `lime`, `navy`, `teal`, `maroon`

### 解析流程

```
"color: red; font-weight: bold; padding: 4px 8px"
    ↓ split by ';'
["color: red", "font-weight: bold", "padding: 4px 8px"]
    ↓ each → split by ':'
[("color", "red"), ("font-weight", "bold"), ("padding", "4px 8px")]
    ↓ match key → parse value → set CssStyle field
```

纯函数实现，无外部依赖，约 150 行。

---

## 4. HTML → DOM 解析 (parser.rs)

### 方案

html5ever 的 `parse_document` + `RcDom` 构建 DOM 树，然后递归遍历转为 `Vec<HtmlNode>`。

HTML 片段包裹：`"<html><body>" + html + "</body></html>"`，只取 `<body>` 子节点。

### 标签分类

```rust
fn classify(tag: &str) -> TagKind {
    match tag {
        "b" | "i" | "u" | "a" | "code" | "span"
        | "del" | "mark" | "sub" | "sup" | "br"
        | "strong" | "em" | "s" | "small" | "big"
        | "font" => TagKind::Inline,

        "div" | "table" | "pre" | "hr"
        | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        | "p" | "blockquote" | "ul" | "ol" | "li"
        | "tr" | "td" | "th" | "thead" | "tbody" => TagKind::Block,

        _ => TagKind::Unknown,  // 透传子节点，不产生标签节点
    }
}
```

### 特殊处理

- `<br>` → 无子节点的 Inline，渲染时输出换行
- `<a href="...">` → 保留 `href` 属性在 `attrs` 中
- `<table>` → 按 `<tr>` + `<td>`/`<th>` 层级构建
- 未知标签 → 递归渲染子节点，标签本身被忽略（不渲染 HTML 源码）

---

## 5. 块级渲染 (block.rs)

### 分发逻辑

```rust
fn render_block_node(ui: &mut Ui, node: &HtmlNode) {
    match node.tag {
        Div | P | Blockquote => render_div(ui, &node),
        H1..=H6 => render_heading(ui, &node, level),
        Pre => render_pre(ui, &node),
        Hr => render_hr(ui),
        Table => render_table(ui, &node),
    }
}
```

### `<div>` / `<p>` / `<blockquote>`

用 `egui::Frame::group()` 包裹，应用 `padding`/`margin`/`background_color`：

```rust
egui::Frame::group(ui.style())
    .inner_margin(padding)
    .outer_margin(margin)
    .fill(background_color)
    .show(ui, |ui| {
        ui.with_layout(align_layout, |ui| {
            for child in &node.children {
                render_node(ui, child);
            }
        });
    })
```

### `<pre>`

类似 code block：`RichText::monospace()` + 等宽字体 + 浅灰背景 Frame。

### `<table>` → `egui::Grid`

按 `tr` 行遍历，`th` 为表头（strong），`td` 为数据单元格，使用 `egui::Grid::new().striped(true)`。

### `<hr>` → `ui.separator()`

### 文本对齐

```
text-align: left   → Layout::left_to_right(Align::Min)
text-align: center → Layout::top_down(Align::Center)
text-align: right  → Layout::right_to_left(Align::Max)
```

---

## 6. 内联渲染 (inline.rs)

### 标签 → RichText 映射

| 标签 | RichText 方法 |
|---|---|
| `<b>` `<strong>` | `.strong()` |
| `<i>` `<em>` | `.italics()` |
| `<u>` `<ins>` | `.underline()` |
| `<del>` `<s>` | `.strikethrough()` |
| `<code>` | `.monospace()` + code 风格 |
| `<mark>` | `.background_color()` (黄色高亮) |
| `<sub>` | 缩小字号 + 下偏移 |
| `<sup>` | 缩小字号 + 上偏移 |
| `<a href>` | `ui.hyperlink_to()` |
| `<br>` | 换行 |
| `<span>` `<font>` | CSS style 属性映射 |
| `<small>` | 缩小字号 (0.85×) |
| `<big>` | 放大字号 (1.2×) |

### CSS style 映射到 RichText

- `color` → `.color(Color32)`
- `background-color` → `.background_color(Color32)`
- `font-size` → `.font(FontId::new(px, family))`

### 内联渲染入口

```rust
pub fn render_inline_html(ui: &mut Ui, html: &str, base_font: &FontId) {
    let nodes = parser::parse_inline_html(html);
    ui.horizontal_wrapped(|ui| {
        for node in &nodes {
            render_inline_node(ui, node, base_font);
        }
    });
}
```

---

## 7. 集成

### 修改 markdown_renderer/src/render.rs

```rust
// Block::HtmlBlock: 原来
ui.label(egui::RichText::new(s).code().weak());

// 改为
html_renderer::render_block_html(ui, s);
```

```rust
// Inline::Html: 原来
ui.label(egui::RichText::new(s).weak().font(font_id.clone()));

// 改为
html_renderer::render_inline_html(ui, s, font_id);
```

### 降级策略

解析失败或标签未识别时，回退到当前行为（显示源码弱化文本）：

```rust
pub fn render_inline_html(ui: &mut Ui, html: &str, base_font: &FontId) {
    match parse_and_render(ui, html, base_font) {
        Ok(_) => {}
        Err(_) => {
            ui.label(RichText::new(html).weak().font(base_font.clone()));
        }
    }
}
```

### 对导出引擎的影响

- **HTML 导出**：`HtmlBlock` 已是原样透传，无需修改 ✅
- **PDF 导出**：`HtmlBlock` 当前被跳过（`_ => {}`），保持不变 ✅

---

## 8. 测试策略

| 层级 | 内容 | 位置 | 数量 |
|---|---|---|---|
| 单元测试 | CSS 属性解析 | `css.rs` | 每个属性 ≥1 条 |
| 单元测试 | 颜色解析（十六进制/rgb/颜色名） | `css.rs` | ≥5 条 |
| 单元测试 | 标签分类 | `parser.rs` | ≥5 条 |
| 单元测试 | HTML → DOM 树结构 | `parser.rs` | ≥5 条 |
| 集成测试 | 内联标签渲染（不需要真正的 egui ctx） | `lib.rs` | 每个标签 ≥1 条 |
| 集成测试 | 块级标签渲染 | `lib.rs` | 每个标签 ≥1 条 |
| 集成测试 | 嵌套标签 | `lib.rs` | ≥3 条 |
| 集成测试 | 降级：空字符串/纯文本/垃圾 HTML | `lib.rs` | ≥3 条 |

覆盖率目标：≥ 80%。

---

## 9. 实现顺序

1. 创建 `crates/html_renderer/` 骨架 + Cargo.toml + workspace member
2. 实现 `css.rs`（CSS 解析器）
3. 实现 `parser.rs`（html5ever 封装）
4. 实现 `inline.rs`（内联标签渲染）
5. 实现 `block.rs`（块级标签渲染）
6. 实现 `lib.rs`（公共 API）
7. 编写测试
8. 修改 `markdown_renderer` 集成
9. 全项目编译 + 测试验证
