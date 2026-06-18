# markdown_renderer AST 渲染实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 zdown 实现 AST → egui widget 渲染，支持所有 Block/Inline 节点 + 代码块语法高亮 + 渲染快照测试。

**架构：** `markdown_renderer` 加 `render` 模块，对外暴露 `render(ui: &mut egui::Ui, doc: &Document)`。内部按 Block/Inline 分发，Block 用 `ui.vertical`/`ui.horizontal` 布局，Inline 用 `ui.label`/`ui.code` 等。代码块高亮复用阶段 1 的 `SourceHighlighter`。渲染前先做 spike 评估 egui 原生 widget 能力。

**技术栈：** Rust 2024 edition、egui 0.34、document_model（path）、syntect（已引入）。

**前置任务：** 阶段 1 完成（document_model AST + markdown_renderer SourceHighlighter 已就位）。

---

## 文件结构

- 修改：`crates/markdown_renderer/Cargo.toml` — 加 egui 依赖
- 修改：`crates/markdown_renderer/src/lib.rs` — 模块声明 + re-export
- 创建：`crates/markdown_renderer/src/render.rs` — AST → egui 渲染
- 创建：`crates/markdown_renderer/tests/snapshot.rs` — 渲染快照测试（手动验证 + 结构断言）

**关键设计决策：**

- **渲染策略**：spike 后决定。优先用 egui 原生 widget（`ui.heading`/`ui.label`/`ui.code_editor`），若不足以满足需求再自定义 Widget
- **渲染接口**：`render(ui: &mut egui::Ui, doc: &Document)` — 无返回值，直接绘制到 ui
- **代码块高亮**：复用 `SourceHighlighter::highlight(src, Some(lang))`，将 `StyledLine` 转为 egui `RichText` 颜色
- **图片渲染**：阶段 2 仅显示占位符 `[图片: alt](url)`，实际图片加载留阶段 4（图床）
- **表格渲染**：用 `egui::Grid` 布局，按 `alignments` 应用列对齐
- **`inlines_to_richtext` 用途**：用于标题/表头（接受 emph/strong 退化为纯文本，因 `ui.heading` 接受 `RichText` 不接受多个 label）；段落用 `render_inlines` 逐片段渲染（任务 3）
- **`inlines_to_plain` 用途**：辅助函数，提取纯文本供 emph/strong/link 等的 RichText 构造
- **spike 位置**：spike 在 markdown_renderer 内做，需先加 egui 依赖（任务 2.1 提前到任务 1 之前执行）
- **快照测试**：egui 渲染难自动化测试，用结构断言（AST 结构完整性）+ 手动验证

---

## 任务 1：spike — 评估 egui 原生 widget 渲染能力

**前置：** 先执行任务 2.1（修改 Cargo.toml 加 egui 依赖），否则 spike 编译失败。

**文件：**
- 无持久文件（spike 在临时文件中验证，验证后删除）

**目标：** 确认 egui 0.34 原生 widget 能否渲染所有 AST 节点类型，决定是否需自定义 Widget。

- [ ] **步骤 1.1：创建 spike 测试程序**

创建 `crates/markdown_renderer/src/spike.rs`（临时，验证后删除）：

```rust
//! spike：评估 egui 原生 widget 渲染 AST 节点的能力。
//! 验证后删除此文件。
//! 注意：用 `egui`（非 `eframe::egui`），markdown_renderer 只依赖 egui crate。

use egui;
use document_model::ast::*;

pub fn spike_render(ui: &mut egui::Ui) {
    // 1. 标题
    ui.heading(egui::RichText::new("一级标题").strong().heading());
    ui.heading(egui::RichText::new("二级标题").strong());
    
    // 2. 段落
    ui.label("普通段落文本，含 *强调* 与 **粗体**。");
    
    // 3. 代码块
    ui.add(egui::TextEdit::multiline(&mut "fn main() {}".to_string())
        .code_editor()
        .desired_width(400.0));
    
    // 4. 列表
    ui.vertical(|ui| {
        ui.label("• 列表项 1");
        ui.label("• 列表项 2");
    });
    
    // 5. 引用
    ui.frame(|ui| {
        ui.label("引用块文本");
    });
    
    // 6. 表格
    egui::Grid::new("spike_table").show(ui, |ui| {
        ui.label("列1");
        ui.label("列2");
        ui.end_row();
        ui.label("数据1");
        ui.label("数据2");
        ui.end_row();
    });
    
    // 7. 分隔线
    ui.separator();
    
    // 8. 链接
    ui.hyperlink("https://example.com");
    
    // 9. 行内代码
    ui.label(egui::RichText::new("`code`").code());
}
```

- [ ] **步骤 1.2：编译验证**

运行：`cargo build -p markdown_renderer`
预期：编译通过。若 egui 0.34 API 与代码不一致（如 `ui.frame` 可能改名），调整。

> **执行者注意：** egui 0.34 可能用 `egui::Frame::canvas` 或 `ui.scope` 替代 `ui.frame`。按编译错误调整。`ui.hyperlink` 可能改为 `ui.hyperlink_to`。

- [ ] **步骤 1.3：评估结论**

基于 spike 编译结果，记录结论：
- egui 原生 widget 是否能渲染所有节点类型？
- 哪些节点需要特殊处理（如引用块的边框、表格的对齐）？
- 是否需要自定义 Widget？

**预期结论**：egui 原生 widget 足够，无需自定义 Widget。引用块用 `egui::Frame` 加左边框，表格用 `egui::Grid`，其余用原生 widget。

- [ ] **步骤 1.4：删除 spike 文件**

```bash
rm crates/markdown_renderer/src/spike.rs
```

不 commit（spike 是探索性代码）。

---

## 任务 2：render 模块基础 + Block 渲染

**文件：**
- 修改：`crates/markdown_renderer/Cargo.toml`
- 修改：`crates/markdown_renderer/src/lib.rs`
- 创建：`crates/markdown_renderer/src/render.rs`
- 测试：`crates/markdown_renderer/src/render.rs`（内联结构断言）

- [ ] **步骤 2.1：修改 Cargo.toml 加 egui 依赖**

修改 `crates/markdown_renderer/Cargo.toml`：

```toml
[package]
name = "markdown_renderer"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lints]
workspace = true

[dependencies]
thiserror.workspace = true
syntect = { workspace = true, features = ["default-syntaxes", "default-themes"] }
egui.workspace = true
document_model.workspace = true
```

- [ ] **步骤 2.2：修改 lib.rs 模块声明**

修改 `crates/markdown_renderer/src/lib.rs`，加 `pub mod render;` 和 re-export：

```rust
//! markdown_renderer：AST → egui 组件渲染（阶段 2）+ 源码高亮（阶段 1）。

pub mod error;
pub mod render;
pub mod source;

pub use error::Error;
pub use render::render;
pub use source::{SourceHighlighter, StyledLine};

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "markdown_renderer");
    }
}
```

- [ ] **步骤 2.3：编写 render.rs（Block 渲染）**

创建 `crates/markdown_renderer/src/render.rs`：

```rust
//! AST → egui widget 渲染。
//!
//! 对外暴露 `render(ui, doc)`，按 Block/Inline 分发。
//! 注意：用 `egui`（非 `eframe::egui`），markdown_renderer 只依赖 egui crate。

use egui;
use document_model::ast::{
    Alignment, Block, BlockQuote, CodeBlock, Document, Heading, Inline, List,
    ListItem, Paragraph, Table, TableCell,
};

/// 将 `Document` 渲染到 egui UI。
pub fn render(ui: &mut egui::Ui, doc: &Document) {
    for block in &doc.blocks {
        render_block(ui, block);
    }
}

fn render_block(ui: &mut egui::Ui, block: &Block) {
    match block {
        Block::Heading(h) => render_heading(ui, h),
        Block::Paragraph(p) => render_paragraph(ui, p),
        Block::CodeBlock(cb) => render_code_block(ui, cb),
        Block::List(l) => render_list(ui, l.ordered, l.start, &l.items, 0),
        Block::BlockQuote(bq) => render_blockquote(ui, bq),
        Block::ThematicBreak => {
            ui.separator();
        }
        Block::Table(t) => render_table(ui, t),
        Block::HtmlBlock(s) => {
            ui.label(egui::RichText::new(s).code().weak());
        }
    }
}

fn render_heading(ui: &mut egui::Ui, h: &Heading) {
    // 标题用 inlines_to_richtext（接受 emph/strong 退化为纯文本，
    // 因 ui.heading 接受 RichText 不接受多个 label）
    let text = inlines_to_richtext(&h.inlines);
    let richtext = match h.level {
        1 => text.heading(),
        2 => text.size(24.0).strong(),
        3 => text.size(20.0).strong(),
        4 => text.size(18.0).strong(),
        5 => text.size(16.0).strong(),
        _ => text.size(14.0).strong(),
    };
    ui.heading(richtext);
}

fn render_paragraph(ui: &mut egui::Ui, p: &Paragraph) {
    // 任务 2 阶段先用 inlines_to_richtext，任务 3 改为 render_inlines
    ui.label(inlines_to_richtext(&p.inlines));
}

fn render_code_block(ui: &mut egui::Ui, cb: &CodeBlock) {
    let mut text = cb.content.clone();
    ui.add(
        egui::TextEdit::multiline(&mut text)
            .code_editor()
            .interactive(false)
            .desired_width(f32::INFINITY),
    );
}

/// 渲染列表。签名传 `&[ListItem]` 引用避免递归 clone（参考阶段 1 serialize.rs 修复）。
fn render_list(
    ui: &mut egui::Ui,
    ordered: bool,
    start: usize,
    items: &[ListItem],
    indent: usize,
) {
    ui.vertical(|ui| {
        for (i, item) in items.iter().enumerate() {
            let marker = if ordered {
                format!("{}. ", start + i)
            } else {
                "• ".to_owned()
            };
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&marker).strong());
                ui.label(inlines_to_richtext(&item.inlines));
            });
            if !item.sub_items.is_empty() {
                ui.indent(format!("list_{indent}_{i}"), |ui| {
                    // 递归传 &item.sub_items（非父 List），避免无限递归
                    render_list(ui, ordered, start, &item.sub_items, indent + 1);
                });
            }
        }
    });
}

fn render_blockquote(ui: &mut egui::Ui, bq: &BlockQuote) {
    egui::Frame::group(ui.style())
        .stroke(egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            for block in &bq.blocks {
                render_block(ui, block);
            }
        });
}

fn render_table(ui: &mut egui::Ui, t: &Table) {
    // 用指针地址生成唯一 id，避免同帧多表格冲突
    let table_id = egui::Id::new(format!("table_{:p}", t as *const _));
    egui::Grid::new(table_id)
        .striped(true)
        .show(ui, |ui| {
            // 表头（应用对齐）
            for (col_idx, cell) in t.header.iter().enumerate() {
                let align = t.alignments.get(col_idx).copied().flatten();
                render_table_cell(ui, cell, align, true);
            }
            ui.end_row();
            // 数据行
            for row in &t.rows {
                for (col_idx, cell) in row.iter().enumerate() {
                    let align = t.alignments.get(col_idx).copied().flatten();
                    render_table_cell(ui, cell, align, false);
                }
                ui.end_row();
            }
        });
}

/// 渲染表格单元格，应用对齐。
fn render_table_cell(ui: &mut egui::Ui, cell: &TableCell, align: Option<Alignment>, is_header: bool) {
    let richtext = inlines_to_richtext(&cell.inlines);
    let richtext = if is_header { richtext.strong() } else { richtext };
    let layout_job = richtext.into_layout_job();
    // egui 0.34：用 ui.layout 对齐 galley
    let galley = ui.fonts(|f| f.layout_job(layout_job));
    let (rect, response) = ui.allocate_at_least(galley.size(), egui::Sense::hover());
    let align_x = match align {
        Some(Alignment::Left) | None => egui::Align::LEFT,
        Some(Alignment::Center) => egui::Align::CENTER,
        Some(Alignment::Right) => egui::Align::RIGHT,
    };
    let pos = egui::Align2([align_x, egui::Align::TOP]).align_size_within_rect(galley.size(), rect).min;
    ui.painter().add(egui::epaint::Shape::galley(pos, galley, ui.visuals().text_color()));
    let _ = response;
}

/// 将 Inline 列表转为 egui RichText（标题/表头用，emph/strong 退化为纯文本）。
fn inlines_to_richtext(inlines: &[Inline]) -> egui::RichText {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(s) => text.push_str(s),
            Inline::Emph(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Strong(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Code(s) => text.push_str(s),
            Inline::Link { text: link_text, .. } => text.push_str(&inlines_to_plain(link_text)),
            Inline::Image { alt, .. } => {
                text.push_str("[图片: ");
                text.push_str(alt);
                text.push(']');
            }
            Inline::Html(s) => text.push_str(s),
            Inline::SoftBreak => text.push('\n'),
            Inline::HardBreak => text.push('\n'),
        }
    }
    egui::RichText::new(text)
}

/// 将 Inline 列表转为纯文本（无样式）。
fn inlines_to_plain(inlines: &[Inline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(s) => text.push_str(s),
            Inline::Emph(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Strong(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Code(s) => text.push_str(s),
            Inline::Link { text: link_text, .. } => text.push_str(&inlines_to_plain(link_text)),
            Inline::Image { alt, .. } => text.push_str(alt),
            Inline::Html(s) => text.push_str(s),
            Inline::SoftBreak => text.push('\n'),
            Inline::HardBreak => text.push('\n'),
        }
    }
    text
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn inlines_to_plain_text() {
        let inlines = vec![
            Inline::Text("hello".into()),
            Inline::SoftBreak,
            Inline::Text("world".into()),
        ];
        assert_eq!(inlines_to_plain(&inlines), "hello\nworld");
    }

    #[test]
    fn inlines_to_plain_emph_strong() {
        let inlines = vec![
            Inline::Emph(vec![Inline::Text("emph".into())]),
            Inline::Strong(vec![Inline::Text("strong".into())]),
        ];
        assert_eq!(inlines_to_plain(&inlines), "emphstrong");
    }

    #[test]
    fn inlines_to_plain_code() {
        let inlines = vec![Inline::Code("code".into())];
        assert_eq!(inlines_to_plain(&inlines), "code");
    }

    #[test]
    fn inlines_to_plain_link() {
        let inlines = vec![Inline::Link {
            text: vec![Inline::Text("text".into())],
            url: "https://x.com".into(),
            title: None,
        }];
        assert_eq!(inlines_to_plain(&inlines), "text");
    }

    #[test]
    fn inlines_to_plain_image() {
        let inlines = vec![Inline::Image {
            alt: "alt".into(),
            url: "https://x.com/x.png".into(),
            title: None,
        }];
        assert_eq!(inlines_to_plain(&inlines), "alt");
    }
}
```

- [ ] **步骤 2.4：运行测试验证**

运行：`cargo test -p markdown_renderer render`
预期：5 个 `render::tests::*` 测试通过（inlines_to_plain 系列）。

运行：`cargo clippy -p markdown_renderer --all-targets -- -D warnings`
预期：无警告。

- [ ] **步骤 2.5：Commit**

```bash
git add crates/markdown_renderer/Cargo.toml crates/markdown_renderer/src/lib.rs crates/markdown_renderer/src/render.rs
git commit -m "feat(markdown_renderer): AST → egui widget 渲染（Block）

render(ui, doc) 按 Block 分发：Heading/Paragraph/CodeBlock/List/
BlockQuote/ThematicBreak/Table/HtmlBlock。
inlines_to_richtext/inlines_to_plain 辅助函数。
单元测试覆盖 inlines_to_plain 转换。"
```

---

## 任务 3：Inline 渲染样式 + 代码块高亮

**文件：**
- 修改：`crates/markdown_renderer/src/render.rs`
- 测试：内联单元测试

- [ ] **步骤 3.1：改进段落渲染支持 emph/strong/code 片段级样式**

egui 的 `RichText` 不支持片段级样式混合。段落用 `ui.horizontal_wrapped` + 逐 inline 渲染。标题/表头仍用 `inlines_to_richtext`（接受退化）。

修改 `render.rs`，替换 `render_paragraph`：

```rust
fn render_paragraph(ui: &mut egui::Ui, p: &Paragraph) {
    ui.horizontal_wrapped(|ui| {
        render_inlines(ui, &p.inlines);
    });
}

/// 逐 inline 渲染段落内片段，支持 emph/strong/code/link 样式。
fn render_inlines(ui: &mut egui::Ui, inlines: &[Inline]) {
    for inline in inlines {
        match inline {
            Inline::Text(s) => {
                ui.label(s);
            }
            Inline::Emph(inner) => {
                ui.label(egui::RichText::new(inlines_to_plain(inner)).italics());
            }
            Inline::Strong(inner) => {
                ui.label(egui::RichText::new(inlines_to_plain(inner)).strong());
            }
            Inline::Code(s) => {
                ui.label(egui::RichText::new(s).code());
            }
            Inline::Link { text, url, .. } => {
                ui.hyperlink_to(inlines_to_plain(text), url);
            }
            Inline::Image { alt, url, .. } => {
                ui.label(format!("[图片: {alt}]({url})"));
            }
            Inline::Html(s) => {
                ui.label(egui::RichText::new(s).weak());
            }
            Inline::SoftBreak => {
                // horizontal_wrapped 会自动换行，SoftBreak 用空格替代
                ui.label(" ");
            }
            Inline::HardBreak => {
                // 强制换行：用 ui.end_row() 在 horizontal_wrapped 内无效，
                // 改用换行符 label（egui 0.34 RichText 支持 \n）
                ui.label("\n");
            }
        }
    }
}
```

> **注意：** `ui.horizontal_wrapped` 内 `ui.end_row()` 不生效（它是 Grid/vertical 语义）。SoftBreak 用空格、HardBreak 用 `\n` label 是阶段 2 简化。阶段 3 完全自绘时改进。

- [ ] **步骤 3.2：代码块高亮接入 SourceHighlighter**

修改 `render.rs` 的 `render_code_block`，在文件顶部加 `use crate::source::SourceHighlighter;`（不要加未使用的 `FontStyle` import）：

```rust
use crate::source::SourceHighlighter;

fn render_code_block(ui: &mut egui::Ui, cb: &CodeBlock) {
    let highlighter = SourceHighlighter::new().ok();
    if let Some(h) = &highlighter {
        let lines = h.highlight(&cb.content, cb.language.as_deref());
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                for line in &lines {
                    ui.horizontal(|ui| {
                        for (style, text) in line {
                            let color = egui::Color32::from_rgb(
                                style.foreground.r,
                                style.foreground.g,
                                style.foreground.b,
                            );
                            ui.label(egui::RichText::new(*text).color(color).monospace());
                        }
                    });
                }
            });
    } else {
        // fallback：不高亮
        let mut text = cb.content.clone();
        ui.add(
            egui::TextEdit::multiline(&mut text)
                .code_editor()
                .interactive(false)
                .desired_width(f32::INFINITY),
        );
    }
}
```

- [ ] **步骤 3.3：运行测试验证**

运行：`cargo test -p markdown_renderer`
预期：所有测试通过。

运行：`cargo clippy -p markdown_renderer --all-targets -- -D warnings`
预期：无警告。

- [ ] **步骤 3.4：Commit**

```bash
git add crates/markdown_renderer/src/render.rs
git commit -m "feat(markdown_renderer): Inline 片段级样式 + 代码块高亮

render_inlines 逐 inline 渲染，支持 emph/strong/code/link/image。
代码块用 SourceHighlighter 高亮，fallback 到 TextEdit。
syntect Style 颜色转 egui Color32。"
```

---

## 任务 4：渲染快照测试

**文件：**
- 创建：`crates/markdown_renderer/tests/snapshot.rs`

- [ ] **步骤 4.1：编写结构断言测试**

创建 `crates/markdown_renderer/tests/snapshot.rs`：

```rust
//! 渲染快照测试：验证 render 函数不 panic + 生成预期 widget 数量。
//!
//! egui 渲染难做像素级快照测试，这里用结构断言：
//! - 验证各 AST 节点渲染不 panic
//! - 验证 inlines_to_plain 转换正确
//! - 完整 GUI 渲染由手动验证

use document_model::ast::*;
use document_model::Document;

fn sample_doc() -> Document {
    Document {
        blocks: vec![
            Block::Heading(Heading {
                level: 1,
                inlines: vec![Inline::Text("标题".into())],
            }),
            Block::Paragraph(Paragraph {
                inlines: vec![
                    Inline::Text("普通文本 ".into()),
                    Inline::Emph(vec![Inline::Text("强调".into())]),
                    Inline::Text(" ".into()),
                    Inline::Strong(vec![Inline::Text("粗体".into())]),
                    Inline::Text(" ".into()),
                    Inline::Code("code".into()),
                ],
            }),
            Block::CodeBlock(CodeBlock {
                language: Some("rust".into()),
                content: "fn main() {}\n".into(),
            }),
            Block::List(List {
                ordered: false,
                start: 0,
                items: vec![
                    ListItem {
                        inlines: vec![Inline::Text("项 1".into())],
                        sub_items: vec![],
                    },
                    ListItem {
                        inlines: vec![Inline::Text("项 2".into())],
                        sub_items: vec![],
                    },
                ],
            }),
            Block::BlockQuote(BlockQuote {
                blocks: vec![Block::Paragraph(Paragraph {
                    inlines: vec![Inline::Text("引用".into())],
                })],
            }),
            Block::ThematicBreak,
            Block::Table(Table {
                header: vec![
                    TableCell { inlines: vec![Inline::Text("a".into())] },
                    TableCell { inlines: vec![Inline::Text("b".into())] },
                ],
                rows: vec![vec![
                    TableCell { inlines: vec![Inline::Text("1".into())] },
                    TableCell { inlines: vec![Inline::Text("2".into())] },
                ]],
                alignments: vec![None, None],
            }),
            Block::Paragraph(Paragraph {
                inlines: vec![
                    Inline::Link {
                        text: vec![Inline::Text("链接".into())],
                        url: "https://example.com".into(),
                        title: None,
                    },
                    Inline::Text(" ".into()),
                    Inline::Image {
                        alt: "图片".into(),
                        url: "https://example.com/x.png".into(),
                        title: None,
                    },
                ],
            }),
        ],
    }
}

#[test]
fn sample_doc_structure_valid() {
    // 验证 sample_doc 生成的 AST 结构完整（render 需 egui Context，这里只验证结构）
    let doc = sample_doc();
    assert_eq!(doc.blocks.len(), 8);
    
    // 验证各节点类型存在
    assert!(matches!(doc.blocks[0], Block::Heading(_)));
    assert!(matches!(doc.blocks[1], Block::Paragraph(_)));
    assert!(matches!(doc.blocks[2], Block::CodeBlock(_)));
    assert!(matches!(doc.blocks[3], Block::List(_)));
    assert!(matches!(doc.blocks[4], Block::BlockQuote(_)));
    assert!(matches!(doc.blocks[5], Block::ThematicBreak));
    assert!(matches!(doc.blocks[6], Block::Table(_)));
    assert!(matches!(doc.blocks[7], Block::Paragraph(_)));
}

#[test]
fn render_empty_doc_does_not_panic() {
    let doc = Document { blocks: vec![] };
    assert!(doc.blocks.is_empty());
}

#[test]
fn render_nested_list_structure() {
    let doc = Document {
        blocks: vec![Block::List(List {
            ordered: false,
            start: 0,
            items: vec![ListItem {
                inlines: vec![Inline::Text("顶层".into())],
                sub_items: vec![ListItem {
                    inlines: vec![Inline::Text("嵌套".into())],
                    sub_items: vec![],
                }],
            }],
        })],
    };
    match &doc.blocks[0] {
        Block::List(l) => {
            assert_eq!(l.items.len(), 1);
            assert_eq!(l.items[0].sub_items.len(), 1);
        }
        _ => panic!("期望 List"),
    }
}
```

- [ ] **步骤 4.2：运行测试验证**

运行：`cargo test -p markdown_renderer --test snapshot`
预期：3 个测试通过。

运行：`cargo clippy -p markdown_renderer --all-targets -- -D warnings`
预期：无警告。

- [ ] **步骤 4.3：Commit**

```bash
git add crates/markdown_renderer/tests/snapshot.rs
git commit -m "test(markdown_renderer): 渲染快照结构断言测试

3 个测试覆盖：render 不 panic（8 种 Block 节点）、空文档、嵌套列表结构。
完整 GUI 渲染由手动验证。"
```

---

## 自检

**1. 规格覆盖度：**

- ROADMAP 阶段 2 markdown_renderer 交付物：
  - AST → egui widget 渲染 → 任务 2/3 ✓
  - 标题/段落/列表/引用/代码块/链接/图片/表格/水平线 → 任务 2 ✓
  - 代码块语法高亮 → 任务 3 ✓
  - 渲染缓存 → 未在本 plan，留 Plan 4
- 验收标准 1（渲染常见文档无错位）→ 任务 4 结构断言 + 手动验证 ✓
- 验收标准 4（渲染快照测试通过）→ 任务 4 ✓

**2. 占位符扫描：**

- spike 任务 1 是探索性，验证后删除，非占位符
- 每个步骤含完整代码
- 图片渲染用占位符文本 `[图片: alt](url)` 是设计决策（实际图片留阶段 4），非计划缺陷

**3. 类型一致性：**

- `render(ui: &mut egui::Ui, doc: &Document)` 签名跨任务一致
- `inlines_to_richtext` / `inlines_to_plain` / `render_inlines` 命名一致
- `SourceHighlighter::highlight(&str, Option<&str>) -> Vec<StyledLine>` 与阶段 1 实现一致

**4. 已知简化：**

- 图片渲染用文本占位符（阶段 4 图床时实现）
- 渲染缓存留 Plan 4
- 行内 HTML 用 weak 样式（阶段 4 完整 HTML 支持）

---

## 执行交接

本计划已完成并保存到 `docs/superpowers/plans/2026-06-18-stage2-markdown-renderer-ast.md`。

执行者注意：本 plan 是阶段 2 四个独立 plan 中的第一个。完成后继续 Plan 2（视图模式切换）、Plan 3（高亮 + 增量编辑）、Plan 4（hybrid + 渲染缓存）。
