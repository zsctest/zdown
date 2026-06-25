# zdown

[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)]()

一个基于 Rust 的跨平台 Markdown 编辑器，提供所见即所得的编辑体验，支持源码视图、多标签页、图床管理、PDF/HTML 导出等功能。

A cross-platform Markdown editor built with Rust, featuring hybrid WYSIWYG editing, source view, multi-tab, image hosting, PDF/HTML export and more.

---

## ✨ 功能 / Features

### 编辑 / Editing

- **Hybrid WYSIWYG** — 所见即所得编辑模式，同时支持一键切换源码视图
- **多标签页** — 同时打开和编辑多个 Markdown 文件
- **自定义快捷键** — 可视化快捷键映射配置，支持任意组合键
- **语法高亮** — 基于 syntect 的编辑器语法高亮

### 导出 / Export

- **PDF 导出** — 支持字体选择、自定义 CSS 样式
- **HTML 导出** — 完整保留渲染样式，支持自定义 CSS

### 图床 / Image Hosting

5 种存储策略，覆盖本地到云端：

| 策略 | 说明 |
|---|---|
| **本地存储** | 复制图片到文档目录的 `images/` 子目录 |
| **Base64 内联** | 将图片编码为 data URI 直接嵌入 Markdown |
| **SM.MS** | 免费云端图床，可选 API Token 提升限额 |
| **腾讯云 COS** | 腾讯云对象存储，支持 CDN 域名、路径模板 |
| **PicGo 桥接** | 通过 PicGo HTTP Server 接入 60+ 图床（GitHub、Imgur、阿里云 OSS、七牛、S3 等） |

### 渲染 / Rendering

- **Mermaid 图表** — 支持流程图、时序图、甘特图、类图等
- **HTML 内联渲染** — 支持内联 HTML 标签和 CSS 样式
- **大綱面板** — 文档结构树，支持折叠/展开、拖拽重排序、关键词过滤

### 外观 / Appearance

- **暗色 / 亮色主题** — 一键切换
- **自定义 CSS** — 全局样式设置，追加到内置样式之后
- **中英双语** — 完整的中文和英文界面本地化

### 工具 / Tools

- **内置终端** — 嵌入命令行终端，支持 PowerShell、bash、zsh 等
- **英文拼写检查** — 基于 spellbook 的实时拼写检查，红色波浪线标记

---

## 🏗️ 架构 / Architecture

项目采用分层架构，各层通过 trait 解耦，依赖单向向下。

```
┌─────────────────────────────────────────────────┐
│  UI Layer       zdown-app (egui/eframe)          │
├─────────────────────────────────────────────────┤
│  Core Layer     editor_engine → document_model   │
├─────────────────────────────────────────────────┤
│  Render Layer   markdown_renderer                │
│                 export_engine (PDF/HTML)          │
│                 html_renderer | mermaid_renderer  │
├─────────────────────────────────────────────────┤
│  Storage Layer  workspace | config | i18n         │
└─────────────────────────────────────────────────┘
```

12 个独立 crate：

| Crate | 职责 |
|---|---|
| `zdown-app` | egui/eframe UI 层，菜单、设置、标签页管理 |
| `editor_engine` | 文本编辑引擎，光标/选区/撤销重做 |
| `document_model` | Markdown 解析（pulldown-cmark）+ 序列化 |
| `markdown_renderer` | Markdown → egui 富文本渲染 |
| `export_engine` | PDF (genpdf) + HTML 导出 |
| `html_renderer` | 内联 HTML/CSS 渲染 |
| `mermaid_renderer` | Mermaid 图表渲染 |
| `workspace` | 文件打开/保存/最近文件 |
| `config` | TOML 配置持久化 |
| `i18n` | Fluent 国际化（zh-CN / en-US） |
| `spellcheck` | 英文拼写检查 |
| `terminal_panel` | 内嵌终端（alacritty_terminal） |

---

## 🚀 快速开始 / Quick Start

### 前置要求 / Prerequisites

- [Rust](https://www.rust-lang.org/) ≥ 1.85（2024 Edition）
- Windows / macOS / Linux

### 编译运行 / Build & Run

```bash
git clone https://github.com/zsctest/zdown.git
cd zdown
cargo run --release -p zdown-app
```

### 使用 PicGo 图床 / Using PicGo

```bash
# 安装 PicGo（需要 Node.js）
npm install -g picgo

# 配置一个上传器（例如 GitHub）
picgo set uploader github

# 启动 PicGo HTTP Server
picgo server
```

然后在 zdown 设置 → 图片 → 选择 "PicGo" → 保存。

---

## ⚙️ 配置 / Configuration

配置文件位于：

| 平台 | 路径 |
|---|---|
| Windows | `%APPDATA%\zdown\config.toml` |
| Linux | `~/.config/zdown/config.toml` |
| macOS | `~/Library/Application Support/zdown/config.toml` |

```toml
[image_hosting]
default_strategy = "Local"   # Local | Base64 | SmMs | TencentCos | PicGo
local_dir = "images"

[image_hosting.smms]
api_token = ""

[image_hosting.tencent_cos]
secret_id = ""
secret_key = ""
bucket = ""
region = "ap-guangzhou"
custom_domain = ""
upload_path = "zdown/{year}/{month}"

[image_hosting.picgo]
server_port = 36677
```

大部分设置可通过 **设置对话框**（`Ctrl+,`）可视化修改，无需手动编辑配置文件。

---

## 🧪 开发 / Development

```bash
# 运行所有测试
cargo test

# 代码格式检查
cargo fmt --check

# Clippy 检查
cargo clippy -- -D warnings

# 提交前一键检查
cargo fmt && cargo clippy && cargo test
```

### 编码标准 / Coding Standards

- Rust 2024 Edition
- 禁止 `unwrap()` / `expect()`，优先使用 `Result<T, E>`
- clippy clean, rustfmt clean
- 测试覆盖率目标 ≥ 80%

### 提交规范 / Commit Convention

遵循 [Conventional Commits](https://www.conventionalcommits.org/)：
- `feat(...):` 新功能
- `fix(...):` 修复
- `chore(...):` 工程化
- `docs(...):` 文档
- `refactor(...):` 重构

---

## 🗺️ 路线图 / Roadmap

- [x] Hybrid WYSIWYG 编辑
- [x] 源码视图 + 语法高亮
- [x] 多标签页
- [x] 暗色/亮色主题
- [x] 自定义 CSS
- [x] 大綱面板（折叠/展开/拖拽排序）
- [x] 命令行终端
- [x] 拼写检查
- [x] 中英双语
- [x] 图床（本地/Base64/SM.MS/COS/PicGo）
- [x] HTML 内联渲染
- [x] Mermaid 图表
- [x] 导出 PDF / HTML
- [x] 自定义快捷键
- [ ] AI 续写
- [ ] 自定义字体
- [ ] 增强的外观设置（字号、行距等）
- [ ] 插件扩展系统

---

## 📄 许可 / License

Licensed under either of [MIT License](LICENSE-MIT) or [Apache License 2.0](LICENSE-APACHE) at your option.
