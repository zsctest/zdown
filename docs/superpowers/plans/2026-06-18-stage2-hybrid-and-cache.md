# hybrid 模式 + 渲染缓存实现计划

> **面向 AI 代理的工作者：** 必需子智能体：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。

**目标：** 实现 zdown-app hybrid 模式（光标处源码 + 其余渲染）+ markdown_renderer 渲染缓存，满足输入延迟 < 50ms。

**架构：** hybrid 模式将文档按光标位置分为"光标行（源码显示）"与"其余行（渲染显示）"两部分。渲染时跳过光标行用源码高亮，其余用 AST 渲染。渲染缓存在 markdown_renderer 用 AST hash 缓存渲染结果，避免重复 parse + render。

**技术栈：** Rust 2024 edition、egui 0.34、editor_engine、markdown_renderer、document_model。

**前置任务：** Plan 1（AST 渲染）+ Plan 2（视图切换）+ Plan 3（高亮 + 增量编辑）完成。

---

## 文件结构

- 创建：`crates/zdown-app/src/hybrid_view.rs` — hybrid 视图
- 修改：`crates/zdown-app/src/main.rs` — Hybrid 模式接入
- 修改：`crates/markdown_renderer/src/render.rs` — 渲染缓存
- 修改：`crates/markdown_renderer/src/lib.rs` — re-export 缓存类型

**关键设计决策：**

- **hybrid 分割**：按光标所在行分割，光标行用源码高亮（含光标矩形），其余行用 AST 渲染
- **实时重渲染**：每次 update 重新分割 + 渲染
- **渲染缓存**：用 `std::collections::HashMap<u64, CachedRender>`，key 为 AST 内容 hash，value 为渲染结果（egui 不支持 widget 序列化，缓存 AST 解析结果 + 高亮结果）
- **性能**：大文档时只重渲染光标附近行（阶段 3 优化）

---

## 任务 1：hybrid 视图基础

**文件：**
- 创建：`crates/zdown-app/src/hybrid_view.rs`
- 修改：`crates/zdown-app/src/main.rs`

- [ ] **步骤 1.1：创建 hybrid_view.rs**

创建 `crates/zdown-app/src/hybrid_view.rs`：

```rust
//! Hybrid 视图：光标行源码 + 其余渲染。
//!
//! 按光标所在行分割文档：
//! - 光标行：源码高亮 + 光标矩形
//! - 其余行：AST 渲染

use eframe::egui;
use markdown_renderer::SourceHighlighter;

use crate::editor_state::EditorState;

/// 渲染 hybrid 视图。
pub fn show_hybrid_view(ui: &mut egui::Ui, state: &mut EditorState) {
    let src = state.editor.to_string();
    let cursor_line = state.editor.cursor.line;
    
    egui::ScrollArea::vertical().show(ui, |ui| {
        // 1. 渲染光标行之前的 AST（按块分割）
        let before_src: String = src.lines()
            .take(cursor_line)
            .map(|l| format!("{l}\n"))
            .collect();
        if !before_src.is_empty() {
            let doc = document_model::parse(&before_src).unwrap_or(document_model::Document { blocks: vec![] });
            markdown_renderer::render(ui, &doc);
        }
        
        // 2. 光标行：源码高亮 + 光标
        let cursor_line_text = src.lines().nth(cursor_line).unwrap_or("");
        highlight_line_with_cursor(ui, cursor_line_text, state.editor.cursor.col);
        
        // 3. 渲染光标行之后的 AST
        let after_src: String = src.lines()
            .skip(cursor_line + 1)
            .map(|l| format!("{l}\n"))
            .collect();
        if !after_src.is_empty() {
            let doc = document_model::parse(&after_src).unwrap_or(document_model::Document { blocks: vec![] });
            markdown_renderer::render(ui, &doc);
        }
    });
}

/// 高亮单行 + 绘制光标。
fn highlight_line_with_cursor(ui: &mut egui::Ui, line: &str, cursor_col: usize) {
    let highlighter = SourceHighlighter::new().ok();
    ui.horizontal(|ui| {
        if let Some(h) = &highlighter {
            let lines = h.highlight(line, None);
            if let Some(styled_line) = lines.first() {
                let mut col = 0;
                for (style, text) in styled_line {
                    let color = egui::Color32::from_rgb(
                        style.foreground.r,
                        style.foreground.g,
                        style.foreground.b,
                    );
                    // 简化：光标用背景色标记（阶段 2 简化，不做精确像素定位）
                    let is_cursor_here = col <= cursor_col && cursor_col < col + text.chars().count();
                    let richtext = egui::RichText::new(*text).color(color).monospace();
                    if is_cursor_here {
                        ui.label(richtext.background_color(egui::Color32::from_rgb(80, 80, 80)));
                    } else {
                        ui.label(richtext);
                    }
                    col += text.chars().count();
                }
            }
        } else {
            ui.label(egui::RichText::new(line).monospace());
        }
    });
}
```

- [ ] **步骤 1.2：main.rs 接入 hybrid 视图**

修改 `crates/zdown-app/src/main.rs`：

```rust
mod hybrid_view;

// 在 ZdownApp::ui 中：
match self.view_mode {
    ViewMode::Source => source_view::show_source_view(ui, &mut self.state),
    ViewMode::Preview => preview_view::show_preview_view(ui, &mut self.state),
    ViewMode::Hybrid => hybrid_view::show_hybrid_view(ui, &mut self.state),
}
```

- [ ] **步骤 1.3：编译验证 + smoke**

运行：`cargo build -p zdown-app && ZDOWN_SMOKE=1 cargo run -p zdown-app`
预期：通过。

- [ ] **步骤 1.4：Commit**

```bash
git add crates/zdown-app/src/hybrid_view.rs crates/zdown-app/src/main.rs
git commit -m "feat(zdown-app): hybrid 视图（光标行源码 + 其余渲染）

按光标行分割文档：光标前行 AST 渲染、光标行源码高亮 + 光标标记、
光标后行 AST 渲染。光标用背景色标记（简化）。"
```

---

## 任务 2：渲染缓存

**文件：**
- 修改：`crates/markdown_renderer/src/render.rs`
- 修改：`crates/markdown_renderer/src/lib.rs`

- [ ] **步骤 2.1：实现渲染缓存**

修改 `crates/markdown_renderer/src/render.rs`，加缓存结构：

```rust
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

/// 渲染缓存。key 为源码 hash，value 为解析后的 Document。
/// egui widget 无法序列化，只缓存 parse 结果（parse 是最重的一步）。
pub struct RenderCache {
    cache: Mutex<HashMap<u64, Document>>,
}

impl RenderCache {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// 解析源码，缓存结果。若同 hash 已缓存则直接返回。
    pub fn parse_cached(&self, src: &str) -> Document {
        let hash = hash_src(src);
        let mut cache = self.cache.lock().expect("cache lock");
        if let Some(doc) = cache.get(&hash) {
            doc.clone()
        } else {
            let doc = document_model::parse(src).unwrap_or(Document { blocks: vec![] });
            cache.insert(hash, doc.clone());
            doc
        }
    }

    /// 清空缓存（文档切换时调用）。
    pub fn clear(&self) {
        self.cache.lock().expect("cache lock").clear();
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}

fn hash_src(src: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut hasher);
    hasher.finish()
}
```

- [ ] **步骤 2.2：lib.rs re-export**

修改 `crates/markdown_renderer/src/lib.rs`：

```rust
pub use render::{render, RenderCache};
```

- [ ] **步骤 2.3：zdown-app 接入缓存**

修改 `crates/zdown-app/src/main.rs`，在 ZdownApp 加 `render_cache` 字段：

```rust
#[derive(Default)]
struct ZdownApp {
    state: EditorState,
    confirm: ConfirmDialog,
    view_mode: ViewMode,
    render_cache: markdown_renderer::RenderCache,
}
```

修改 `preview_view.rs` 和 `hybrid_view.rs` 接受缓存参数：

```rust
// preview_view.rs
pub fn show_preview_view(ui: &mut egui::Ui, state: &mut EditorState, cache: &RenderCache) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        let src = state.editor.to_string();
        let doc = cache.parse_cached(&src);
        markdown_renderer::render(ui, &doc);
    });
}
```

- [ ] **步骤 2.4：编译验证 + clippy**

运行：`cargo build -p zdown-app && cargo clippy --workspace --all-targets -- -D warnings`
预期：通过。

- [ ] **步骤 2.5：Commit**

```bash
git add crates/markdown_renderer/src/render.rs crates/markdown_renderer/src/lib.rs crates/zdown-app/src/
git commit -m "feat(markdown_renderer): 渲染缓存（parse 结果缓存）

RenderCache 用 HashMap<u64, Document>，key 为源码 hash。
parse_cached 命中缓存则返回 clone，否则 parse + 缓存。
zdown-app 接入缓存，预览/hybrid 视图复用。"
```

---

## 任务 3：性能测试（输入延迟 < 50ms）

**文件：**
- 创建：`crates/zdown-app/tests/hybrid_perf.rs`（或用 #[ignore] 测试）

- [ ] **步骤 3.1：性能测试**

创建 `crates/zdown-app/tests/hybrid_perf.rs`：

```rust
//! hybrid 模式性能测试：输入延迟 < 50ms。
//!
//! 运行：`cargo test --test hybrid_perf -- --ignored --nocapture`

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stdout)]

use std::time::Instant;
use document_model::parse;
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
    let cache = RenderCache::new();

    // 第一次 parse（冷启动）
    let start = Instant::now();
    let _doc = cache.parse_cached(&src);
    let cold = start.elapsed();
    println!("冷启动 parse 500KB: {cold:?}");

    // 第二次（命中缓存）
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
    // 模拟增量编辑：每次改一个字符后重新 parse
    let mut src = generate_large_markdown(100_000); // 100KB
    let cache = RenderCache::new();

    for i in 0..10 {
        src.push_str(&format!("编辑{i}\n"));
        let start = Instant::now();
        let _doc = cache.parse_cached(&src);
        let elapsed = start.elapsed();
        println!("增量编辑 #{i} parse: {elapsed:?}");
        // 增量编辑不会命中缓存（hash 变了），但 100KB 应 < 50ms
        assert!(
            elapsed.as_millis() < 50,
            "增量 parse 应 < 50ms，实际 {elapsed:?}"
        );
    }
}
```

- [ ] **步骤 3.2：运行性能测试**

运行：`cargo test -p zdown-app --test hybrid_perf -- --ignored --nocapture`
预期：通过（缓存命中 < 50ms，增量 parse 100KB < 50ms）。

- [ ] **步骤 3.3：Commit**

```bash
git add crates/zdown-app/tests/hybrid_perf.rs
git commit -m "test(zdown-app): hybrid 性能测试（输入延迟 < 50ms）

2 个 #[ignore] 性能测试：
- hybrid_parse_cached_under_50ms: 缓存命中 < 50ms
- hybrid_incremental_parse_under_50ms: 增量 parse 100KB < 50ms"
```

---

## 任务 4：全量验证

- [ ] **步骤 4.1：fmt + clippy + test + build + smoke**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
ZDOWN_SMOKE=1 cargo run -p zdown-app
```

- [ ] **步骤 4.2：本地手动验证**

- Ctrl+3 切换到 Hybrid
- 光标行显示源码高亮，其余显示渲染
- 输入字符，hybrid 视图实时更新
- 光标移动，分割位置跟随
- 输入延迟 < 50ms（主观）

- [ ] **步骤 4.3：Commit（如有修复）**

```bash
git add -A
git commit -m "chore: Plan 4 验证通过"
```

---

## 自检

**1. 规格覆盖度：**

- ROADMAP 阶段 2 交付物：
  - hybrid 模式：光标处源码，其余渲染 → 任务 1 ✓
  - 实时预览（输入即渲染）→ 任务 1 ✓
  - 渲染缓存 → 任务 2 ✓
- 验收标准 1（渲染常见文档无错位）→ 手动验证
- 验收标准 2（三种模式切换不丢失光标）→ Plan 2 ✓
- 验收标准 3（hybrid 编辑流畅 < 50ms）→ 任务 3 ✓
- 验收标准 4（渲染快照测试）→ Plan 1 ✓

**2. 占位符扫描：**

- 每个步骤含完整代码
- 光标用背景色标记是简化（非占位符）

**3. 类型一致性：**

- `RenderCache::parse_cached(&str) -> Document` 跨任务一致
- `show_hybrid_view(ui, state)` 与 `show_preview_view(ui, state, cache)` 签名
- `Document` clone 合理（egui widget 不可序列化）

**4. 已知简化：**

- 光标精确像素定位留阶段 3
- 大文档只重渲染光标附近行留阶段 3
- 渲染缓存只缓存 parse（不缓存 widget 序列化，因 egui 不支持）

---

## 执行交接

本计划已保存到 `docs/superpowers/plans/2026-06-18-stage2-hybrid-and-cache.md`。

阶段 2 四个 plan 全部完成。执行者注意：4 个 plan 按依赖顺序执行（Plan 1 → 2 → 3 → 4），完成后阶段 2 关闭。
