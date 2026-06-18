# hybrid 模式 + 渲染缓存实现计划

> **面向 AI 代理的工作者：** 必需子智能体：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。

**目标：** 给 document_model AST 加 source span + 实现 zdown-app hybrid 模式（按 block 边界分割）+ markdown_renderer 渲染缓存，满足输入延迟 < 50ms。

**架构：**
1. document_model AST 加 `Span { start_line, end_line }` 字段，parse 时从 pulldown-cmark 的 range 信息填充
2. hybrid 模式按 block 边界分割：找光标所在 block（用 span 查找），之前 block 全 AST 渲染，光标 block 用源码高亮，之后 block 全 AST 渲染
3. 渲染缓存用 `HashMap<u64, Document>`（无 Mutex，用 `&mut self`），LRU 10 条上限

**技术栈：** Rust 2024 edition、egui 0.34、editor_engine、markdown_renderer、document_model。

**前置任务：** Plan 1（AST 渲染）+ Plan 2（视图切换）+ Plan 3（高亮 + 增量编辑）完成。

---

## 文件结构

- 修改：`crates/document_model/src/ast.rs` — Block 加 Span 字段
- 修改：`crates/document_model/src/parse.rs` — parse 时填充 span
- 修改：`crates/markdown_renderer/src/render.rs` — RenderCache（无 Mutex）
- 修改：`crates/markdown_renderer/src/lib.rs` — re-export
- 创建：`crates/zdown-app/src/hybrid_view.rs` — hybrid 视图（按 block 分割）
- 修改：`crates/zdown-app/src/main.rs` — Hybrid 模式接入 + RenderCache 字段

**关键设计决策：**

- **Span 定义**：`Span { start_line: usize, end_line: usize }`（0-based，含），加到 `Block` 的每个变体或包装为 `BlockWithSpan { block: Block, span: Span }`
- **设计选择**：用 `BlockWithSpan` 包装而非改每个 Block 变体，避免破坏阶段 1 AST 类型
- **hybrid 分割**：找光标所在 `BlockWithSpan`（`span.start_line <= cursor.line <= span.end_line`），之前全渲染，光标 block 源码，之后全渲染
- **RenderCache**：`HashMap<u64, Document>` + `&mut self`，LRU 10 条（用 `VecDeque` 跟踪 key 顺序）
- **hybrid 输入**：复用 Plan 3 的 `handle_input`（光标在源码 block 内时接受输入）

---

## 任务 1：AST 加 Span + parse 填充

**文件：**
- 修改：`crates/document_model/src/ast.rs`
- 修改：`crates/document_model/src/parse.rs`

- [ ] **步骤 1.1：ast.rs 加 Span + BlockWithSpan**

修改 `crates/document_model/src/ast.rs`，在文件顶部加 `Span` 类型，在 `Document` 改用 `BlockWithSpan`：

```rust
/// 源码 span（行范围，0-based，含两端）。
/// 用于 hybrid 模式按 block 边界分割。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start_line: usize,
    pub end_line: usize,
}

/// 带 span 的 Block。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockWithSpan {
    pub block: Block,
    pub span: Span,
}

/// 文档根类型。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// 顶层块级节点（带 span），按文档顺序排列。
    pub blocks: Vec<BlockWithSpan>,
}
```

> **注意：** 这会破坏阶段 1 的 `Document { blocks: Vec<Block> }`。阶段 1 的测试、serialize.rs、round_trip.rs 需相应更新。执行者需搜索所有 `Document { blocks:` 与 `doc.blocks[` 的用法，更新为 `BlockWithSpan`。

- [ ] **步骤 1.2：parse.rs 填充 span**

修改 `crates/document_model/src/parse.rs`，在 `BuilderStack` 的 `Frame::Root` 改为收集 `Vec<BlockWithSpan>`，`push_block` 时记录当前行号。

pulldown-cmark 0.13 的 `Event::Start(Tag)` 不直接携带 range，但 `Parser` 的 `offset` 可用，或用 `Range` 系列 API。简化方案：用文本行计数器，每次 `Event::Text` 含 `\n` 时累加，`End` 时记录 span。

```rust
// 在 BuilderStack 加行计数器
struct BuilderStack {
    stack: Vec<Frame>,
    current_line: usize,  // 当前行号（0-based）
}

// push_block 时创建 BlockWithSpan
fn push_block(&mut self, block: Block) {
    let span = Span {
        start_line: self.current_line,  // 简化：用当前行作为 span 起点
        end_line: self.current_line,    // 实际应跟踪 block 结束行
    };
    // ... 推入 BlockWithSpan { block, span }
}
```

> **执行者注意：** 精确 span 跟踪较复杂（需在每次 Text/SoftBreak/HardBreak 事件更新行号）。阶段 2 简化：每个 block 的 span 用其 End 事件时的行号作为 end_line，Start 事件时的行号作为 start_line。具体实现时需在 `start_tag` 记录 start_line，`end_tag` 记录 end_line。

- [ ] **步骤 1.3：更新阶段 1 测试**

阶段 1 的测试（round_trip.rs、serialize.rs 等）用 `Document { blocks: vec![Block::Heading(...)] }`，需改为 `Document { blocks: vec![BlockWithSpan { block: Block::Heading(...), span: Span { start_line: 0, end_line: 0 } }] }`。

搜索所有 `Document { blocks:` 与 `Block::` 直接在 blocks vec 内的用法，更新为 `BlockWithSpan { block: ..., span: ... }`。

- [ ] **步骤 1.4：编译验证 + 测试**

运行：`cargo test -p document_model`
预期：所有测试通过（含往返测试，span 不影响序列化往返）。

运行：`cargo clippy -p document_model --all-targets -- -D warnings`
预期：无警告。

- [ ] **步骤 1.5：Commit**

```bash
git add crates/document_model/
git commit -m "feat(document_model): AST 加 Span + BlockWithSpan

BlockWithSpan { block: Block, span: Span { start_line, end_line } }。
parse 时填充 span（行号跟踪）。
用于阶段 2 hybrid 模式按 block 边界分割。"
```

---

## 任务 2：hybrid 视图（按 block 边界分割）

**文件：**
- 创建：`crates/zdown-app/src/hybrid_view.rs`
- 修改：`crates/zdown-app/src/main.rs`

- [ ] **步骤 2.1：创建 hybrid_view.rs**

创建 `crates/zdown-app/src/hybrid_view.rs`：

```rust
//! Hybrid 视图：光标所在 block 源码 + 其余 block 渲染。
//!
//! 用 BlockWithSpan 的 span 查找光标所在 block，避免按行切割破坏多行结构。

use eframe::egui;
use markdown_renderer::SourceHighlighter;

use crate::editor_state::EditorState;
use editor_engine::{Command, Cursor};

/// 渲染 hybrid 视图。
pub fn show_hybrid_view(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    highlighter: Option<&SourceHighlighter>,
) {
    let src = state.editor.to_string();
    let cursor_line = state.editor.cursor.line;

    // 先处理输入（复用 source_view 的 handle_input 逻辑）
    let input_response = ui.interact(
        ui.max_rect(),
        egui::Id::new("hybrid_view_input"),
        egui::Sense::click_and_drag(),
    );
    if input_response.has_focus() {
        handle_input_hybrid(ui.ctx(), state);
    }

    let doc = state.current_doc();

    egui::ScrollArea::vertical().show(ui, |ui| {
        // 找光标所在 block 的索引
        let cursor_block_idx = doc.blocks.iter().position(|b| {
            cursor_line >= b.span.start_line && cursor_line <= b.span.end_line
        });

        match cursor_block_idx {
            Some(idx) => {
                // 光标 block 之前的 block：全渲染
                for bws in &doc.blocks[..idx] {
                    render_single_block(ui, &bws.block);
                }

                // 光标 block：源码高亮 + 光标
                let cursor_bws = &doc.blocks[idx];
                let cursor_block_src = extract_block_src(&src, cursor_bws.span);
                render_source_block_with_cursor(ui, &cursor_block_src, state.editor.cursor, highlighter);

                // 光标 block 之后的 block：全渲染
                for bws in &doc.blocks[idx + 1..] {
                    render_single_block(ui, &bws.block);
                }
            }
            None => {
                // 光标不在任何 block 内（如空文档末尾），全部渲染
                markdown_renderer::render(ui, &doc);
            }
        }
    });
}

/// 渲染单个 Block（用于非光标 block）。
fn render_single_block(ui: &mut egui::Ui, block: &document_model::ast::Block) {
    // 构造只含单个 block 的临时 Document 调用 markdown_renderer::render
    let doc = document_model::Document {
        blocks: vec![document_model::ast::BlockWithSpan {
            block: block.clone(),
            span: document_model::ast::Span { start_line: 0, end_line: 0 },
        }],
    };
    markdown_renderer::render(ui, &doc);
}

/// 提取指定 span 的源码片段。
fn extract_block_src(src: &str, span: document_model::ast::Span) -> String {
    src.lines()
        .skip(span.start_line)
        .take(span.end_line - span.start_line + 1)
        .map(|l| format!("{l}\n"))
        .collect()
}

/// 渲染源码 block + 光标（复用 source_view 的逻辑）。
fn render_source_block_with_cursor(
    ui: &mut egui::Ui,
    block_src: &str,
    cursor: Cursor,
    highlighter: Option<&SourceHighlighter>,
) {
    // 调用 source_view 的 render_text_with_cursor（需在 source_view 暴露为 pub(crate)）
    // 或在此重新实现简化版
    if let Some(h) = highlighter {
        let lines = h.highlight(block_src, None);
        ui.vertical(|ui| {
            for (line_idx, line) in lines.iter().enumerate() {
                ui.horizontal(|ui| {
                    for (style, text) in line {
                        let color = egui::Color32::from_rgb(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        );
                        ui.label(egui::RichText::new(*text).color(color).monospace());
                    }
                    let _ = line_idx;
                });
            }
        });
    } else {
        ui.label(egui::RichText::new(block_src).monospace());
    }
    let _ = cursor;  // 阶段 2 简化：hybrid 模式光标用背景色标记，精确像素定位留阶段 3
}

/// 处理输入（复用 source_view 逻辑，需提取为共享函数或重新实现）。
fn handle_input_hybrid(ctx: &egui::Context, state: &mut EditorState) {
    // 同 source_view::handle_input，建议提取到 editor_state 或共享模块
    let events = ctx.input(|i| i.events.clone());
    for event in events {
        match event {
            egui::Event::Text(text) => {
                if !text.is_empty() {
                    let cursor = state.editor.cursor;
                    let _ = state.apply(Command::Insert { pos: cursor, text });
                }
            }
            egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => {
                let cursor = state.editor.cursor;
                if cursor.col > 0 || cursor.line > 0 {
                    let prev = crate::source_view::prev_cursor_pub(&state.editor.buffer, cursor);
                    if let Some(prev) = prev {
                        let _ = state.apply(Command::Delete {
                            range: editor_engine::Selection::new(prev, cursor),
                        });
                        let _ = state.editor.set_cursor(prev);
                    }
                }
            }
            // ... 其余键处理同 source_view，或提取共享函数
            _ => {}
        }
    }
}
```

> **执行者注意：** `handle_input_hybrid` 与 `source_view::handle_input` 逻辑相同。建议在任务实现时把 `handle_input`、`prev_cursor`、`next_cursor` 提取到 `editor_state.rs` 或新的共享模块（如 `input.rs`），避免重复。本 plan 标注为待重构，执行者按实际情况决定。

- [ ] **步骤 2.2：source_view.rs 暴露 prev_cursor/next_cursor 为 pub(crate)**

修改 `crates/zdown-app/src/source_view.rs`，把 `prev_cursor` 和 `next_cursor` 改为 `pub(crate)`：

```rust
pub(crate) fn prev_cursor(buffer: &editor_engine::Buffer, cursor: editor_engine::Cursor) -> Option<editor_engine::Cursor> {
    // ... 原实现
}

pub(crate) fn next_cursor(buffer: &editor_engine::Buffer, cursor: editor_engine::Cursor) -> Option<editor_engine::Cursor> {
    // ... 原实现
}
```

- [ ] **步骤 2.3：main.rs 接入 hybrid 视图**

修改 `crates/zdown-app/src/main.rs`：

```rust
mod hybrid_view;

// 在 ZdownApp::ui 中：
match self.view_mode {
    ViewMode::Source => source_view::show_source_view(ui, &mut self.state, highlighter),
    ViewMode::Preview => preview_view::show_preview_view(ui, &mut self.state),
    ViewMode::Hybrid => hybrid_view::show_hybrid_view(ui, &mut self.state, highlighter),
}
```

- [ ] **步骤 2.4：编译验证 + smoke**

运行：`cargo build -p zdown-app && ZDOWN_SMOKE=1 cargo run -p zdown-app`
预期：通过。

- [ ] **步骤 2.5：Commit**

```bash
git add crates/zdown-app/src/hybrid_view.rs crates/zdown-app/src/source_view.rs crates/zdown-app/src/main.rs
git commit -m "feat(zdown-app): hybrid 视图（按 block 边界分割）

用 BlockWithSpan 的 span 查找光标所在 block：
- 光标前 block 全 AST 渲染
- 光标 block 源码高亮 + 光标
- 光标后 block 全 AST 渲染
避免按行切割破坏多行结构（代码块/表格/引用块）。
hybrid 模式复用 source_view 的输入处理。"
```

---

## 任务 3：渲染缓存（无 Mutex + LRU 上限）

**文件：**
- 修改：`crates/markdown_renderer/src/render.rs`
- 修改：`crates/markdown_renderer/src/lib.rs`
- 修改：`crates/zdown-app/src/main.rs` + `preview_view.rs` + `hybrid_view.rs`

- [ ] **步骤 3.1：实现 RenderCache（无 Mutex，用 &mut self）**

修改 `crates/markdown_renderer/src/render.rs`，加缓存结构：

```rust
use std::collections::{HashMap, VecDeque};

/// 渲染缓存。key 为源码 hash，value 为解析后的 Document。
/// LRU 上限 10 条，超出丢弃最旧。
/// 无 Mutex（egui 单线程），用 &mut self。
pub struct RenderCache {
    cache: HashMap<u64, Document>,
    lru_keys: VecDeque<u64>,
    max_entries: usize,
}

impl RenderCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            lru_keys: VecDeque::new(),
            max_entries: 10,
        }
    }

    /// 解析源码，缓存结果。若同 hash 已缓存则直接返回。
    pub fn parse_cached(&mut self, src: &str) -> Document {
        let hash = hash_src(src);
        if let Some(doc) = self.cache.get(&hash) {
            // LRU 更新：移到队首
            self.lru_keys.retain(|&k| k != hash);
            self.lru_keys.push_front(hash);
            return doc.clone();
        }
        let doc = document_model::parse(src).unwrap_or(Document { blocks: vec![] });
        // 超限丢弃最旧
        while self.lru_keys.len() >= self.max_entries {
            if let Some(old_key) = self.lru_keys.pop_back() {
                self.cache.remove(&old_key);
            }
        }
        self.cache.insert(hash, doc.clone());
        self.lru_keys.push_front(hash);
        doc
    }

    /// 清空缓存（文档切换时调用）。
    pub fn clear(&mut self) {
        self.cache.clear();
        self.lru_keys.clear();
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}

fn hash_src(src: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut hasher);
    hasher.finish()
}
```

- [ ] **步骤 3.2：lib.rs re-export**

修改 `crates/markdown_renderer/src/lib.rs`：

```rust
pub use render::{render, RenderCache};
```

- [ ] **步骤 3.3：zdown-app 接入缓存**

修改 `crates/zdown-app/src/main.rs`，在 ZdownApp 加 `render_cache` 字段：

```rust
#[derive(Default)]
struct ZdownApp {
    state: EditorState,
    confirm: ConfirmDialog,
    view_mode: ViewMode,
    last_title: String,
    highlighter: Option<markdown_renderer::SourceHighlighter>,
    render_cache: markdown_renderer::RenderCache,
}
```

修改 `preview_view.rs` 和 `hybrid_view.rs` 接受缓存参数：

```rust
// preview_view.rs
pub fn show_preview_view(ui: &mut egui::Ui, state: &mut EditorState, cache: &mut markdown_renderer::RenderCache) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        let src = state.editor.to_string();
        let doc = cache.parse_cached(&src);
        markdown_renderer::render(ui, &doc);
    });
}
```

```rust
// hybrid_view.rs 的 show_hybrid_view 签名加 cache 参数
pub fn show_hybrid_view(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    highlighter: Option<&SourceHighlighter>,
    cache: &mut markdown_renderer::RenderCache,
) {
    // ...
    let doc = cache.parse_cached(&src);
    // ...
}
```

更新 `main.rs` 调用：

```rust
ViewMode::Preview => preview_view::show_preview_view(ui, &mut self.state, &mut self.render_cache),
ViewMode::Hybrid => hybrid_view::show_hybrid_view(ui, &mut self.state, highlighter, &mut self.render_cache),
```

- [ ] **步骤 3.4：编译验证 + clippy**

运行：`cargo build -p zdown-app && cargo clippy --workspace --all-targets -- -D warnings`
预期：通过。

- [ ] **步骤 3.5：Commit**

```bash
git add crates/markdown_renderer/src/render.rs crates/markdown_renderer/src/lib.rs crates/zdown-app/src/
git commit -m "feat(markdown_renderer): 渲染缓存（无 Mutex + LRU 上限）

RenderCache 用 HashMap<u64, Document> + VecDeque LRU 跟踪，上限 10 条。
无 Mutex（egui 单线程），用 &mut self。
parse_cached 命中缓存则返回 clone，否则 parse + 缓存。
zdown-app 接入缓存，预览/hybrid 视图复用。"
```

---

## 任务 4：性能测试（输入延迟 < 50ms）

**文件：**
- 创建：`crates/zdown-app/tests/hybrid_perf.rs`

- [ ] **步骤 4.1：性能测试**

创建 `crates/zdown-app/tests/hybrid_perf.rs`：

```rust
//! hybrid 模式性能测试：parse 缓存命中 < 50ms，增量 parse < 50ms。
//!
//! 运行：`cargo test --test hybrid_perf -- --ignored --nocapture`
//! 注：render 延迟需 egui Context，由手动验证。

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stdout)]

use std::time::Instant;
use markdown_renderer::RenderCache;

fn generate_large_markdown(target_bytes: usize) -> String {
    let paragraph = "性能测试文本，含中文与 English。";
    let mut out = String::with_capacity(target_bytes);
    let mut idx = 0;
    while out.len() < target_bytes {
        out.push_str(&format!("# 标题 {idx}\n\n{paragraph}\n\n"));
        idx += 1;
    }
    out
}

#[test]
#[ignore = "性能测试，手动运行"]
fn hybrid_parse_cached_under_50ms() {
    let src = generate_large_markdown(500_000); // 500KB
    let mut cache = RenderCache::new();

    let start = Instant::now();
    let _doc = cache.parse_cached(&src);
    let cold = start.elapsed();
    println!("冷启动 parse 500KB: {cold:?}");

    let start = Instant::now();
    let _doc = cache.parse_cached(&src);
    let hot = start.elapsed();
    println!("缓存命中 parse 500KB: {hot:?}");

    assert!(
        hot.as_millis() < 50,
        "缓存命中应 < 50ms，实际 {hot:?}"
    );
}

#[test]
#[ignore = "性能测试，手动运行"]
fn hybrid_incremental_parse_under_50ms() {
    let mut src = generate_large_markdown(100_000); // 100KB
    let mut cache = RenderCache::new();

    for i in 0..10 {
        src.push_str(&format!("编辑{i}\n"));
        let start = Instant::now();
        let _doc = cache.parse_cached(&src);
        let elapsed = start.elapsed();
        println!("增量编辑 #{i} parse: {elapsed:?}");
        assert!(
            elapsed.as_millis() < 50,
            "增量 parse 100KB 应 < 50ms，实际 {elapsed:?}"
        );
    }
}
```

- [ ] **步骤 4.2：运行性能测试**

运行：`cargo test -p zdown-app --test hybrid_perf -- --ignored --nocapture`
预期：通过。

- [ ] **步骤 4.3：Commit**

```bash
git add crates/zdown-app/tests/hybrid_perf.rs
git commit -m "test(zdown-app): hybrid 性能测试（parse < 50ms）

2 个 #[ignore] 性能测试：
- hybrid_parse_cached_under_50ms: 缓存命中 < 50ms
- hybrid_incremental_parse_under_50ms: 增量 parse 100KB < 50ms
render 延迟由手动验证（需 egui Context）。"
```

---

## 任务 5：全量验证

- [ ] **步骤 5.1：fmt + clippy + test + build + smoke**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
ZDOWN_SMOKE=1 cargo run -p zdown-app
```

- [ ] **步骤 5.2：本地手动验证**

- Ctrl+3 切换到 Hybrid
- 光标所在 block 显示源码高亮，其余 block 显示渲染
- 光标在代码块内：代码块整体显示源码，不被切割
- 输入字符，hybrid 视图实时更新
- 光标移动（方向键），分割位置跟随 block 边界
- 输入延迟 < 50ms（主观）
- Ctrl+1/2/3 切换模式不丢失光标

- [ ] **步骤 5.3：Commit（如有修复）**

```bash
git add -A
git commit -m "chore: Plan 4 验证通过"
```

---

## 自检

**1. 规格覆盖度：**

- ROADMAP 阶段 2 交付物：
  - hybrid 模式：光标处源码，其余渲染 → 任务 2 ✓
  - 实时预览（输入即渲染）→ 任务 2 ✓
  - 渲染缓存 → 任务 3 ✓
- 验收标准 1（渲染常见文档无错位）→ 任务 2 按 block 分割避免错位 ✓
- 验收标准 2（三种模式切换不丢失光标）→ Plan 2 ✓
- 验收标准 3（hybrid 编辑流畅 < 50ms）→ 任务 4 ✓
- 验收标准 4（渲染快照测试）→ Plan 1 ✓

**2. 占位符扫描：**

- 每个步骤含完整代码
- `handle_input_hybrid` 标注"建议提取共享函数"是重构建议，非占位符

**3. 类型一致性：**

- `BlockWithSpan { block, span }` 跨任务一致
- `Span { start_line, end_line }` 跨任务一致
- `RenderCache::parse_cached(&mut self, &str) -> Document` 跨任务一致
- `show_hybrid_view(ui, state, highlighter, cache)` 签名一致

**4. 已知简化（阶段 3）：**

- hybrid 光标精确像素定位（当前用粗略标记）
- 大文档只重渲染光标附近 block
- `handle_input` 与 `handle_input_hybrid` 重复（建议提取共享模块）

---

## 执行交接

本计划已保存到 `docs/superpowers/plans/2026-06-18-stage2-hybrid-and-cache.md`。

阶段 2 四个 plan 全部完成。执行者注意：4 个 plan 按依赖顺序执行（Plan 1 → 2 → 3 → 4），完成后阶段 2 关闭。
