# Outline / TOC — Design Spec

**Date**: 2026-06-20
**Branch**: N/A (to be created from main)
**Status**: approved

## 1. Overview

新增 Markdown 编辑器的大纲/目录侧边栏。从 Document AST 提取标题层级（H1-H6），在左侧固定面板显示可点击的层级树，点击跳转到编辑器对应行。

## 2. Architecture

### 2.1 New module: `outline_view.rs`

```
crates/zdown-app/src/outline_view.rs
```

**Data structure**:

```rust
#[derive(Debug, Clone)]
struct OutlineItem {
    /// 标题层级 1-6
    pub level: u8,
    /// 纯文本标题（已去除内联标记）
    pub text: String,
    /// 在源码中的行号（0-based，来自 BlockWithSpan.span.start_line）
    pub line: usize,
}
```

**Public API**:

```rust
/// 在侧边栏渲染大纲面板。
pub fn show_outline_panel(ui: &mut egui::Ui, state: &mut EditorState)
```

**Internal helpers**:

```rust
/// 从 Document AST 提取所有标题
fn extract_outline(doc: &Document) -> Vec<OutlineItem>

/// 将行内节点转换为纯文本（去除 markdown 标记）
fn inlines_to_plain(inlines: &[Inline]) -> String
```

### 2.2 Heading extraction

遍历 `doc.blocks`，筛选 `Block::Heading` 变体：

- `level` = `h.level`
- `text` = `inlines_to_plain(&h.inlines)`
- `line` = `bws.span.start_line`

不需要额外的文本扫描或正则匹配。

### 2.3 Panel rendering

使用 `egui::SidePanel::left` 实现：

- 面板宽度默认 200px，可拖拽调整（`resizable(true)`）
- 可折叠（点击面板标题或设置 `collapsible`）
- 内部 `ScrollArea::vertical` 支持长文档滚动
- 标题项缩进 = `(level - 1) * 16` px
- H1/H2 粗体 14px，H3+ 常规 13px，颜色按层级递减对比度
- 点击标题项 → `state.editor.set_cursor(Cursor::new(line, 0))`
- cursor 变动由已有的 `Editor.set_cursor()` 验证行号有效性

### 2.4 Integration in main.rs

```rust
// 在 show_menu 之后、视图之前插入：
egui::SidePanel::left("outline_panel")
    .resizable(true)
    .default_width(200.0)
    .min_width(60.0)
    .show_inside(ui, |ui| {
        outline_view::show_outline_panel(ui, &mut self.state);
    });

egui::CentralPanel::default().show_inside(ui, |ui| {
    // 现有的视图分发逻辑
    match self.view_mode { ... }
});
```

TopBottomPanel（菜单）→ SidePanel（大纲）+ CentralPanel（视图）。

### 2.5 Edge cases

| Scenario | Behavior |
|----------|----------|
| 空文档 / 无标题 | 面板显示 "（无标题）" |
| 标题文本为空（`###` 无内容） | 显示 "(空标题)" |
| 标题文本非常长 | `RichText` 不做截断，UI 自然溢出（ScrollArea 内允许横向滚动） |
| 非标题的 `#`（如代码块内） | parser 不识别为 Heading，不出现在大纲 |
| 点击不存在的行号 | `set_cursor` 内部验证行号，不会 panic |
| 面板被拖拽到最小宽度 | `min_width(60.0)` 保证可见性 |
| 文档编辑中 | 每帧从 `state.current_doc()` 重新解析，实时更新大纲 |

## 3. File Changes

| File | Change |
|------|--------|
| `crates/zdown-app/src/outline_view.rs` | **New** — extract_outline + inlines_to_plain + show_outline_panel |
| `crates/zdown-app/src/main.rs` | Add `mod outline_view;` + SidePanel::left integration |

No new dependencies. No changes to existing crates.

## 4. Testing

### Unit tests (`outline_view.rs`):

- `extract_outline_empty_doc` — 空文档返回空 Vec
- `extract_outline_single_h1` — 单个 H1 标题，level=1，text 正确
- `extract_outline_multiple_levels` — H1/H2/H3 混合，count 和 order 正确
- `extract_outline_no_headings` — 只有段落无标题，返回空 Vec
- `extract_outline_correct_line_numbers` — 标题在正确行号
- `extract_outline_text_is_plain` — 粗体/斜体/链接等内联标记已被去除
- `extract_outline_empty_heading_text` — 空标题文本返回 "(空标题)"

### Integration tests (via main loop):

- 点击大纲项后 `state.editor.cursor.line` 变为目标行号

## 5. Non-Goals

- 不实现大纲项折叠/展开（全部平铺显示）
- 不支持 Setext 风格标题（`===`/`---`）——由 parser 层负责
- 不显示当前光标位置在大纲中的同步高亮（后续迭代）
- 不在大纲面板中显示非 Heading 元素
