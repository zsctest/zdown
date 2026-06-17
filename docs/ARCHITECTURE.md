# zdown 架构设计

## 1. 总体架构

zdown 采用分层架构，各层之间通过 trait 抽象解耦，依赖方向单向向下。

```
┌─────────────────────────────────────────────────┐
│  UI Layer        zdown-app (egui)                │
├─────────────────────────────────────────────────┤
│  Core Layer      editor_engine → document_model  │
├─────────────────────────────────────────────────┤
│  Render Layer    markdown_renderer / export_engine → document_model │
├─────────────────────────────────────────────────┤
│  Storage Layer   workspace → document_model      │
│                  config                          │
└─────────────────────────────────────────────────┘
```

依赖方向（单向向下，下层不反向依赖上层）：

- UI → Core / Render / Storage
- Core：`editor_engine` → `document_model`
- Render：`markdown_renderer` / `export_engine` → `document_model`
- Storage：`workspace` → `document_model`（workspace 负责文件 IO，调用 document_model 的 `parse` / `to_markdown`）
- `config` 独立，不依赖其他业务 crate
- `document_model` 是叶子节点，不依赖任何业务 crate（纯数据模型 + 解析/序列化）

## 2. Crate 划分

工作区根 `Cargo.toml` 使用 workspace 模式，成员 crate 全部位于 `crates/` 目录下：

```
zdown/
├── Cargo.toml              # workspace 根
├── crates/
│   ├── document_model/     # 文档数据模型与 AST
│   ├── editor_engine/      # 编辑操作、命令栈、光标管理
│   ├── markdown_renderer/  # AST → egui 组件渲染
│   ├── export_engine/      # AST → PDF / HTML 导出
│   ├── workspace/          # 文件 IO、最近文件、项目树
│   ├── config/             # 用户配置、主题、快捷键映射
│   └── zdown-app/          # egui 应用入口（二进制）
└── docs/
```

### 2.1 `document_model`

**职责**：定义 Markdown 文档的数据模型。

- AST 节点类型（标题、段落、代码块、列表、表格、引用等）
- 文档结构（前端 matter、正文、后端 matter）
- 序列化 / 反序列化（与 Markdown 源码互转）
- 不依赖 UI、不依赖文件系统（纯内存数据模型）

文件 IO 由 `workspace` 负责：`workspace` 读文件得到字符串后调用 `document_model::parse`，保存时调用 `Document::to_markdown` 再写入。`document_model` 本身不接触 `Path`。

**对外接口**：

- `Document`：根类型
- `parse(src: &str) -> Result<Document>`
- `Document::to_markdown(&self) -> String`

**依赖**：`pulldown-cmark`（解析）、`serde`（可选序列化）

### 2.2 `editor_engine`

**职责**：编辑操作与状态管理。

- 文本缓冲区（piece table 或 rope）
- 光标与选区管理
- 撤销 / 重做栈（command pattern）
- 编辑命令（插入、删除、格式化快捷键触发）
- 与 `document_model` 双向同步：源码改动 → AST 更新；AST 改动 → 源码重生成

**对外接口**：

- `Editor`：核心编辑器状态
- `Editor::apply(&mut self, cmd: Command)`
- `Editor::undo(&mut self)` / `Editor::redo(&mut self)`

**依赖**：`document_model`、`ropey`（文本缓冲）

### 2.3 `markdown_renderer`

**职责**：将 AST 渲染为 egui 组件。

- AST 节点 → egui `Widget` 映射
- 语法高亮（代码块）
- mermaid 图形渲染（后续阶段）
- 渲染缓存与增量更新

**对外接口**：

- `render(ctx: &egui::Context, doc: &Document, view: &View) -> Response`

  其中 `View` 封装光标、选区、滚动位置等编辑器状态，用于支持 hybrid 模式下渲染需感知光标位置的场景。具体字段在阶段 2 实施时补全。

**依赖**：`document_model`、`egui`、`syntect`（语法高亮）

### 2.4 `export_engine`

**职责**：导出为非编辑器格式。

- AST → HTML（带内联 CSS）
- AST → PDF（通过 `wkhtmltopdf` 中转）
- 主题样式应用

**对外接口**（统一返回字节内容，写入由调用方决定）：

- `export_html(doc: &Document, opts: &HtmlOptions) -> Result<Vec<u8>>`
- `export_pdf(doc: &Document, opts: &PdfOptions) -> Result<Vec<u8>>`

调用方（`zdown-app`）负责将 `Vec<u8>` 写入目标路径或弹窗预览。

**依赖**：`document_model`、`wkhtmltopdf` crate（PDF 导出，需用户系统安装 wkhtmltopdf）

### 2.5 `workspace`

**职责**：文件与项目管理。

- 打开 / 保存本地文件
- 最近文件列表
- 项目树（文件夹视图）
- 文件监听（外部修改检测）

**对外接口**：

- `Workspace::open(&mut self, path: &Path) -> Result<Document>`
- `Workspace::save(&mut self, doc: &Document) -> Result<()>`

**依赖**：`document_model`、`std::fs`、`notify`（文件监听）

### 2.6 `config`

**职责**：用户配置与个性化。

- 配置文件读写（TOML）
- 主题（亮色 / 暗色 / 自定义）
- 快捷键映射
- 字体设置
- 拼写检查开关
- 多语言（i18n）字符串表

**对外接口**：

- `Config::load() -> Result<Config>`
- `Config::save(&self) -> Result<()>`
- `Theme`、`Keymap` 等子结构

**依赖**：`serde`、`toml`、`fluent`（i18n，待选型）

### 2.7 `zdown-app`

**职责**：egui 应用入口与 UI 编排。

- 窗口管理
- 菜单栏、工具栏、状态栏
- tab 管理（多文件）
- 命令面板（Ctrl+Shift+P）
- 终端面板（后续阶段）
- 调用其他 crate 编排完整体验

**对外接口**：二进制 crate，`main.rs` 入口。

**依赖**：上述所有 crate、`eframe`、`egui`

## 3. 跨层约定

### 3.1 错误处理

- 所有 crate 定义自己的 `Error` 类型，实现 `std::error::Error`
- 跨层传递时用 `thiserror` 派生
- 禁止 `unwrap()` / `expect()`（见 AGENTS.md 编码标准）
- 优先 `Result<T, E>`，必要时 `Option<T>`

### 3.2 日志

- 统一使用 `tracing` + `tracing-subscriber`
- `zdown-app` 初始化全局 subscriber
- 各 crate 用 `tracing::info!` / `warn!` / `error!`

### 3.3 平台抽象

- 路径用 `Path` / `PathBuf`，不硬编码分隔符
- 平台差异封装在 `workspace` 与 `zdown-app` 内
- 不在 Core / Render 层出现平台判断

### 3.4 线程模型

- UI 线程：egui 主循环
- IO 线程：文件读写、网络（图床上传）走 `std::thread` 或 `rayon`
- 通道：`crossbeam-channel` 或 `std::sync::mpsc` 向 UI 线程回传结果

## 4. 测试策略

每个 crate 独立测试：

- `document_model`：解析正确性、往返测试（source → AST → source 等价）
- `editor_engine`：命令应用、撤销重做、光标边界
- `markdown_renderer`：快照测试（给定 AST → 期望 widget 树）
- `export_engine`：HTML 输出快照、PDF 生成冒烟测试
- `workspace`：临时目录集成测试
- `config`：TOML 往返、默认值
- `zdown-app`：集成测试（eframe 测试模式）

覆盖率目标 ≥ 80%（见 AGENTS.md）。

## 5. 选型决策（已敲定）

| 项目 | 决策 | 备注 |
| --- | --- | --- |
| 文本缓冲区 | `ropey::Rope` | 不自实现 piece table |
| PDF 导出 | `wkhtmltopdf` crate | 需用户系统安装 wkhtmltopdf；排版质量优先 |
| mermaid 渲染 | `mermaid-cli`（调用本地 node） | 需用户系统安装 Node + mermaid-cli |
| 插件系统 | `wasmtime`（WASM 沙箱） | 安全隔离优先 |
| 终端嵌入 | `portable-pty` + 自绘 | 不复用第三方 egui-terminal 封装 |
| AI 续写 | 留接口，后续决定 | `CompletionProvider` trait，本地/远程实现待定 |
| i18n 框架 | `fluent` | 阶段 4 引入 |
