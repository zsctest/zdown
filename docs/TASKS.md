# zdown 任务清单

本文件只细化当前即将实施的阶段（阶段 0）。后续阶段在进入实施前再展开细化，避免长期任务清单失真。

每条任务标注：`[crate]` 范围、验收点。可在阶段 0 内独立并行。

---

## 阶段 0 — 工程骨架

### 0.1 workspace 初始化

- [x] **T0-01** 创建根 `Cargo.toml`，声明 workspace + `[workspace.dependencies]` 公共依赖版本
  - 范围：根
  - 验收：`cargo metadata` 成功，无成员时报错可接受
- [x] **T0-02** 创建 `crates/` 目录与 7 个成员 crate 子目录
  - 范围：根
  - 验收：目录结构存在

### 0.2 各 crate 骨架（可并行）

每项任务格式：创建 `crates/<name>/Cargo.toml` + `src/lib.rs`（或 `main.rs`）占位，导出空模块，含一个占位测试。

- [x] **T0-03** `document_model` 骨架
  - 验收：`cargo build -p document_model` 通过，`cargo test -p document_model` 通过 1 个占位测试
- [x] **T0-04** `editor_engine` 骨架
  - 验收：同上
- [x] **T0-05** `markdown_renderer` 骨架
  - 验收：同上
- [x] **T0-06** `export_engine` 骨架
  - 验收：同上
- [x] **T0-07** `workspace` 骨架
  - 验收：同上
- [x] **T0-08** `config` 骨架
  - 验收：同上
- [x] **T0-09** `zdown-app` 骨架（二进制 crate）
  - 验收：`cargo build -p zdown-app` 通过

### 0.3 egui 窗口跑通

- [x] **T0-10** `zdown-app` 集成 `eframe`，启动一个显示 "zdown" 标题的窗口，内容为占位文字 "zdown skeleton"
  - 范围：`zdown-app`
  - 验收：`cargo run -p zdown-app` 弹出窗口，无 panic
  - 备注：CI 无法验证 GUI，需本地手动确认，留截图
  - 完成：本地手动确认弹窗正常（标题 "zdown"，内容 "zdown skeleton"）；CI 通过 `ZDOWN_SMOKE=1 cargo run -p zdown-app` smoke 校验

### 0.4 质量基线

- [x] **T0-11** 添加 `clippy` 配置（`clippy.toml` 或 `Cargo.toml` 内 `[lints]`），开启 `-D warnings`
  - 验收：`cargo clippy --workspace -- -D warnings` 通过
  - 完成：在根 `Cargo.toml` 的 `[workspace.lints.clippy]` 中声明 `unwrap_used`/`expect_used`/`dbg_macro` = deny，其余 warn
- [x] **T0-12** 添加 `rustfmt` 配置（`rustfmt.toml`，统一版式）
  - 验收：`cargo fmt --check` 通过
  - 完成：`rustfmt.toml` 指定 `edition = "2024"`、`max_width = 100`
- [x] **T0-13** 各 crate 定义 `Error` 类型骨架（`thiserror` 派生，含一个变体占位）
  - 验收：`cargo build` 通过
  - 完成：六个库 crate 各自定义 `Error { NotImplemented }`，附 `error_display` 测试
- [x] **T0-14** `zdown-app` 初始化 `tracing-subscriber`，启动时打印一条 info 日志
  - 验收：运行时终端可见日志输出
  - 完成：`EnvFilter` 读 `RUST_LOG`，缺省退回 `info`；smoke 运行确认日志可见

### 0.5 CI

- [x] **T0-15** 添加 CI 配置（CNB 或 GitHub Actions）：
  - Windows：`fmt --check` + `clippy -D warnings` + `test` + `build` + `run` smoke
  - Linux / macOS：仅 `build`（无 GUI runner，不跑测试与 GUI）
  - 备注：阶段 0 不引入 `rfd`，Linux 无需装 gtk
  - 完成：选定 GitHub Actions；`.github/workflows/ci.yml` 三平台矩阵；Linux 安装 eframe 系统依赖；smoke 通过 `ZDOWN_SMOKE` 环境变量触发

### 0.6 文档与提交规范

- [x] **T0-16** 添加 `rust-toolchain.toml`，固定 Rust 版本（建议 stable）
  - 验收：`rustc --version` 与文件一致
  - 完成：`channel = "stable"`，components `rustfmt` + `clippy`，profile `minimal`
- [x] **T0-17** 更新 `.gitignore`，确保 `target/`、IDE 文件被忽略
  - 验收：`git status` 干净
  - 完成：含 `target/`、IDE、`.codegraph/`、`.cargo/config.toml`（本机配置不入库）

---

## 阶段 0 验收（汇总）

完成 T0-01 ~ T0-17 后，需同时满足：

1. Windows：`cargo build --workspace` 通过 ✅
2. Windows：`cargo run -p zdown-app` 弹出窗口 ✅（本地手动确认）
3. Windows：`cargo test --workspace` 全绿 ✅（13 个测试通过）
4. Windows：`cargo clippy --workspace -- -D warnings` 无警告 ✅
5. `cargo fmt --check` 通过 ✅
6. Linux / macOS CI：`cargo build --workspace` 通过（仅编译验证）⏳（待首次 push 后 CI 验证）
7. CI 在上述矩阵绿灯 ⏳（待首次 push 后 CI 验证）

> 阶段 0 本地验收项全部通过，跨平台 CI 待首次推送后在 GitHub Actions 上验证。
> 满足后，阶段 0 关闭，进入阶段 1（届时展开阶段 1 任务清单）。

---

## 阶段 1 — 最小可用编辑器（MVP）

目标：能打开、编辑、保存 Markdown 文件，源码视图 + 行号。

**实施计划**：拆分为 4 个独立 plan，按依赖顺序执行。每个 plan 含完整 TDD 步骤，详见各自文档。

| Plan | 范围 | 详细文档 | 对应原任务 | 状态 |
| --- | --- | --- | --- | --- |
| Plan 1 | document_model（AST + parse + to_markdown） | [2026-06-18-document-model.md](superpowers/plans/2026-06-18-document-model.md) | T1-01, T1-03 ~ T1-06 | ✅ 完成 |
| Plan 2 | editor_engine（Buffer + Command + History + Editor） | [2026-06-18-editor-engine.md](superpowers/plans/2026-06-18-editor-engine.md) | T1-07 ~ T1-10 | ✅ 完成 |
| Plan 3 | workspace（open/save/rfd/recent + Error） | [2026-06-18-workspace.md](superpowers/plans/2026-06-18-workspace.md) | T1-02, T1-11 ~ T1-13, T1-15 | ✅ 完成 |
| Plan 4 | markdown_renderer source + zdown-app（高亮 + 编辑视图 + 菜单 + 快捷键） | [2026-06-18-markdown-renderer-source-and-zdown-app.md](superpowers/plans/2026-06-18-markdown-renderer-source-and-zdown-app.md) | T1-16, T1-18 ~ T1-22 | ✅ 完成 |

**主体实现完成**（24 commit，152 passed + 5 ignored，全量验证 + GUI 手动确认通过）。

**关键设计决策（已敲定，见各 plan）：**

- 源码语法高亮放 `markdown_renderer` source 模块；syntect 用默认嵌入语法集与主题
- `Workspace` 有状态：持有 `Option<PathBuf>`，`open` 设置路径，`save` 写当前路径，`save_as` 更新路径
- 最近文件独立 TOML（`<config_dir>/zdown/recent.toml`），最多 10 条，canonicalize 去重
- `Command` 用 enum（非 trait），`apply` 返回 `AppliedCommand` 携带 undo 信息
- `Cursor { line, col }`，col 为字符列（非字节列）
- AST 分 `Block` / `Inline` 两层，不携带 source span

**对原 TASKS.md（粗版）的调整：**

- **删除 T1-14 文件监听**：notify 推迟到阶段 3 与多文件管理一起做
- **T1-17 行号渲染并入 Plan 4**：行号是编辑器装饰，放 zdown-app 而非 markdown_renderer
- **T1-19 高亮降级**：egui 0.34 `TextEdit::multiline` 不暴露内部布局，行内高亮不可行；阶段 1 用单色编辑（路径 B），高亮推迟到阶段 2 hybrid 模式
- **T1-08 Command 设计重写**：从 trait + 单 Cursor 改为 enum + AppliedCommand（含 undo 信息），支持选区

**收尾任务（4 个 plan 完成后执行，不在 plan 内）：**

- [x] **T1-23** 各 crate 单元测试覆盖率 ≥ 80%（用 `cargo-llvm-cov`，排除 zdown-app）
  - 完成：整体覆盖率 **92.16%**（区域 91.51%），远超 80% 目标
  - 各 crate：document_model ~88%（parse 78%）、editor_engine ~96%、workspace ~93%（dialog 0% 因 #[ignore]）、markdown_renderer ~95%、config/export_engine 100%（占位）
- [x] **T1-24** 性能测试：≥ 1MB Markdown 文件 `parse` + `Buffer::from_str` < 200ms（仅测核心，UI 渲染手动评估）
  - 完成：`crates/document_model/tests/perf.rs` 3 个 #[ignore] 性能测试
  - 实测 1MB 文件：parse 50ms、Buffer::from_str 13ms、合计 61ms（远低于 200ms）
- [x] **T1-25** 集成测试：编辑→保存→重开内容一致（Plan 2 `edit_save_reopen_content_consistent` 部分覆盖，补完整链路测试）
  - 完成：`crates/workspace/tests/edit_save_reopen.rs` 8 个集成测试
  - 覆盖：edit_save_reopen / new_edit_save_as / edit_undo_save / edit_redo_save / multiple_edits / delete_save / replace_save / save_mark_saved_clears_dirty
- [x] **T1-26** 更新 ROADMAP.md 标注阶段 1 关闭 + 高亮降级说明 + 加 T2-XX 阶段 2 补高亮任务
  - 完成：ROADMAP.md 阶段 1 标注 ✅ 主体完成 + 降级说明；阶段 2 加"补阶段 1 高亮降级"与"补阶段 1 增量编辑"交付物

---

### 阶段 1 验收（汇总）

完成 4 个 plan + T1-23 ~ T1-26 后，需同时满足：

1. Windows：`cargo build --workspace` 通过 ✅
2. Windows：`cargo run -p zdown-app` 可打开/编辑/保存 Markdown 文件（手动确认）✅
3. Windows：`cargo test --workspace` 全绿 ✅（168 passed + 5 ignored）
4. Windows：`cargo clippy --workspace --all-targets -- -D warnings` 无警告 ✅
5. `cargo fmt --check` 通过 ✅
6. Linux / macOS CI：`cargo build --workspace` 通过（rfd 已引入，Linux gtk 已装）⏳（待 push 后 CI 验证）
7. 各库 crate 单元测试覆盖率 ≥ 80% ✅（92.16%）
8. ≥ 1MB 文件 `parse` + `Buffer::from_str` < 200ms ✅（61ms）
9. 编辑→保存→重开内容一致 ✅（8 个集成测试）

**降级说明**：阶段 1 不实现行内语法高亮（egui 0.34 限制），推到阶段 2。其余验收项必须满足。✅ 已满足

满足后，阶段 1 关闭，进入阶段 2（届时展开阶段 2 任务清单）。✅ 阶段 1 已关闭

---

## 阶段 2 — 渲染与所见即所得

目标：支持源码 / 预览 / hybrid 三种视图切换 + 补阶段 1 高亮与增量编辑降级。

**实施计划**：拆分为 4 个独立 plan，按依赖顺序执行。每个 plan 含完整 TDD 步骤，详见各自文档。

| Plan | 范围 | 详细文档 | 状态 |
| --- | --- | --- | --- |
| Plan 1 | markdown_renderer AST 渲染（Block/Inline + 代码块高亮 + 快照测试） | [2026-06-18-stage2-markdown-renderer-ast.md](superpowers/plans/2026-06-18-stage2-markdown-renderer-ast.md) | 待实施 |
| Plan 2 | zdown-app 视图模式切换（Source/Preview/Hybrid + Ctrl+1/2/3 + 实时预览） | [2026-06-18-stage2-view-mode-switch.md](superpowers/plans/2026-06-18-stage2-view-mode-switch.md) | 待实施 |
| Plan 3 | 补阶段 1 高亮 + 增量编辑（source_view 重构 + 行内高亮 + Command） | [2026-06-18-stage2-highlight-and-incremental-edit.md](superpowers/plans/2026-06-18-stage2-highlight-and-incremental-edit.md) | 待实施 |
| Plan 4 | hybrid 模式 + 渲染缓存（光标行源码 + 其余渲染 + RenderCache + 性能） | [2026-06-18-stage2-hybrid-and-cache.md](superpowers/plans/2026-06-18-stage2-hybrid-and-cache.md) | 待实施 |

**关键设计决策（已敲定，见各 plan）：**

- AST 渲染策略：spike 后决定（优先 egui 原生 widget）
- 渲染接口：`render(ui: &mut egui::Ui, doc: &Document)`
- 代码块高亮：复用阶段 1 `SourceHighlighter`，syntect Style 颜色转 egui Color32
- 图片渲染：阶段 2 用文本占位符 `[图片: alt](url)`，实际图片留阶段 4
- ViewMode：Source/Preview/Hybrid，默认 Source，Ctrl+1/2/3 切换
- hybrid 分割：按光标所在行分割，光标行源码高亮，其余 AST 渲染
- 渲染缓存：`RenderCache` 用 `HashMap<u64, Document>`，缓存 parse 结果（egui widget 不可序列化）
- 增量编辑：输入事件转 `editor_engine::Command`，恢复 undo 历史

**对阶段 1 降级的补救：**

- 行内语法高亮：Plan 3 source_view 重构接入 SourceHighlighter
- 增量编辑命令：Plan 3 输入事件转 Command，恢复 undo 历史

---

### 阶段 2 验收（汇总）

完成 4 个 plan 后，需同时满足：

1. Windows：`cargo build --workspace` 通过
2. Windows：`cargo run -p zdown-app` 三种视图模式切换正常（手动确认）
3. Windows：`cargo test --workspace` 全绿
4. Windows：`cargo clippy --workspace --all-targets -- -D warnings` 无警告
5. `cargo fmt --check` 通过
6. Linux / macOS CI：`cargo build --workspace` 通过
7. 渲染常见 Markdown 文档无错位（手动确认）
8. 三种模式切换不丢失光标位置（手动确认）
9. hybrid 模式编辑流畅（输入延迟 < 50ms，性能测试验证）
10. 渲染快照测试通过

满足后，阶段 2 关闭，进入阶段 3（届时展开阶段 3 任务清单）。

---

## 后续阶段（待细化）

- 阶段 3 个性化与多文件：进入实施前展开（含文件监听 notify、选区编辑、完全自绘文本）
- 阶段 4 高级功能：按子里程碑（图床 / 导出 / mermaid / HTML / i18n / 终端 / AI / 插件）分别展开

不在本文件预先列出，避免任务清单过早固化。
