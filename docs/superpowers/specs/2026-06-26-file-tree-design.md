# 文件树面板设计规格

**日期**: 2026-06-26
**状态**: 已批准
**关联需求**: 文件夹浏览与 Markdown 文件编辑

---

## 1. 功能概述

在左侧面板的大纲视图下方新增文件树面板，支持：
- 打开文件夹并浏览目录结构
- 双击/单击打开 `.md` 文件到标签页
- 右键菜单：打开、重命名、删除、在资源管理器中查看、新建 Markdown 文件
- 通过菜单项或拖拽打开文件夹
- 上下两段布局：大纲在上，文件树在下，可拖拽调节高度

---

## 2. 架构

### 2.1 新增模块

**`crates/zdown-app/src/file_tree.rs`** — 文件树面板的完整实现

```
file_tree.rs
├── FileTreeNode           — 树节点数据结构
├── FileTreeState          — 面板状态管理
│   ├── open_folder()      — 打开文件夹，扫描目录
│   ├── scan_directory()   — 递归读取目录条目构建节点树
│   ├── toggle_expand()    — 折叠/展开目录
│   ├── refresh()          — 重新扫描当前文件夹
│   └── find_by_path()     — 按路径查找节点索引
├── FileContextMenu        — 右键菜单状态
├── FileContextAction      — 右键菜单操作枚举
├── show_file_tree_panel() — UI 渲染入口
└── Tests
```

### 2.2 修改模块

| 文件 | 修改内容 |
|------|----------|
| `crates/zdown-app/src/main.rs` | ZdownApp 新增 `file_tree: FileTreeState`、拖拽处理、布局调整 |
| `crates/zdown-app/src/menu.rs` | 新增"打开文件夹"菜单项 + `trigger_open_folder()` |
| `crates/zdown-app/src/input.rs` | 可能调整焦点逻辑（文件树点击不触发编辑器焦点） |
| `crates/i18n/` | 新增国际化键 |

### 2.3 依赖

- `rfd`（已有）：原生文件夹选择对话框 (`pick_folder`)
- `walkdir`：高效的递归目录遍历（或直接用 `std::fs::read_dir` 按需展开，避免一次性全量扫描）
- `egui`（已有）：UI 渲染、拖拽事件 (`dropped_files`)

---

## 3. 数据结构

### 3.1 FileTreeNode

```rust
/// 文件树中的一个节点。
/// 节点按深度优先、扁平化存储在 Vec 中，节点间通过索引引用父子关系。
#[derive(Debug, Clone)]
struct FileTreeNode {
    /// 显示名称（文件名或目录名）。
    name: String,
    /// 文件系统绝对路径。
    path: PathBuf,
    /// 是否为目录。
    is_dir: bool,
    /// 子节点在 nodes Vec 中的起始索引（仅目录有效）。
    /// 子节点范围: [first_child, next_sibling) 或到有下一个同深度的节点为止。
    first_child: Option<usize>,
    /// 下一个兄弟节点索引（同一目录下），用于遍历。
    next_sibling: Option<usize>,
    /// 是否隐藏文件（以 . 开头）。
    hidden: bool,
    /// 缩进层级（0 = 根目录）。
    depth: usize,
}
```

### 3.2 FileTreeState

```rust
/// 文件树面板的完整状态。
#[derive(Debug, Clone)]
struct FileTreeState {
    /// 当前打开的文件夹路径。None 表示未打开文件夹。
    root_path: Option<PathBuf>,
    /// 扁平化存储的所有节点。
    nodes: Vec<FileTreeNode>,
    /// 已展开的目录路径集合。
    expanded: BTreeSet<PathBuf>,
    /// 右键菜单状态。
    context_menu: Option<FileContextMenu>,
    /// 内联重命名状态（节点索引 + 正在编辑的新名称）。
    renaming: Option<(usize, String)>,
}
```

### 3.3 FileContextMenu

```rust
struct FileContextMenu {
    node_index: usize,
    /// 菜单锚点（屏幕坐标）。
    anchor: egui::Pos2,
}

enum FileContextAction {
    Open,
    Rename,
    Delete,
    RevealInExplorer,
    NewMarkdownFile,
    ExpandAll,
    CollapseAll,
    CloseMenu,
}
```

---

## 4. UI 布局

### 4.1 整体布局

```
┌─ TopBottomPanel::top ──────────────────────────────────────┐
│  menu bar: 文件 | 编辑 | 视图                                │
├─ tab_bar ──────────────────────────────────────────────────┤
│  [README.md] [notes.md] [draft.md]                         │
├─ SidePanel::left ──────────────┬─ CentralPanel ────────────┤
│  ┌─ outline ────────────────┐  │                             │
│  │  # 第一章                 │  │  search bar (if visible)    │
│  │  ## 1.1 概述             │  │  ┌───────────────────────┐  │
│  │  # 第二章                 │  │  │ editor / preview      │  │
│  └──────────────────────────┘  │  │                       │  │
│  ── ResizeHandle ────────────  │  └───────────────────────┘  │
│  ┌─ file_tree ──────────────┐  │                             │
│  │  📁 docs/                │  │                             │
│  │  │ 📁 specs/             │  │                             │
│  │  │ │ 📄 design.md        │  │                             │
│  │  │ 📄 README.md          │  │                             │
│  │  📁 src/                 │  │                             │
│  │  📄 Cargo.toml           │  │                             │
│  └──────────────────────────┘  │                             │
└────────────────────────────────┴─────────────────────────────┘
```

### 4.2 尺寸策略

- 大纲面板默认高度：可用高度的 50%
- 文件树面板默认高度：可用高度的 50%
- 分隔线：`egui::ResizeHandle` 或自绘拖拽条（参考 `SidePanel::resizable` 实现）
- 大纲为空时：大纲区域自动收缩到最小高度（~60px），文件树占据剩余空间

### 4.3 未打开文件夹时的提示

文件树区域显示居中的弱文字：
> 打开文件夹以浏览 Markdown 文件
> 拖拽文件夹到窗口 或 文件 → 打开文件夹

### 4.4 节点渲染

每个节点一行，包含：
1. 缩进（depth × 16px）
2. 折叠/展开三角图标（仅目录，▶/▼）
3. 文件类型图标（📁 目录 | 📄 Markdown | 📄 其他文件）
4. 文件名文本
5. Markdown 文件：鼠标悬停显示完整路径 tooltip
6. 右键弹出菜单

---

## 5. 交互行为

### 5.1 单击

| 目标 | 行为 |
|------|------|
| 目录（三角区域） | 折叠/展开该目录 |
| 目录（名称区域） | 折叠/展开该目录 |
| `.md` 文件 | 在新标签页打开（若已在某标签页打开则切换到该页） |
| 非 `.md` 文件 | 显示提示 "不支持的文件类型" |

### 5.2 双击

| 目标 | 行为 |
|------|------|
| `.md` 文件 | 同单击，在新标签页打开 |

### 5.3 右键菜单

#### 文件节点

| 菜单项 | 行为 |
|--------|------|
| 打开 | 在新标签页打开 |
| 重命名 | 进入内联编辑模式 |
| 删除 | 弹出确认框，确认后删除文件 |
| 在文件管理器中显示 | `open::that(path.parent())` |

#### 目录节点

| 菜单项 | 行为 |
|--------|------|
| 展开全部 | 递归展开所有子目录 |
| 折叠全部 | 递归折叠所有子目录 |
| 新建 Markdown 文件 | 弹出输入框，创建 `新文件.md` 并在编辑器中打开 |
| 重命名 | 进入内联编辑模式 |
| 删除 | 弹出确认框，确认后递归删除目录 |
| 在文件管理器中显示 | `open::that(path)` |

### 5.4 内联重命名

- 点击"重命名"后，节点名称变为 `TextEdit` 输入框
- Enter 确认，Esc 取消
- 重命名后刷新文件树并更新已打开标签页中的路径引用

### 5.5 新建 Markdown 文件

- 在目录节点上右键 → "新建 Markdown 文件"
- 输入文件名（自动追加 `.md` 后缀）
- 创建文件后在编辑器中打开，并展开父目录

---

## 6. 文件操作 API

### 6.1 目录扫描

采用**按需展开**策略（非全量扫描）：
- `open_folder` 只扫描根目录的直接子项
- 展开目录时扫描该目录的直接子项
- 避免大项目的性能问题

```rust
fn scan_direct_children(&mut self, parent_path: &Path, parent_idx: usize) {
    let entries = std::fs::read_dir(parent_path)?;
    // 按目录优先 + 字母排序
    // 插入到 nodes 中 parent_idx 的 children 区间
    // 更新 sibling 和 first_child 指针
}
```

### 6.2 文件监听（可选后续版本）

- 不实现主动文件监控
- 提供手动刷新按钮（🔄）
- 保存文件后自动刷新文件树中该条目的状态

---

## 7. 拖拽支持

```rust
// 在主 ui 方法中
ctx.input(|i| {
    for file in &i.raw.dropped_files {
        if let Some(path) = &file.path {
            if path.is_dir() {
                self.file_tree.open_folder(path);
            }
        }
    }
});
```

---

## 8. 国际化键

新增 Fluent 键：

```
menu-file-open-folder = 打开文件夹...
file-tree-empty-hint = 打开文件夹以浏览 Markdown 文件
file-tree-empty-hint-detail = 拖拽文件夹到窗口 或 文件 → 打开文件夹
file-tree-open = 打开
file-tree-rename = 重命名
file-tree-delete = 删除
file-tree-reveal = 在文件管理器中显示
file-tree-new-file = 新建 Markdown 文件
file-tree-expand-all = 展开全部
file-tree-collapse-all = 折叠全部
file-tree-unsupported = 不支持的文件类型
file-tree-new-file-name = 新文件
file-tree-refresh = 刷新
file-tree-confirm-delete = 确认删除 "{name}"？
```

---

## 9. 错误处理

| 场景 | 处理方式 |
|------|----------|
| 目录不可读（权限） | 显示警告 status_message，跳过该目录 |
| 文件删除失败 | 显示错误 status_message |
| 重命名冲突（同名文件已存在） | 显示错误，不执行重命名 |
| 已打开文件被外部删除 | 不影响编辑器内容（内存中仍保留），文件树中移除该条目 |
| 大目录（>1000 项） | 按需展开避免性能问题，扫描时限制单层条目数 |

所有文件操作不 panic，错误通过 `EditorState::status_message` 反馈。

---

## 10. 测试计划

### 10.1 单元测试（`file_tree.rs`）

- `open_folder` — 正常目录、空目录、不存在路径
- `toggle_expand` — 展开/折叠切换
- `scan_direct_children` — 排序（目录优先）、隐藏文件过滤
- `find_by_path` — 存在/不存在的路径

### 10.2 集成测试

- 菜单触发打开文件夹
- 拖拽文件夹到窗口
- 右键菜单各项操作
- 打开文件后标签页切换

---

## 11. 不在此版本范围

- 文件系统监控（自动检测外部变更）
- 自定义文件图标/颜色
- 拖拽文件/文件夹到文件树中移动
- Git 状态标记
- 文件排序方式自定义（按名称/类型/日期）
