# zdown 第三方依赖清单

选型原则：

1. **优先成熟生态**：用广泛使用、活跃维护的 crate，避免造轮子
2. **最小依赖**：能用 `std` 解决的不引入第三方
3. **跨平台**：必须在 Windows / Linux / macOS 三平台可用
4. **许可证兼容**：优先 MIT / Apache-2.0

下表按 crate 分组列出候选依赖。标注"待选型"的项需在阶段 0 或对应阶段开始前敲定。

---

## workspace 根

| crate | 用途 | 版本约束建议 |
| --- | --- | --- |
| `tracing` | 结构化日志 | `^0.1` |
| `tracing-subscriber` | 日志后端 | `^0.3` |
| `thiserror` | 错误类型派生 | `^1` |
| `serde` | 序列化框架 | `^1` |

---

## `document_model`

| crate | 用途 | 备注 |
| --- | --- | --- |
| `pulldown-cmark` | Markdown 解析 | 主流选择，CommonMark 兼容 |
| `serde` | AST 可选序列化 | 仅在需要跨 crate 序列化时启用 `serde::Serialize`/`Deserialize` derive |
| `thiserror` | 错误类型 | |

> 不引入 `serde_json`：document_model 不对外暴露 JSON 序列化。若插件系统（阶段 4）需要 JSON 表示 AST，由 `zdown-app` 或插件适配层在调用时转换，避免污染数据模型层。

---

## `editor_engine`

| crate | 用途 | 备注 |
| --- | --- | --- |
| `ropey` | Rope 文本缓冲 | 高效大文本编辑，成熟 |
| `document_model`（path） | AST 同步 | workspace 内部依赖 |
| `thiserror` | 错误类型 | |

**备选**：自实现 piece table（已否决）。`ropey` 已足够成熟，采用之。

---

## `markdown_renderer`

| crate | 用途 | 备注 |
| --- | --- | --- |
| `egui` | 即时模式 GUI | 版本由 workspace.dependencies 统一固定为 latest stable（不写死 `^0.27`，该版本已过时） |
| `syntect` | 语法高亮 | 支持 markdown 等 |
| `document_model`（path） | AST 输入 | |
| `thiserror` | 错误类型 | |

**mermaid 渲染（阶段 4）**：待选型，候选见 export_engine。

---

## `export_engine`

| crate | 用途 | 备注 |
| --- | --- | --- |
| `document_model`（path） | AST 输入 | |
| `wkhtmltopdf` crate | PDF 导出 | **需用户系统安装 wkhtmltopdf**，跨平台分发需在文档中说明 |
| `thiserror` | 错误类型 | |
| HTML 导出 | 自实现，不依赖第三方 | 模板 + AST 遍历 |

**PDF 导出方案（已选型）**：`wkhtmltopdf`。排版质量优先，依赖系统安装 wkhtmltopdf 二进制（用户文档需说明）。备选 `printpdf`（纯 Rust）若后续发现 wkhtmltopdf 分发成本过高再评估切换。

**接口约定**：HTML 与 PDF 导出统一返回 `Vec<u8>`，写入由调用方负责（见 ARCHITECTURE.md 2.4 节）。

---

## `workspace`

| crate | 用途 | 备注 |
| --- | --- | --- |
| `rfd` | 原生文件对话框 | **阶段 1 引入**，阶段 0 不依赖（避免提前引入 gtk 依赖拖慢 CI） |
| `notify` | 文件变更监听 | 检测外部修改，阶段 1 引入 |
| `document_model`（path） | 调用 parse / to_markdown | |
| `thiserror` | 错误类型 | |
| `tracing` | 日志 | |

---

## `config`

| crate | 用途 | 备注 |
| --- | --- | --- |
| `serde` | 配置序列化 | |
| `toml` | TOML 读写 | 配置文件格式 |
| `dirs` | 平台配置目录定位 | `~/.config`、`%APPDATA%` 等 |
| `thiserror` | 错误类型 | |

**i18n（阶段 4，已选型）**：`fluent`。Mozilla 成熟方案，复数与上下文支持好。

---

## `zdown-app`

| crate | 用途 | 备注 |
| --- | --- | --- |
| `eframe` | egui 应用框架 | 窗口、事件循环 |
| `egui` | UI | |
| `tracing-subscriber` | 日志初始化 | |
| 上述所有 workspace crate | 编排 | path 依赖 |

**终端面板（阶段 4，已选型）**：`portable-pty` + 自绘终端。不复用第三方 egui-terminal 封装，可控性高。

**mermaid 渲染（阶段 4，已选型）**：调用 `mermaid-cli`（需用户系统安装 Node + mermaid-cli）。不内嵌 JS 引擎。

**插件系统（阶段 4，已选型）**：`wasmtime`（WASM 沙箱）。安全隔离优先，体积代价可接受。

---

## 平台无关性约束

以下 crate 因平台限制需特别注意：

- `rfd`（阶段 1 引入）：在 Linux 依赖 `gtk`，CI Linux runner 需安装 `libgtk-3-dev`；阶段 0 不引入，CI 无需装 gtk
- `notify`（阶段 1 引入）：各平台后端不同，需三平台集成测试
- `portable-pty`（阶段 4 引入）：Windows / Unix 实现差异大，测试需覆盖两端
- `wkhtmltopdf`（阶段 4 引入）：需用户系统安装 wkhtmltopdf 二进制，分发文档需说明

---

## 版本固定策略

- workspace 根 `Cargo.toml` 统一管理版本（`[workspace.dependencies]`）
- 各 crate 通过 `dep.workspace = true` 引用
- 上线前生成 `Cargo.lock` 并提交（保证可复现构建）

> 注：根据项目 RULES.md，`Cargo.toml` 与 `Cargo.lock` 受保护，未经授权禁止删除。修改按正常编辑流程处理。
