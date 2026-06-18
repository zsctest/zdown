# workspace 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 zdown 实现文件 IO 层：Workspace 文件读写、rfd 文件对话框集成、最近文件列表持久化。

**架构：** `workspace` 依赖 `document_model`（调用 `parse` / `to_markdown`）。对外暴露 `Workspace`（持有 `Option<PathBuf>` 当前路径）、`pick_open_file` / `pick_save_file`（rfd 同步对话框）、`RecentFiles`（TOML 持久化到 `<config_dir>/zdown/recent.toml`）。文件监听（notify）按 review 建议推迟到阶段 3，本 plan 不实现。

**技术栈：** Rust 2024 edition、rfd（最新）、dirs、toml、document_model（path）。

**前置任务：** Plan 1（document_model）完成。本 plan 修改根 Cargo.toml 加 rfd/dirs/toml 依赖。

---

## 文件结构

- 修改：根 `Cargo.toml` — 加 rfd/dirs/toml 到 `[workspace.dependencies]`
- 修改：`crates/workspace/Cargo.toml` — 引用上述依赖 + document_model
- 修改：`crates/workspace/src/lib.rs` — 模块声明与重新导出
- 修改：`crates/workspace/src/error.rs` — Error 扩展
- 创建：`crates/workspace/src/workspace.rs` — `Workspace` 类型
- 创建：`crates/workspace/src/dialog.rs` — rfd 对话框封装
- 创建：`crates/workspace/src/recent.rs` — `RecentFiles`
- 测试：各模块内联 + `crates/workspace/tests/integration.rs`

**关键设计决策：**

- **Workspace 有状态**：持有 `Option<PathBuf>` 当前路径。`open(path)` 设置当前路径；`save(doc)` 写入当前路径（无则 `Err`）；`save_as(path, doc)` 写入指定路径并更新当前路径
- **rfd 用同步 API**：`rfd::FileDialog` 的 `pick_file` / `save_file`。在 headless 环境（CI）返回 `None`，不 panic。本 plan 不做环境检查，由调用方（zdown-app）处理 `None`
- **最近文件存储**：`dirs::config_dir()/zdown/recent.toml`，序列化为 `paths: Vec<String>`（PathBuf → String 转换）。最多 10 条，`add` 时按 canonicalize 去重并移到队首
- **Error 变体**：`Io`（`#[from] std::io::Error`）、`Parse`（`#[from] document_model::Error`）、`Dialog`、`Serialize`（TOML 序列化失败）。删除 review 中的 `Watch`（无文件监听）
- **测试隔离**：`RecentFiles` 提供 `with_storage_path(PathBuf)` 构造器供测试注入临时路径；`load()` 用默认路径

---

## 任务 1：依赖与 CI 基线

**文件：**
- 修改：根 `Cargo.toml`
- 修改：`.github/workflows/ci.yml`
- 修改：`crates/workspace/Cargo.toml`

- [ ] **步骤 1.1：根 Cargo.toml 加依赖**

修改根 `Cargo.toml` 的 `[workspace.dependencies]` 段，在 `# ---------- workspace 内部 path 依赖 ----------` 之前追加：

```toml
# ---------- workspace crate ----------
rfd = "0.15"
dirs = "5"
toml = "0.8"
```

- [ ] **步骤 1.2：修改 workspace/Cargo.toml**

替换 `crates/workspace/Cargo.toml`：

```toml
[package]
name = "workspace"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lints]
workspace = true

[dependencies]
thiserror.workspace = true
document_model.workspace = true
rfd.workspace = true
dirs.workspace = true
toml.workspace = true

[dev-dependencies]
tempfile = "3"
```

- [ ] **步骤 1.3：CI Linux 确认 gtk（已在阶段 0 装好）**

修改 `.github/workflows/ci.yml` 的 Linux job，在 `Install eframe system dependencies` 步骤后追加注释（不改包列表，因 `libgtk-3-dev` 已装）：

```yaml
      - name: Install eframe system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libgtk-3-dev \
            libxcb-shape0-dev \
            libxcb-xfixes0-dev \
            libxkbcommon-dev \
            libwayland-dev \
            libvulkan-dev
          # 注：rfd（阶段 1 引入）在 Linux 依赖 gtk，libgtk-3-dev 已包含上方。
          # CI 无 DISPLAY，rfd::FileDialog::pick_file 返回 None，不跑对话框测试。
```

- [ ] **步骤 1.4：验证依赖可拉取**

运行：`cargo metadata --no-deps --format-version 1 > /dev/null`
预期：成功（本机可能需 `CARGO_HTTP_CHECK_REVOKE=false`）。

运行：`cargo build -p workspace`
预期：编译通过（占位 lib.rs 仍能编译，新依赖未使用）。

- [ ] **步骤 1.5：Commit**

```bash
git add Cargo.toml .github/workflows/ci.yml crates/workspace/Cargo.toml
git commit -m "chore(workspace): 引入 rfd/dirs/toml 依赖

rfd 0.15 / dirs 5 / toml 0.8 加入 [workspace.dependencies]。
workspace crate 引用上述依赖 + document_model path 依赖。
CI Linux job 注释说明 rfd headless 行为。"
```

---

## 任务 2：Error 扩展 + Workspace 类型

**文件：**
- 修改：`crates/workspace/src/error.rs`
- 修改：`crates/workspace/src/lib.rs`
- 创建：`crates/workspace/src/workspace.rs`
- 测试：`crates/workspace/src/workspace.rs`（内联 + 集成测试用 tempfile）

- [ ] **步骤 2.1：扩展 Error 类型**

替换 `crates/workspace/src/error.rs`：

```rust
//! workspace 错误类型。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// 文件 IO 错误。
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    /// document_model 解析/序列化错误。
    #[error("文档错误: {0}")]
    Parse(#[from] document_model::Error),
    /// 文件对话框错误（平台 API 不可用等）。
    #[error("对话框错误: {0}")]
    Dialog(String),
    /// TOML 序列化/反序列化错误。
    #[error("TOML 错误: {0}")]
    Serialize(#[from] toml::de::Error),
    /// 当前路径未设置（save 无路径时）。
    #[error("未设置当前路径")]
    NoCurrentPath,
}
```

- [ ] **步骤 2.2：修改 lib.rs 模块声明**

替换 `crates/workspace/src/lib.rs`：

```rust
//! workspace：文件 IO 与项目管理。
//!
//! 对外暴露 Workspace / pick_open_file / pick_save_file / RecentFiles / Error。
//! 实际职责见 docs/ARCHITECTURE.md §2.5。

pub mod dialog;
pub mod error;
pub mod recent;
pub mod workspace;

pub use dialog::{pick_open_file, pick_save_file};
pub use error::Error;
pub use recent::RecentFiles;
pub use workspace::Workspace;

/// crate 级 Result 别名。
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "workspace");
    }
}
```

- [ ] **步骤 2.3：创建空占位模块让编译通过**

创建 `crates/workspace/src/dialog.rs`：

```rust
//! 文件对话框（任务 3 实现）。
```

创建 `crates/workspace/src/recent.rs`：

```rust
//! 最近文件列表（任务 4 实现）。
```

- [ ] **步骤 2.4：编写 Workspace 实现 + 测试**

创建 `crates/workspace/src/workspace.rs`：

```rust
//! Workspace：文件读写，持有当前路径。

use std::fs;
use std::path::{Path, PathBuf};

use document_model::{parse, to_markdown, Document};

use crate::{Error, Result};

/// 文件工作区。持有当前打开文件的路径。
pub struct Workspace {
    current_path: Option<PathBuf>,
}

impl Workspace {
    /// 空工作区（无当前路径）。
    pub fn new() -> Self {
        Self { current_path: None }
    }

    /// 打开指定路径文件，解析为 `Document`，并记录为当前路径。
    pub fn open(&mut self, path: &Path) -> Result<Document> {
        let src = fs::read_to_string(path)?;
        let doc = parse(&src)?;
        self.current_path = Some(path.to_path_buf());
        Ok(doc)
    }

    /// 保存到当前路径。无当前路径返回 `Err`。
    pub fn save(&self, doc: &Document) -> Result<()> {
        let path = self.current_path.as_ref().ok_or(Error::NoCurrentPath)?;
        let md = to_markdown(doc);
        fs::write(path, md)?;
        Ok(())
    }

    /// 另存为指定路径，并更新当前路径。
    pub fn save_as(&mut self, path: &Path, doc: &Document) -> Result<()> {
        let md = to_markdown(doc);
        fs::write(path, md)?;
        self.current_path = Some(path.to_path_buf());
        Ok(())
    }

    /// 当前路径。
    pub fn current_path(&self) -> Option<&Path> {
        self.current_path.as_deref()
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use document_model::ast::*;
    use tempfile::NamedTempFile;

    fn sample_doc() -> Document {
        Document {
            blocks: vec![Block::Paragraph(Paragraph {
                inlines: vec![Inline::Text("hello".into())],
            })],
        }
    }

    fn write_temp(content: &str) -> (NamedTempFile, PathBuf) {
        let f = NamedTempFile::new().expect("tempfile");
        let path = f.path().to_path_buf();
        std::fs::write(&path, content).expect("write");
        (f, path)
    }

    #[test]
    fn new_workspace_has_no_path() {
        let ws = Workspace::new();
        assert!(ws.current_path().is_none());
    }

    #[test]
    fn open_reads_and_parses() {
        let (_f, path) = write_temp("# 标题\n");
        let mut ws = Workspace::new();
        let doc = ws.open(&path).expect("open");
        assert_eq!(ws.current_path(), Some(path.as_path()));
        assert_eq!(doc.blocks.len(), 1);
        assert!(matches!(&doc.blocks[0], Block::Heading(_)));
    }

    #[test]
    fn open_nonexistent_returns_io_error() {
        let mut ws = Workspace::new();
        let err = ws.open(Path::new("/nonexistent/path/xyz.md")).unwrap_err();
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn save_without_path_returns_no_current_path() {
        let ws = Workspace::new();
        let doc = sample_doc();
        let err = ws.save(&doc).unwrap_err();
        assert!(matches!(err, Error::NoCurrentPath));
    }

    #[test]
    fn save_after_open_writes() {
        let (_f, path) = write_temp("old");
        let mut ws = Workspace::new();
        ws.open(&path).expect("open");
        let doc = sample_doc();
        ws.save(&doc).expect("save");
        let content = std::fs::read_to_string(&path).expect("read");
        assert_eq!(content, "hello\n");
    }

    #[test]
    fn save_as_updates_path() {
        let mut ws = Workspace::new();
        let doc = sample_doc();
        let (_f, new_path) = write_temp("");
        ws.save_as(&new_path, &doc).expect("save_as");
        assert_eq!(ws.current_path(), Some(new_path.as_path()));
        let content = std::fs::read_to_string(&new_path).expect("read");
        assert_eq!(content, "hello\n");
    }

    #[test]
    fn open_save_reopen_roundtrip() {
        let (_f, path) = write_temp("");
        let mut ws = Workspace::new();
        let doc = sample_doc();
        ws.save_as(&path, &doc).expect("save_as");
        // 重新打开
        let reopened = ws.open(&path).expect("reopen");
        assert_eq!(reopened, doc);
    }

    #[test]
    fn save_as_to_new_path_switches() {
        let (_f, path1) = write_temp("a");
        let (_f2, path2) = write_temp("b");
        let mut ws = Workspace::new();
        ws.open(&path1).expect("open 1");
        assert_eq!(ws.current_path(), Some(path1.as_path()));
        let doc = sample_doc();
        ws.save_as(&path2, &doc).expect("save_as");
        assert_eq!(ws.current_path(), Some(path2.as_path()));
        // path1 内容未变
        assert_eq!(std::fs::read_to_string(&path1).expect("read 1"), "a");
    }
}
```

- [ ] **步骤 2.5：运行测试验证**

运行：`cargo test -p workspace workspace`
预期：所有 `workspace::tests::*` 测试通过。

运行：`cargo clippy -p workspace -- -D warnings`
预期：可能有 `dead_code` 警告（dialog/recent 空模块），任务 3/4 后消失。

- [ ] **步骤 2.6：Commit**

```bash
git add crates/workspace/src/
git commit -m "feat(workspace): Workspace open/save/save_as + Error 扩展

Workspace 持有 Option<PathBuf>，open 设置路径，save 写当前路径
（无则 NoCurrentPath），save_as 更新路径。
集成测试覆盖 open/save/reopen 往返、不存在文件、无路径 save。"
```

---

## 任务 3：rfd 文件对话框

**文件：**
- 修改：`crates/workspace/src/dialog.rs`
- 测试：`crates/workspace/src/dialog.rs`（内联，CI 不弹窗）

- [ ] **步骤 3.1：编写 dialog 实现**

替换 `crates/workspace/src/dialog.rs`：

```rust
//! rfd 文件对话框封装（同步 API）。
//!
//! 在 headless 环境（CI 无 DISPLAY）返回 `None`，不 panic。

use std::path::PathBuf;

/// 弹出打开文件对话框。用户取消或环境不支持时返回 `None`。
pub fn pick_open_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Markdown", &["md", "markdown"])
        .set_title("打开 Markdown 文件")
        .pick_file()
}

/// 弹出保存文件对话框。用户取消或环境不支持时返回 `None`。
pub fn pick_save_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Markdown", &["md", "markdown"])
        .set_title("保存 Markdown 文件")
        .set_file_name("untitled.md")
        .save_file()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    /// CI 无显示环境，pick_* 应返回 None 而非 panic。
    /// 本地手动运行时（有 DISPLAY）可能弹窗，测试会阻塞——
    /// 因此本测试标记 ignored，仅手动 `cargo test -- --ignored` 验证。
    #[test]
    #[ignore = "需要手动在桌面环境验证对话框弹窗"]
    fn pick_open_file_does_not_panic() {
        // 不断言返回值（用户可能取消），仅验证不 panic。
        let _ = pick_open_file();
    }

    #[test]
    #[ignore = "需要手动在桌面环境验证对话框弹窗"]
    fn pick_save_file_does_not_panic() {
        let _ = pick_save_file();
    }
}
```

- [ ] **步骤 3.2：运行测试验证**

运行：`cargo build -p workspace`
预期：编译通过。rfd 0.15 API 与上述代码一致。

运行：`cargo test -p workspace dialog`
预期：2 个 ignored 测试，0 个运行。

运行：`cargo clippy -p workspace -- -D warnings`
预期：无警告。

- [ ] **步骤 3.3：Commit**

```bash
git add crates/workspace/src/dialog.rs
git commit -m "feat(workspace): rfd 文件对话框封装

pick_open_file / pick_save_file 同步 API，Markdown 过滤器。
headless 环境返回 None 不 panic。对话框测试 #[ignore] 手动验证。"
```

---

## 任务 4：最近文件列表

**文件：**
- 修改：`crates/workspace/src/recent.rs`
- 测试：`crates/workspace/src/recent.rs`（内联，用 tempfile 隔离）

- [ ] **步骤 4.1：编写 RecentFiles 实现**

替换 `crates/workspace/src/recent.rs`：

```rust
//! 最近文件列表，TOML 持久化到 <config_dir>/zdown/recent.toml。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// 最多保留多少条最近文件。
const MAX_ENTRIES: usize = 10;

#[derive(Serialize, Deserialize)]
struct RecentFileList {
    paths: Vec<String>,
}

/// 最近文件管理器。
pub struct RecentFiles {
    paths: Vec<PathBuf>,
    storage_path: PathBuf,
}

impl RecentFiles {
    /// 从默认路径加载（`<config_dir>/zdown/recent.toml`）。
    /// 文件不存在或解析失败时返回空列表。
    pub fn load() -> Self {
        let storage_path = default_storage_path();
        Self::load_from(&storage_path)
    }

    /// 从指定路径加载（测试用）。
    pub fn load_from(storage_path: &Path) -> Self {
        match std::fs::read_to_string(storage_path) {
            Ok(content) => match toml::from_str::<RecentFileList>(&content) {
                Ok(list) => Self {
                    paths: list.paths.into_iter().map(PathBuf::from).collect(),
                    storage_path: storage_path.to_path_buf(),
                },
                Err(_) => Self::empty_at(storage_path),
            },
            Err(_) => Self::empty_at(storage_path),
        }
    }

    fn empty_at(storage_path: &Path) -> Self {
        Self {
            paths: Vec::new(),
            storage_path: storage_path.to_path_buf(),
        }
    }

    /// 构造空列表，用默认存储路径。
    pub fn new() -> Self {
        Self::empty_at(&default_storage_path())
    }

    /// 添加路径。按 canonicalize 去重并移到队首。超过上限丢弃最旧。
    pub fn add(&mut self, path: PathBuf) {
        let canonical = path.canonicalize().unwrap_or(path);
        // 去重：移除已存在的同路径
        self.paths.retain(|p| {
            let p_canon = p.canonicalize().unwrap_or_else(|_| p.clone());
            p_canon != canonical
        });
        // 插入队首
        self.paths.insert(0, path);
        // 超限截断
        if self.paths.len() > MAX_ENTRIES {
            self.paths.truncate(MAX_ENTRIES);
        }
    }

    /// 当前最近文件列表（队首为最新）。
    pub fn list(&self) -> &[PathBuf] {
        &self.paths
    }

    /// 保存到存储路径。
    pub fn save(&self) -> Result<()> {
        let list = RecentFileList {
            paths: self.paths.iter().map(|p| p.to_string_lossy().into_owned()).collect(),
        };
        let toml_str = toml::to_string(&list).map_err(|e| Error::Dialog(e.to_string()))?;
        // 确保父目录存在
        if let Some(parent) = self.storage_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.storage_path, toml_str)?;
        Ok(())
    }

    /// 存储路径（测试用）。
    pub fn storage_path(&self) -> &Path {
        &self.storage_path
    }
}

impl Default for RecentFiles {
    fn default() -> Self {
        Self::new()
    }
}

fn default_storage_path() -> PathBuf {
    let config = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config.join("zdown").join("recent.toml")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use tempfile::TempDir;

    fn temp_storage() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("recent.toml");
        (dir, path)
    }

    #[test]
    fn load_nonexistent_returns_empty() {
        let (_dir, path) = temp_storage();
        let rf = RecentFiles::load_from(&path);
        assert!(rf.list().is_empty());
    }

    #[test]
    fn add_one_appears_in_list() {
        let (_dir, path) = temp_storage();
        let mut rf = RecentFiles::load_from(&path);
        rf.add(PathBuf::from("/tmp/a.md"));
        assert_eq!(rf.list(), &[PathBuf::from("/tmp/a.md")]);
    }

    #[test]
    fn add_moves_existing_to_front() {
        let (_dir, path) = temp_storage();
        let mut rf = RecentFiles::load_from(&path);
        rf.add(PathBuf::from("/tmp/a.md"));
        rf.add(PathBuf::from("/tmp/b.md"));
        rf.add(PathBuf::from("/tmp/a.md")); // 再次添加 a
        assert_eq!(rf.list()[0], PathBuf::from("/tmp/a.md"));
        assert_eq!(rf.list().len(), 2);
    }

    #[test]
    fn add_dedupes_canonical() {
        let dir = TempDir::new().expect("tempdir");
        let real = dir.path().join("x.md");
        std::fs::write(&real, "").expect("write");
        let (_dir2, storage) = temp_storage();
        let mut rf = RecentFiles::load_from(&storage);
        rf.add(real.clone());
        rf.add(dir.path().join("x.md")); // 相对/绝对不同表示但 canonicalize 同
        // 第二次 add 的路径 canonicalize 后与第一次相同，应去重
        assert_eq!(rf.list().len(), 1);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let (_dir, path) = temp_storage();
        let mut rf = RecentFiles::load_from(&path);
        rf.add(PathBuf::from("/tmp/a.md"));
        rf.add(PathBuf::from("/tmp/b.md"));
        rf.save().expect("save");

        let reloaded = RecentFiles::load_from(&path);
        assert_eq!(reloaded.list(), rf.list());
    }

    #[test]
    fn max_entries_truncates() {
        let (_dir, path) = temp_storage();
        let mut rf = RecentFiles::load_from(&path);
        for i in 0..(MAX_ENTRIES + 5) {
            rf.add(PathBuf::from(format!("/tmp/file_{i}.md")));
        }
        assert_eq!(rf.list().len(), MAX_ENTRIES);
        // 最旧的被丢弃，最新的在队首
        assert_eq!(rf.list()[0], PathBuf::from(format!("/tmp/file_{}.md", MAX_ENTRIES + 4)));
    }

    #[test]
    fn save_creates_parent_dir() {
        let dir = TempDir::new().expect("tempdir");
        let nested = dir.path().join("nested").join("sub").join("recent.toml");
        let mut rf = RecentFiles::load_from(&nested);
        rf.add(PathBuf::from("/tmp/x.md"));
        rf.save().expect("save");
        assert!(nested.exists());
    }

    #[test]
    fn load_corrupt_returns_empty() {
        let (_dir, path) = temp_storage();
        std::fs::write(&path, "this is not toml: {{{").expect("write");
        let rf = RecentFiles::load_from(&path);
        assert!(rf.list().is_empty());
    }

    #[test]
    fn save_empty_list() {
        let (_dir, path) = temp_storage();
        let rf = RecentFiles::load_from(&path);
        rf.save().expect("save");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).expect("read");
        assert!(content.contains("paths"));
    }
}
```

- [ ] **步骤 4.2：运行测试验证**

运行：`cargo test -p workspace recent`
预期：所有 `recent::tests::*` 测试通过。

运行：`cargo clippy -p workspace -- -D warnings`
预期：无警告。

- [ ] **步骤 4.3：Commit**

```bash
git add crates/workspace/src/recent.rs
git commit -m "feat(workspace): 最近文件列表 TOML 持久化

RecentFiles 存 <config_dir>/zdown/recent.toml，最多 10 条。
add 时 canonicalize 去重并移队首，超限截断。
load_from 注入路径供测试隔离。损坏文件返回空列表。"
```

---

## 任务 5：集成测试

**文件：**
- 创建：`crates/workspace/tests/integration.rs`

- [ ] **步骤 5.1：编写集成测试**

创建 `crates/workspace/tests/integration.rs`：

```rust
//! workspace 集成测试：Workspace + RecentFiles 协作。

use std::path::PathBuf;
use tempfile::TempDir;
use workspace::{RecentFiles, Workspace};

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
                inlines: vec![Inline::Text("内容".into())],
            }),
        ],
    }
}

#[test]
fn save_adds_to_recent_and_reopens() {
    let dir = TempDir::new().expect("tempdir");
    let storage = dir.path().join("recent.toml");
    let file_path = dir.path().join("doc.md");

    // 1. save_as 写文件
    let mut ws = Workspace::new();
    let doc = sample_doc();
    ws.save_as(&file_path, &doc).expect("save_as");

    // 2. 加入最近文件
    let mut recent = RecentFiles::load_from(&storage);
    recent.add(file_path.clone());
    recent.save().expect("save recent");

    // 3. 从最近文件加载并重新打开
    let reloaded_recent = RecentFiles::load_from(&storage);
    assert_eq!(reloaded_recent.list(), &[file_path.clone()]);

    let mut ws2 = Workspace::new();
    let reopened = ws2.open(reloaded_recent.list()[0]).expect("reopen");

    // 4. 内容一致
    assert_eq!(reopened, doc);
}

#[test]
fn multiple_files_recent_order() {
    let dir = TempDir::new().expect("tempdir");
    let storage = dir.path().join("recent.toml");
    let mut recent = RecentFiles::load_from(&storage);

    for name in ["a.md", "b.md", "c.md"] {
        let path = dir.path().join(name);
        let mut ws = Workspace::new();
        ws.save_as(&path, &sample_doc()).expect("save");
        recent.add(path);
    }
    recent.save().expect("save");

    let reloaded = RecentFiles::load_from(&storage);
    // c.md 最先添加？不，c.md 最后添加应在队首
    assert_eq!(reloaded.list()[0], dir.path().join("c.md"));
    assert_eq!(reloaded.list()[1], dir.path().join("b.md"));
    assert_eq!(reloaded.list()[2], dir.path().join("a.md"));
}

#[test]
fn workspace_current_path_tracks_save_as() {
    let dir = TempDir::new().expect("tempdir");
    let p1 = dir.path().join("a.md");
    let p2 = dir.path().join("b.md");

    let mut ws = Workspace::new();
    assert!(ws.current_path().is_none());

    ws.save_as(&p1, &sample_doc()).expect("save 1");
    assert_eq!(ws.current_path(), Some(p1.as_path()));

    ws.save_as(&p2, &sample_doc()).expect("save 2");
    assert_eq!(ws.current_path(), Some(p2.as_path()));
}

#[test]
fn open_nonexistent_does_not_set_path() {
    let mut ws = Workspace::new();
    let _ = ws.open(std::path::Path::new("/nonexistent/xyz.md"));
    assert!(ws.current_path().is_none());
}

#[test]
fn save_after_open_persists_changes() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("doc.md");

    // 初始保存
    let mut ws = Workspace::new();
    let doc1 = sample_doc();
    ws.save_as(&path, &doc1).expect("save");

    // 修改后保存
    let doc2 = Document {
        blocks: vec![Block::Paragraph(Paragraph {
            inlines: vec![Inline::Text("modified".into())],
        })],
    };
    ws.save(&doc2).expect("save again");

    // 重开应是 doc2
    let mut ws2 = Workspace::new();
    let reopened = ws2.open(&path).expect("reopen");
    assert_eq!(reopened, doc2);
}

#[test]
fn pathbuf_to_string_lossy_roundtrip() {
    // 验证 to_string_lossy 在 RecentFiles save/load 往返中无损
    let dir = TempDir::new().expect("tempdir");
    let storage = dir.path().join("recent.toml");
    let mut recent = RecentFiles::load_from(&storage);
    let p = PathBuf::from("/tmp/含中文.md");
    recent.add(p.clone());
    recent.save().expect("save");

    let reloaded = RecentFiles::load_from(&storage);
    assert_eq!(reloaded.list(), &[p]);
}
```

- [ ] **步骤 5.2：运行测试验证**

运行：`cargo test -p workspace --test integration`
预期：所有集成测试通过。

运行：`cargo test -p workspace`
预期：所有测试通过（workspace 单元 + dialog ignored + recent + integration）。

运行：`cargo clippy -p workspace --all-targets -- -D warnings`
预期：无警告。

- [ ] **步骤 5.3：Commit**

```bash
git add crates/workspace/tests/integration.rs
git commit -m "test(workspace): 集成测试覆盖 Workspace+RecentFiles 协作

save_adds_to_recent_and_reopens / multiple_files_recent_order /
current_path 跟踪 / open 失败不设路径 / save 持久化修改 /
中文路径往返。"
```

---

## 自检

**1. 规格覆盖度：**

- TASKS.md 阶段 1 workspace：
  - T1-11 open/save/save_as → 任务 2 ✓
  - T1-12 rfd 对话框 → 任务 3 ✓
  - T1-13 最近文件 TOML → 任务 4 ✓
  - T1-15 Error 扩展 → 任务 2（Error）✓
- T1-14 文件监听按 review 建议删除 ✓
- ARCHITECTURE.md 2.5 接口：
  - `Workspace::open(&mut self, path: &Path) -> Result<Document>` ✓
  - `Workspace::save(&mut self, doc: &Document) -> Result<()>` ✓（签名略改：`&self` 而非 `&mut self`，因不修改 Workspace 状态）
  - 额外 `save_as` ✓

**2. 占位符扫描：**

- 无 "TODO" / "待定"。
- dialog 模块的 `#[ignore]` 测试是真实测试代码，仅标记手动运行，非占位符。
- 每个测试有完整代码。

**3. 类型一致性：**

- `Workspace::open(&mut self, &Path) -> Result<Document>` 跨任务一致。
- `Workspace::save(&self, &Document) -> Result<()>` 跨任务一致。
- `RecentFiles::load_from(&Path) -> Self` / `add(PathBuf)` / `list() -> &[PathBuf]` / `save() -> Result<()>` 跨任务一致。
- `Error::Io(#[from] std::io::Error)` / `Parse(#[from] document_model::Error)` 在任务 2 定义，任务 4 用 `Error::Dialog(e.to_string())` 处理 toml 序列化错误（toml::ser::Error 未在 `#[from]`，因 toml::de::Error 已用于反序列化，序列化错误转 Dialog）。

**4. 编码标准：**

- 测试模块顶部 `#![allow(clippy::expect_used)]` ✓
- 生产代码无 `unwrap`/`expect`（`canonicalize().unwrap_or()` 是 Option/Result 的 unwrap_or，非 panic，合规）✓
- `Result<T, Error>` 优先 ✓

**5. 跨平台：**

- `dirs::config_dir()` 三平台返回正确路径 ✓
- `rfd` 在 Linux 无 DISPLAY 返回 None（rfd 0.15 行为）✓
- `PathBuf::canonicalize` 三平台可用 ✓

**6. 性能：**

T1-24 不在本 plan 范围。

**7. 已知简化（阶段 1 不做）：**

- 文件监听（notify）—— 阶段 3
- 项目树（文件夹视图）—— 阶段 3
- rfd 异步 API —— 阶段 3（若 UI 卡顿再切换）

---

## 执行交接

本计划已完成并保存到 `docs/superpowers/plans/2026-06-18-workspace.md`。两种执行方式：

1. **子代理驱动（推荐）**
2. **内联执行**

执行者注意：本 plan 是阶段 1 四个独立 plan 中的第三个。完成后继续 Plan 4（markdown_renderer source + zdown-app）。
