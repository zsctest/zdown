# zdown 路线图

本文档按阶段划分里程碑，每阶段产出可独立运行、可验证的增量。阶段编号固定，不随实现进度调整。

---

## 阶段 0 — 工程骨架

**目标**：建立 workspace 与各 crate 骨架，跑通 egui 空窗口在 Windows / Linux / macOS 三平台编译。

**交付物**：

- 根 `Cargo.toml`（workspace 声明）
- `crates/` 下 7 个成员 crate（document_model / editor_engine / markdown_renderer / export_engine / workspace / config / zdown-app）
- 每个 crate 的 `lib.rs` 或 `main.rs` 占位
- `zdown-app` 显示一个 egui 窗口，标题 "zdown"，内容为占位文字
- `.gitignore` 已含 `target/`
- CI 配置：Windows 全量（fmt + clippy + test + build + run smoke），Linux / macOS 仅编译验证（无 GUI runner）

**验收标准**：

1. Windows：`cargo build` 通过，`cargo run -p zdown-app` 弹出窗口，`cargo test` 通过，`cargo clippy -- -D warnings` 无警告
2. Linux / macOS：`cargo build` 通过（CI 跨平台编译验证，不跑 GUI）
3. 各 crate 占位测试通过

---

## 阶段 1 — 最小可用编辑器（MVP） ✅ 主体完成

**目标**：能打开、编辑、保存 Markdown 文件，源码视图 + 行号。

**交付物**：

- `document_model`：基于 `pulldown-cmark` 的解析器、AST 类型、`to_markdown` 序列化 ✅
- `editor_engine`：基于 `ropey::Rope` 的文本缓冲、光标、撤销重做 ✅
- `workspace`：打开 / 保存文件对话框（`rfd`）、最近文件 TOML 持久化 ✅
- `markdown_renderer`：`SourceHighlighter`（syntect 默认嵌入集） ✅
- `zdown-app`：
  - 单 tab 编辑视图 ✅
  - 源码编辑模式（等宽字体、行号） ✅
  - 快捷键：Ctrl+N/O/S/Shift+S/Z/Y ✅
  - 未保存提示（三选项对话框）、最近文件菜单 ✅

**降级说明（推到阶段 2）**：

- **行内语法高亮**：egui 0.34 `TextEdit::multiline` 不暴露内部文本布局，无法精确叠加高亮。阶段 1 用单色编辑，`SourceHighlighter` 已实现留待阶段 2 hybrid 模式接入
- **增量编辑命令**：阶段 1 整体替换文本（丢失 undo 历史），阶段 2 改为基于光标事件的增量 `Command`
- **文件监听（notify）**：原 T1-14 删除，推迟到阶段 3 与多文件管理一起做

**验收标准**：

1. 能打开 ≥ 1MB 的 Markdown 文件不卡顿（< 200ms 渲染）⏳ T1-24
2. 编辑后保存，重新打开内容一致 ✅（Plan 3 集成测试覆盖）
3. 撤销 / 重做链路正确 ✅（Plan 2 测试覆盖）
4. 三平台编译通过 ✅（CI 绿灯）
5. 各 crate 单元测试覆盖率 ≥ 80% ⏳ T1-23

---

## 阶段 2 — 渲染与所见即所得

**目标**：支持源码 / 预览 / hybrid 三种视图切换。

**交付物**：

- `markdown_renderer`：AST → egui widget 渲染
  - 标题、段落、列表、引用、代码块、链接、图片、表格、水平线
  - 代码块语法高亮
  - 渲染缓存
- `zdown-app`：
  - 视图模式切换（Ctrl+1 源码 / Ctrl+2 预览 / Ctrl+3 hybrid）
  - hybrid 模式：光标处显示源码，其余渲染
  - 实时预览（输入即渲染）
  - **补阶段 1 高亮降级**：源码模式行内语法高亮（接入 `SourceHighlighter`）
  - **补阶段 1 增量编辑**：基于光标事件的增量 `Command`，恢复 undo 历史

**验收标准**：

1. 渲染常见 Markdown 文档无错位
2. 三种模式切换不丢失光标位置
3. hybrid 模式编辑流畅（输入延迟 < 50ms）
4. 渲染快照测试通过

---

## 阶段 3 — 个性化与多文件

**目标**：配置系统、多 tab、大纲、主题。

**交付物**：

- `config`：
  - TOML 配置文件（`~/.config/zdown/config.toml` 或平台等价路径）
  - 主题（亮 / 暗 / 自定义 CSS）
  - 快捷键映射
  - 字体设置
  - 拼写检查开关
- `zdown-app`：
  - 多 tab 文件管理
  - 大纲面板（从 AST 提取标题树）
  - 设置面板（GUI 配置）
  - 主题切换实时生效
  - 自定义字体加载

**验收标准**：

1. 配置改动持久化，重启后生效
2. 多 tab 切换不丢失各自撤销栈
3. 大纲点击可跳转对应标题
4. 主题切换无需重启

---

## 阶段 4 — 高级功能

按优先级分批交付，每项独立里程碑。

### 4.1 图床

- 本地图片嵌入
- 图片转 base64 内联
- 云端图床（接口抽象，先实现一种，如 GitHub / SM.MSM）

### 4.2 导出

- `export_engine`：HTML 导出（带内联 CSS）
- PDF 导出（方案待选型，见 ARCHITECTURE.md 待决策项）

### 4.3 mermaid 支持

- mermaid 代码块识别
- 渲染方案待定（内嵌 JS 引擎 vs mermaid-cli）

### 4.4 HTML 支持

- 内嵌 HTML 渲染
- HTML 实体处理

### 4.5 多语言

- `fluent` 集成
- 中 / 英双语初始支持
- 语言切换无需重启

### 4.6 终端面板

- 嵌入式终端
- 工作目录跟随当前文件

### 4.7 AI 续写

- 接口抽象（`CompletionProvider` trait）
- 本地 / 远程实现留待选型
- 快捷键触发（Tab 接受建议）

### 4.8 插件系统

- 插件运行时选型（WASM / Lua / 进程隔离，见待决策项）
- 插件 API：访问 AST、注册命令、注册渲染器
- 插件市场（远期）

**验收标准（每子项）**：

1. 功能可用且不破坏既有测试
2. 新增功能自带单元测试 + 集成测试
3. 三平台编译通过
4. 文档更新

---

## 阶段间约束

- 不跳阶段：阶段 N 的验收标准未达成前，不开始阶段 N+1 的功能开发
- 允许前置：可在阶段 N 内提前为阶段 N+1 做接口预留，但实现留空
- 每阶段结束前必须通过：`cargo fmt` + `cargo clippy` + `cargo test`（Windows 全量）+ Linux/macOS 编译通过（跨平台 CI 验证）
