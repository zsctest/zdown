# 多语言/国际化 (i18n) 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 新建 `crates/i18n` crate，使用 fluent-rs 实现运行时热切换的中英双语支持，覆盖所有 UI 文本。

**架构：** 新增 `i18n` crate 封装 fluent-bundle，FTL 文件通过 `include_str!` 编译时嵌入。`I18n` 实例由 `ZdownApp` 持有，通过 `&I18n` 引用传递给所有 UI 函数。`config` crate 用 `String` 存储语言，不依赖 `i18n`。

**技术栈：** fluent 0.17 / fluent-bundle 0.16 / unic-langid 0.9 / intl-memoizer 0.5

---

## 文件结构

| 操作 | 文件路径 | 职责 |
|---|---|---|
| 创建 | `crates/i18n/Cargo.toml` | 依赖声明 |
| 创建 | `crates/i18n/src/lib.rs` | Lang 枚举、I18n 结构体、公共 API |
| 创建 | `crates/i18n/src/resource.rs` | include_str! 嵌入 FTL，创建 FluentBundle |
| 创建 | `crates/i18n/locales/zh-CN/menu.ftl` | 菜单栏中文 |
| 创建 | `crates/i18n/locales/zh-CN/settings.ftl` | 设置面板中文 |
| 创建 | `crates/i18n/locales/zh-CN/editor.ftl` | 编辑器 UI 中文 |
| 创建 | `crates/i18n/locales/zh-CN/actions.ftl` | 操作名称中文 |
| 创建 | `crates/i18n/locales/en-US/menu.ftl` | 菜单栏英文 |
| 创建 | `crates/i18n/locales/en-US/settings.ftl` | 设置面板英文 |
| 创建 | `crates/i18n/locales/en-US/editor.ftl` | 编辑器 UI 英文 |
| 创建 | `crates/i18n/locales/en-US/actions.ftl` | 操作名称英文 |
| 修改 | `Cargo.toml` (root) | 添加 workspace deps + i18n member |
| 修改 | `crates/config/src/lib.rs` | AppConfig 加 `lang: String` |
| 修改 | `crates/config/src/keybinding.rs` | Action::display_name() 返回 FTL key |
| 修改 | `crates/workspace/src/dialog.rs` | pick_* 函数接受 title 参数 |
| 修改 | `crates/zdown-app/Cargo.toml` | 加 i18n 依赖 |
| 修改 | `crates/zdown-app/src/main.rs` | ZdownApp 加 I18n 字段，传入所有 UI 函数，内联文本翻译 |
| 修改 | `crates/zdown-app/src/menu.rs` | 所有硬编码文本替换为 i18n 调用 |
| 修改 | `crates/zdown-app/src/settings_dialog.rs` | 所有硬编码文本替换 + 语言选择器 |
| 修改 | `crates/zdown-app/src/tab_bar.rs` | 右键菜单文本替换 |
| 修改 | `crates/zdown-app/src/outline_view.rs` | 面板文本替换 |
| 修改 | `crates/zdown-app/src/view_mode.rs` | label() 返回 FTL key |

---

### 任务 1：创建工作区依赖和 i18n crate 骨架

**文件：**
- 修改：`Cargo.toml`（根）
- 创建：`crates/i18n/Cargo.toml`
- 创建：`crates/i18n/src/lib.rs`（stub）

- [ ] **步骤 1：添加 workspace 依赖声明**

编辑 `Cargo.toml`，在 `[workspace.dependencies]` 中添加 fluent 系列：

```toml
# ---------- i18n ----------
fluent = "0.17"
fluent-bundle = "0.16"
unic-langid = "0.9"
intl-memoizer = "0.5"
```

在 `members` 数组中增加 `"crates/i18n"`。

- [ ] **步骤 2：创建 i18n/Cargo.toml**

```toml
[package]
name = "i18n"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
fluent.workspace = true
fluent-bundle.workspace = true
unic-langid.workspace = true
intl-memoizer.workspace = true
serde.workspace = true
```

- [ ] **步骤 3：创建 i18n/src/lib.rs stub**

```rust
//! zdown 多语言国际化模块。
//!
//! 基于 Fluent 实现运行时热切换的中英双语支持。

pub mod resource;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use intl_memoizer::concurrent::IntlLangMemoizer;
use std::collections::HashMap;

/// 支持的语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Lang {
    /// 中文简体
    ZhCN,
    /// English (United States)
    EnUS,
}

impl Lang {
    /// 返回语言标签字符串（用于持久化到 config.toml）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ZhCN => "zh-CN",
            Self::EnUS => "en-US",
        }
    }

    /// 从字符串解析语言（不匹配时回退到中文）。
    pub fn from_str(s: &str) -> Self {
        match s {
            "en-US" => Self::EnUS,
            _ => Self::ZhCN,
        }
    }

    /// 返回用户可读的显示名。
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::ZhCN => "中文",
            Self::EnUS => "English",
        }
    }
}

/// 国际化管理器。
pub struct I18n {
    lang: Lang,
    bundles: HashMap<Lang, FluentBundle<FluentResource, IntlLangMemoizer>>,
}

impl I18n {
    /// 创建实例，预加载所有语言的 FTL 资源。
    pub fn new() -> Self {
        let mut bundles = HashMap::new();
        bundles.insert(Lang::ZhCN, resource::create_bundle_zh_cn());
        bundles.insert(Lang::EnUS, resource::create_bundle_en_us());
        Self {
            lang: Lang::ZhCN,
            bundles,
        }
    }

    /// 以指定语言创建实例。
    pub fn with_lang(lang: Lang) -> Self {
        let mut slf = Self::new();
        slf.lang = lang;
        slf
    }

    /// 获取当前语言。
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// 切换语言（热切换）。
    pub fn set_lang(&mut self, lang: Lang) {
        self.lang = lang;
    }

    /// 翻译指定 key，可选参数插值。
    pub fn tr(&self, key: &str, args: Option<&FluentArgs>) -> String {
        let bundle = match self.bundles.get(&self.lang) {
            Some(b) => b,
            None => return key.to_string(),
        };
        let msg = match bundle.get_message(key) {
            Some(m) => m,
            None => return key.to_string(),
        };
        let pattern = match msg.value() {
            Some(p) => p,
            None => return key.to_string(),
        };
        let mut errors = vec![];
        let value = bundle.format_pattern(pattern, args, &mut errors);
        value.to_string()
    }

    /// 无参数翻译便捷方法。
    pub fn t(&self, key: &str) -> String {
        self.tr(key, None)
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **步骤 4：编译检查**

```bash
cargo check -p i18n
```

预期：`resource` 模块未找到错误（将在任务 2 中修复）——这是可接受的。

---

### 任务 2：实现 FTL 资源加载

**文件：**
- 创建：`crates/i18n/src/resource.rs`

- [ ] **步骤 1：编写 resource.rs（使用占位 include_str! 路径，任务 3/4 创建文件前会有编译错误）**

```rust
//! FTL 资源加载：通过 include_str! 在编译时嵌入，构建 FluentBundle。

use fluent_bundle::{FluentBundle, FluentResource};
use intl_memoizer::concurrent::IntlLangMemoizer;
use unic_langid::langid;

/// 为中文创建 FluentBundle。
pub(crate) fn create_bundle_zh_cn() -> FluentBundle<FluentResource, IntlLangMemoizer> {
    let langid = langid!("zh-CN");
    let mut bundle = FluentBundle::new_concurrent(vec![langid]);

    add_resource(
        &mut bundle,
        include_str!("../locales/zh-CN/menu.ftl"),
    );
    add_resource(
        &mut bundle,
        include_str!("../locales/zh-CN/settings.ftl"),
    );
    add_resource(
        &mut bundle,
        include_str!("../locales/zh-CN/editor.ftl"),
    );
    add_resource(
        &mut bundle,
        include_str!("../locales/zh-CN/actions.ftl"),
    );

    bundle
}

/// 为英文创建 FluentBundle。
pub(crate) fn create_bundle_en_us() -> FluentBundle<FluentResource, IntlLangMemoizer> {
    let langid = langid!("en-US");
    let mut bundle = FluentBundle::new_concurrent(vec![langid]);

    add_resource(
        &mut bundle,
        include_str!("../locales/en-US/menu.ftl"),
    );
    add_resource(
        &mut bundle,
        include_str!("../locales/en-US/settings.ftl"),
    );
    add_resource(
        &mut bundle,
        include_str!("../locales/en-US/editor.ftl"),
    );
    add_resource(
        &mut bundle,
        include_str!("../locales/en-US/actions.ftl"),
    );

    bundle
}

fn add_resource(
    bundle: &mut FluentBundle<FluentResource, IntlLangMemoizer>,
    source: &str,
) {
    let res = FluentResource::try_new(source.to_string())
        .expect("FTL 解析失败：静态嵌入的 FTL 文件存在语法错误");
    bundle
        .add_resource(res)
        .expect("FTL 资源添加失败：可能存在重复的 message ID");
}
```

- [ ] **步骤 2：创建空的 FTL 占位文件（编译需要）**

每个语言目录下创建空的 FTL 文件以通过编译（内容在任务 3/4 中填充）。

```bash
mkdir -p crates/i18n/locales/zh-CN
mkdir -p crates/i18n/locales/en-US
# 每个目录创建 4 个空文件
```

创建空文件：
- `crates/i18n/locales/zh-CN/menu.ftl`（空）
- `crates/i18n/locales/zh-CN/settings.ftl`（空）
- `crates/i18n/locales/zh-CN/editor.ftl`（空）
- `crates/i18n/locales/zh-CN/actions.ftl`（空）
- `crates/i18n/locales/en-US/menu.ftl`（空）
- `crates/i18n/locales/en-US/settings.ftl`（空）
- `crates/i18n/locales/en-US/editor.ftl`（空）
- `crates/i18n/locales/en-US/actions.ftl`（空）

- [ ] **步骤 3：编译验证**

```bash
cargo check -p i18n
```

预期：编译通过（空 FTL 文件是合法的 FluentResource）。

---

### 任务 3：编写中文 FTL 翻译文件

**文件：**
- 修改：`crates/i18n/locales/zh-CN/menu.ftl`
- 修改：`crates/i18n/locales/zh-CN/settings.ftl`
- 修改：`crates/i18n/locales/zh-CN/editor.ftl`
- 修改：`crates/i18n/locales/zh-CN/actions.ftl`

- [ ] **步骤 1：编写 zh-CN/menu.ftl**

```ftl
# 菜单栏 - 中文

# 顶级菜单
menu-file = 文件
menu-edit = 编辑
menu-view = 视图

# 文件菜单
menu-file-new = 新建 (Ctrl+N)
menu-file-open = 打开... (Ctrl+O)
menu-file-save = 保存 (Ctrl+S)
menu-file-save-as = 另存为... (Ctrl+Shift+S)
menu-file-save-all = 保存所有
menu-file-export-pdf = 导出 PDF...
menu-file-export-html = 导出 HTML...
menu-file-recent = 最近文件
menu-file-recent-empty = (无)
menu-file-settings = 设置...
menu-file-quit = 退出

# 编辑菜单
menu-edit-undo = 撤销 (Ctrl+Z)
menu-edit-redo = 重做 (Ctrl+Y)
menu-edit-insert-image = 插入图片... (Ctrl+I)

# 视图菜单
menu-view-source = 源码 (Ctrl+1)
menu-view-preview = 预览 (Ctrl+2)
menu-view-hybrid = Hybrid (Ctrl+3)
menu-theme-light = ☀️ 亮色主题
menu-theme-dark = 🌙 暗色主题

# 未保存确认对话框
confirm-unsaved-title-quit = 未保存修改 - 退出
confirm-unsaved-title-close = 未保存修改 - 关闭标签页
confirm-unsaved-body = 当前文档有未保存修改。是否保存?
confirm-btn-save = 保存
confirm-btn-discard = 不保存
confirm-btn-cancel = 取消

# 状态消息
status-save-result = 保存完成：{$saved} 个文件
status-save-skipped = 保存完成：{$saved} 个文件，{$skipped} 个未命名文件已跳过
status-pdf-failed = PDF 导出失败: {$error}
status-html-failed = HTML 导出失败: {$error}
status-image-read-failed = 图片读取失败: {$error}
status-image-insert-failed = 图片插入失败
status-image-store-failed = 图片存储失败: {$error}
status-replaced-count = 已替换 {$count} 处
```

- [ ] **步骤 2：编写 zh-CN/settings.ftl**

```ftl
# 设置对话框 - 中文

# 窗口标题
settings-title = 设置

# 标签页
settings-tab-css = 样式
settings-tab-image = 图片
settings-tab-spell = 拼写
settings-tab-keybind = 快捷键

# 样式标签页
settings-css-label = 自定义 CSS（追加到内置样式之后，留空表示不使用）：
settings-css-hint = /* 在此输入自定义 CSS，例如 */\nh1 { color: #2196F3; }\nbody { max-width: 900px; }

# 图片标签页
settings-image-strategy-label = 默认存储策略：
settings-image-local = 本地
settings-image-base64 = Base64
settings-image-smms = SM.MS
settings-image-local-dir-label = 本地图片目录：
settings-image-smms-token-label = SM.MS API Token：
settings-image-get-token = 获取 Token
settings-image-token-hint = 无 Token 也可上传，但有数量限制。注册后在网站获取。

# 拼写标签页
settings-spell-label = 英文拼写检查：
settings-spell-enable = 启用英文拼写检查
settings-spell-hint-save = 拼写检查在保存文件时自动执行。
settings-spell-hint-underline = 错误单词将以红色波浪下划线标记。
settings-spell-dict = 词典：English (United States) — en_US

# 快捷键标签页
settings-keybind-hint = 点击快捷键单元格后按下新组合键，Esc 取消
settings-keybind-reset-all = 恢复全部默认
settings-keybind-header-action = 操作
settings-keybind-header-shortcut = 快捷键
settings-keybind-capturing = ⌨ 按下新快捷键...
settings-keybind-reset = ↶
settings-keybind-restore-tooltip = 恢复默认
settings-keybind-conflict = ⚠

# 语言选择
settings-language-label = 界面语言：

# 通用按钮
settings-btn-save = 保存
settings-btn-cancel = 取消
```

- [ ] **步骤 3：编写 zh-CN/editor.ftl**

```ftl
# 编辑器 UI - 中文

# 搜索栏
search-find = 查找:
search-replace = 替换:
search-replace-btn = 替换
search-replace-all = 全部

# 标签页右键菜单
tab-close-others = 关闭其他
tab-close-right = 关闭右侧
tab-copy-path = 复制路径

# 大纲面板
outline-heading = 📑 大纲 ({$count})
outline-empty = （无标题）
outline-empty-heading = （空标题）
outline-image-prefix = 图片:
```

- [ ] **步骤 4：编写 zh-CN/actions.ftl**

```ftl
# 操作名称 - 中文

# Action::display_name()
action-save = 保存
action-save-as = 另存为
action-new-file = 新建文件
action-open = 打开文件
action-close-tab = 关闭标签
action-next-tab = 下一个标签
action-prev-tab = 上一个标签
action-move-tab-left = 左移标签
action-move-tab-right = 右移标签
action-undo = 撤销
action-redo = 重做
action-view-source = 源码视图
action-view-preview = 预览视图
action-view-hybrid = 混合视图
action-toggle-theme = 切换主题

# ViewMode::label()
view-source = 源码
view-preview = 预览
view-hybrid = Hybrid
```

- [ ] **步骤 5：编译验证**

```bash
cargo check -p i18n
```

预期：编译通过。FTL 文件在编译时被嵌入和解析。

---

### 任务 4：编写英文 FTL 翻译文件

**文件：**
- 修改：`crates/i18n/locales/en-US/menu.ftl`
- 修改：`crates/i18n/locales/en-US/settings.ftl`
- 修改：`crates/i18n/locales/en-US/editor.ftl`
- 修改：`crates/i18n/locales/en-US/actions.ftl`

- [ ] **步骤 1：编写 en-US/menu.ftl**

```ftl
# Menu bar - English

# Top-level menus
menu-file = File
menu-edit = Edit
menu-view = View

# File menu
menu-file-new = New (Ctrl+N)
menu-file-open = Open... (Ctrl+O)
menu-file-save = Save (Ctrl+S)
menu-file-save-as = Save As... (Ctrl+Shift+S)
menu-file-save-all = Save All
menu-file-export-pdf = Export PDF...
menu-file-export-html = Export HTML...
menu-file-recent = Recent Files
menu-file-recent-empty = (None)
menu-file-settings = Settings...
menu-file-quit = Quit

# Edit menu
menu-edit-undo = Undo (Ctrl+Z)
menu-edit-redo = Redo (Ctrl+Y)
menu-edit-insert-image = Insert Image... (Ctrl+I)

# View menu
menu-view-source = Source (Ctrl+1)
menu-view-preview = Preview (Ctrl+2)
menu-view-hybrid = Hybrid (Ctrl+3)
menu-theme-light = ☀️ Light Theme
menu-theme-dark = 🌙 Dark Theme

# Unsaved changes dialog
confirm-unsaved-title-quit = Unsaved Changes - Quit
confirm-unsaved-title-close = Unsaved Changes - Close Tab
confirm-unsaved-body = The document has unsaved changes. Save?
confirm-btn-save = Save
confirm-btn-discard = Discard
confirm-btn-cancel = Cancel

# Status messages
status-save-result = Saved: {$saved} file(s)
status-save-skipped = Saved: {$saved} file(s), {$skipped} unnamed file(s) skipped
status-pdf-failed = PDF export failed: {$error}
status-html-failed = HTML export failed: {$error}
status-image-read-failed = Image read failed: {$error}
status-image-insert-failed = Image insert failed
status-image-store-failed = Image store failed: {$error}
status-replaced-count = Replaced {$count} occurrence(s)
```

- [ ] **步骤 2：编写 en-US/settings.ftl**

```ftl
# Settings dialog - English

# Window title
settings-title = Settings

# Tabs
settings-tab-css = Style
settings-tab-image = Image
settings-tab-spell = Spell
settings-tab-keybind = Keybind

# Style tab
settings-css-label = Custom CSS (appended after built-in styles, leave empty to disable):
settings-css-hint = /* Enter custom CSS here, e.g. */\nh1 { color: #2196F3; }\nbody { max-width: 900px; }

# Image tab
settings-image-strategy-label = Default storage strategy:
settings-image-local = Local
settings-image-base64 = Base64
settings-image-smms = SM.MS
settings-image-local-dir-label = Local image directory:
settings-image-smms-token-label = SM.MS API Token:
settings-image-get-token = Get Token
settings-image-token-hint = Upload works without a token, but has rate limits. Register on the website to get one.

# Spell tab
settings-spell-label = English spell check:
settings-spell-enable = Enable English spell check
settings-spell-hint-save = Spell check runs automatically when saving files.
settings-spell-hint-underline = Misspelled words are marked with red wavy underlines.
settings-spell-dict = Dictionary: English (United States) — en_US

# Keybind tab
settings-keybind-hint = Click a shortcut cell then press a new key combo, Esc to cancel
settings-keybind-reset-all = Reset All Defaults
settings-keybind-header-action = Action
settings-keybind-header-shortcut = Shortcut
settings-keybind-capturing = ⌨ Press new shortcut...
settings-keybind-reset = ↶
settings-keybind-restore-tooltip = Restore default
settings-keybind-conflict = ⚠

# Language selector
settings-language-label = Interface language:

# Common buttons
settings-btn-save = Save
settings-btn-cancel = Cancel
```

- [ ] **步骤 3：编写 en-US/editor.ftl**

```ftl
# Editor UI - English

# Search bar
search-find = Find:
search-replace = Replace:
search-replace-btn = Replace
search-replace-all = All

# Tab context menu
tab-close-others = Close Others
tab-close-right = Close to Right
tab-copy-path = Copy Path

# Outline panel
outline-heading = 📑 Outline ({$count})
outline-empty = (No headings)
outline-empty-heading = (Empty heading)
outline-image-prefix = Image:
```

- [ ] **步骤 4：编写 en-US/actions.ftl**

```ftl
# Action names - English

# Action::display_name()
action-save = Save
action-save-as = Save As
action-new-file = New File
action-open = Open File
action-close-tab = Close Tab
action-next-tab = Next Tab
action-prev-tab = Previous Tab
action-move-tab-left = Move Tab Left
action-move-tab-right = Move Tab Right
action-undo = Undo
action-redo = Redo
action-view-source = Source View
action-view-preview = Preview View
action-view-hybrid = Hybrid View
action-toggle-theme = Toggle Theme

# ViewMode::label()
view-source = Source
view-preview = Preview
view-hybrid = Hybrid
```

- [ ] **步骤 5：编译验证**

```bash
cargo check -p i18n
```

预期：编译通过。

---

### 任务 5：编写 i18n 单元测试

**文件：**
- 修改：`crates/i18n/src/lib.rs`（在文件末尾添加 `#[cfg(test)]` 模块）

- [ ] **步骤 1：编写测试**

在 `crates/i18n/src/lib.rs` 末尾添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fluent_bundle::FluentArgs;

    #[test]
    fn new_creates_bundles() {
        let i18n = I18n::new();
        // 两种语言的 bundle 都存在
        assert!(i18n.bundles.contains_key(&Lang::ZhCN));
        assert!(i18n.bundles.contains_key(&Lang::EnUS));
        // 默认语言为中文
        assert_eq!(i18n.lang(), Lang::ZhCN);
    }

    #[test]
    fn with_lang_en_us() {
        let i18n = I18n::with_lang(Lang::EnUS);
        assert_eq!(i18n.lang(), Lang::EnUS);
    }

    #[test]
    fn t_zh_cn_menu() {
        let i18n = I18n::with_lang(Lang::ZhCN);
        assert_eq!(i18n.t("menu-file"), "文件");
        assert_eq!(i18n.t("menu-file-new"), "新建 (Ctrl+N)");
        assert_eq!(i18n.t("menu-edit"), "编辑");
    }

    #[test]
    fn t_en_us_menu() {
        let i18n = I18n::with_lang(Lang::EnUS);
        assert_eq!(i18n.t("menu-file"), "File");
        assert_eq!(i18n.t("menu-file-new"), "New (Ctrl+N)");
        assert_eq!(i18n.t("menu-edit"), "Edit");
    }

    #[test]
    fn t_actions_both_langs() {
        let mut i18n = I18n::new();

        i18n.set_lang(Lang::ZhCN);
        assert_eq!(i18n.t("action-save"), "保存");
        assert_eq!(i18n.t("action-undo"), "撤销");
        assert_eq!(i18n.t("view-source"), "源码");

        i18n.set_lang(Lang::EnUS);
        assert_eq!(i18n.t("action-save"), "Save");
        assert_eq!(i18n.t("action-undo"), "Undo");
        assert_eq!(i18n.t("view-source"), "Source");
    }

    #[test]
    fn tr_with_args() {
        let i18n = I18n::with_lang(Lang::ZhCN);
        let mut args = FluentArgs::new();
        args.set("count", 5);
        assert_eq!(i18n.tr("outline-heading", Some(&args)), "📑 大纲 (5)");
    }

    #[test]
    fn tr_with_multiple_args() {
        let i18n = I18n::with_lang(Lang::EnUS);
        let mut args = FluentArgs::new();
        args.set("saved", 3);
        args.set("skipped", 1);
        let result = i18n.tr("status-save-skipped", Some(&args));
        assert!(result.contains("3"));
        assert!(result.contains("1"));
    }

    #[test]
    fn missing_key_returns_key_name() {
        let i18n = I18n::new();
        assert_eq!(i18n.t("nonexistent-key"), "nonexistent-key");
    }

    #[test]
    fn lang_set_switches_translations() {
        let mut i18n = I18n::with_lang(Lang::ZhCN);
        assert_eq!(i18n.t("menu-file"), "文件");

        i18n.set_lang(Lang::EnUS);
        assert_eq!(i18n.t("menu-file"), "File");

        i18n.set_lang(Lang::ZhCN);
        assert_eq!(i18n.t("menu-file"), "文件");
    }

    #[test]
    fn lang_as_str_roundtrip() {
        assert_eq!(Lang::from_str("zh-CN"), Lang::ZhCN);
        assert_eq!(Lang::from_str("en-US"), Lang::EnUS);
        assert_eq!(Lang::from_str("unknown"), Lang::ZhCN); // fallback
    }

    #[test]
    fn lang_display_name() {
        assert_eq!(Lang::ZhCN.display_name(), "中文");
        assert_eq!(Lang::EnUS.display_name(), "English");
    }

    #[test]
    fn settings_tab_translations() {
        let i18n = I18n::with_lang(Lang::ZhCN);
        assert_eq!(i18n.t("settings-tab-css"), "样式");
        assert_eq!(i18n.t("settings-tab-image"), "图片");
        assert_eq!(i18n.t("settings-tab-spell"), "拼写");
        assert_eq!(i18n.t("settings-tab-keybind"), "快捷键");

        let i18n = I18n::with_lang(Lang::EnUS);
        assert_eq!(i18n.t("settings-tab-css"), "Style");
        assert_eq!(i18n.t("settings-tab-image"), "Image");
    }

    #[test]
    fn confirm_dialog_translations() {
        let i18n = I18n::with_lang(Lang::ZhCN);
        assert_eq!(i18n.t("confirm-unsaved-body"), "当前文档有未保存修改。是否保存?");
        assert_eq!(i18n.t("confirm-btn-save"), "保存");

        let i18n = I18n::with_lang(Lang::EnUS);
        assert_eq!(i18n.t("confirm-unsaved-body"), "The document has unsaved changes. Save?");
        assert_eq!(i18n.t("confirm-btn-save"), "Save");
    }

    #[test]
    fn editor_ui_translations() {
        let i18n = I18n::with_lang(Lang::ZhCN);
        assert_eq!(i18n.t("search-find"), "查找:");
        assert_eq!(i18n.t("tab-close-others"), "关闭其他");
        assert_eq!(i18n.t("outline-empty"), "（无标题）");

        let i18n = I18n::with_lang(Lang::EnUS);
        assert_eq!(i18n.t("search-find"), "Find:");
        assert_eq!(i18n.t("tab-close-others"), "Close Others");
        assert_eq!(i18n.t("outline-empty"), "(No headings)");
    }
}
```

- [ ] **步骤 2：运行测试**

```bash
cargo test -p i18n
```

预期：全部 13 个测试通过。

- [ ] **步骤 3：Commit**

```bash
git add crates/i18n/ Cargo.toml
git commit -m "feat(i18n): add i18n crate with fluent-rs integration

- Lang enum (ZhCN/EnUS) with serde support
- I18n struct with runtime hot-switch
- FTL resources embedded via include_str!
- zh-CN and en-US translations (menu/settings/editor/actions)
- 13 unit tests covering translations, args, switching, fallbacks

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 6：修改 config crate

**文件：**
- 修改：`crates/config/src/lib.rs`
- 修改：`crates/config/src/keybinding.rs`

- [ ] **步骤 1：AppConfig 添加 lang 字段**

在 `crates/config/src/lib.rs` 中，找到 `pub struct AppConfig`，添加 `lang` 字段：

```rust
/// 界面语言，值为 "zh-CN" 或 "en-US"。默认 "zh-CN"。
#[serde(default = "default_lang")]
pub lang: String,
```

在文件末尾（`Default` impl 之前）添加辅助函数：

```rust
fn default_lang() -> String {
    "zh-CN".to_string()
}
```

更新 `Default` impl，添加 `lang` 字段：

```rust
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            custom_css: None,
            theme: ThemeMode::Dark,
            image_hosting: ImageHostingConfig::default(),
            spell_check_enabled: true,
            keymap: Keymap::default(),
            lang: default_lang(),  // 新增
        }
    }
}
```

- [ ] **步骤 2：修改 Action::display_name() 返回 FTL key**

在 `crates/config/src/keybinding.rs` 中，将 `display_name()` 的返回值从中文改为 FTL key：

```rust
/// 返回此动作的 FTL 翻译 key（由调用方通过 i18n 翻译）。
pub fn display_name(&self) -> &'static str {
    match self {
        Self::Save => "action-save",
        Self::SaveAs => "action-save-as",
        Self::NewFile => "action-new-file",
        Self::Open => "action-open",
        Self::CloseTab => "action-close-tab",
        Self::NextTab => "action-next-tab",
        Self::PrevTab => "action-prev-tab",
        Self::MoveTabLeft => "action-move-tab-left",
        Self::MoveTabRight => "action-move-tab-right",
        Self::Undo => "action-undo",
        Self::Redo => "action-redo",
        Self::ViewSource => "action-view-source",
        Self::ViewPreview => "action-view-preview",
        Self::ViewHybrid => "action-view-hybrid",
        Self::ToggleTheme => "action-toggle-theme",
    }
}
```

- [ ] **步骤 3：更新 config 测试**

在 `crates/config/src/lib.rs` 的 `#[cfg(test)]` 模块中添加：

```rust
#[test]
fn app_config_default_lang_is_zh_cn() {
    let config = AppConfig::default();
    assert_eq!(config.lang, "zh-CN");
}

#[test]
fn config_toml_contains_lang_field() {
    let config = AppConfig::default();
    let toml_str = toml::to_string_pretty(&config).expect("serialize");
    assert!(
        toml_str.contains("lang"),
        "TOML 应包含 lang 字段: {}",
        toml_str
    );
    assert!(
        toml_str.contains("zh-CN"),
        "TOML 应包含默认语言 zh-CN: {}",
        toml_str
    );
}
```

在 `crates/config/src/keybinding.rs` 的测试中更新 display_name 期望值（如果有的话），或添加：

```rust
#[test]
fn display_name_returns_ftl_keys() {
    assert_eq!(Action::Save.display_name(), "action-save");
    assert_eq!(Action::Undo.display_name(), "action-undo");
    // 确保所有 action 都有对应的 FTL key
    for action in Action::all() {
        let key = action.display_name();
        assert!(key.starts_with("action-"), "Key {key} should start with 'action-'");
    }
}
```

- [ ] **步骤 4：运行测试**

```bash
cargo test -p config
```

预期：全部测试通过。

- [ ] **步骤 5：Commit**

```bash
git add crates/config/
git commit -m "feat(config): add lang field to AppConfig, convert Action::display_name to FTL keys

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 7：修改 workspace crate（对话框接受标题参数）

**文件：**
- 修改：`crates/workspace/src/dialog.rs`

- [ ] **步骤 1：修改 pick_* 函数，接受 title 参数**

将每个函数的 `set_title()` 改为接受 `title: &str` 参数：

```rust
use std::path::PathBuf;

/// 弹出打开文件对话框。用户取消或环境不支持时返回 `None`。
pub fn pick_open_file(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Markdown", &["md", "markdown"])
        .set_title(title)
        .pick_file()
}

/// 弹出保存文件对话框。
pub fn pick_save_file(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Markdown", &["md", "markdown"])
        .set_title(title)
        .set_file_name("untitled.md")
        .save_file()
}

/// 弹出 PDF 导出保存对话框。
pub fn pick_save_file_pdf(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("PDF", &["pdf"])
        .set_title(title)
        .set_file_name("untitled.pdf")
        .save_file()
}

/// 弹出 HTML 导出保存对话框。
pub fn pick_save_file_html(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("HTML", &["html", "htm"])
        .set_title(title)
        .set_file_name("untitled.html")
        .save_file()
}

/// 弹出打开图片文件对话框。
pub fn pick_open_image(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Image", &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"])
        .set_title(title)
        .pick_file()
}
```

注意：`pick_open_image` 的 filter 名称从中文 `"图片"` 改为英文 `"Image"`（rfd filter 名称不是用户主要关心的翻译点）。

- [ ] **步骤 2：编译检查**

```bash
cargo check -p workspace
```

预期：workspace crate 编译通过，但 zdown-app 中的调用点会报错（缺少 title 参数）。先不修——任务 8 中处理。

- [ ] **步骤 3：Commit**

```bash
git add crates/workspace/
git commit -m "refactor(workspace): accept dialog title as parameter for i18n support

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 8：修改 view_mode.rs（label 返回 FTL key）

**文件：**
- 修改：`crates/zdown-app/src/view_mode.rs`

- [ ] **步骤 1：修改 ViewMode::label()**

```rust
impl ViewMode {
    /// 返回 FTL 翻译 key（由调用方通过 i18n 翻译）。
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "view-source",
            Self::Preview => "view-preview",
            Self::Hybrid => "view-hybrid",
        }
    }
}
```

- [ ] **步骤 2：编译验证**

```bash
cargo check -p zdown-app
```

预期：调用 `label()` 的地方编译错误（因为返回类型未变，但调用方需要改为 `i18n.t(label())`），将在任务 9-13 中一并修复。

---

### 任务 9：zdown-app 集成 — 核心（main.rs + Cargo.toml）

**文件：**
- 修改：`crates/zdown-app/Cargo.toml`
- 修改：`crates/zdown-app/src/main.rs`

- [ ] **步骤 1：添加 i18n 依赖**

在 `crates/zdown-app/Cargo.toml` 的 `[dependencies]` 中添加：

```toml
i18n = { path = "../i18n" }
```

- [ ] **步骤 2：修改 main.rs — ZdownApp 结构体和初始化**

在 `crates/zdown-app/src/main.rs` 中：

在 imports 区域添加：
```rust
use i18n::I18n;
```

在 `struct ZdownApp` 中添加：
```rust
/// 国际化管理器。
i18n: I18n,
```

在 `Default for ZdownApp` 中，`let app_config = ...` 后添加语言解析，并在 `Self {` 中添加 `i18n` 字段：

```rust
let lang = i18n::Lang::from_str(&app_config.lang);
// ...
Self {
    // ... existing fields ...
    i18n: I18n::with_lang(lang),
}
```

- [ ] **步骤 3：修改 main.rs — 搜索栏内联文本**

将搜索栏中的硬编码中文替换为 `self.i18n.t(...)`：

| 原文本 | 替换为 |
|---|---|
| `"查找:"` | `self.i18n.t("search-find")` |
| `"替换:"` | `self.i18n.t("search-replace")` |
| `"替换"` (按钮) | `self.i18n.t("search-replace-btn")` |
| `"全部"` (按钮) | `self.i18n.t("search-replace-all")` |
| `"已替换 {count} 处"` | `self.i18n.tr("status-replaced-count", ...)`（带 args） |

对于 `"已替换 {count} 处"`：
```rust
let mut args = fluent_bundle::FluentArgs::new();
args.set("count", count as i64);
self.state.status_message = self.i18n.tr("status-replaced-count", Some(&args));
```

对于搜索计数 `format!("{}/{}", idx + 1, self.search.matches.len())`：这个不需要翻译，是数字显示。

- [ ] **步骤 4：修改 main.rs — 函数调用传入 i18n**

将所有 UI 函数调用传入 `&self.i18n`：

```rust
// menu::show_menu 调用 — 加 &self.i18n
menu::show_menu(
    ui,
    &mut self.state,
    &mut self.confirm,
    &mut self.view_mode,
    &mut self.settings_dialog,
    &self.app_config,
    &mut self.theme,
    &self.app_config.image_hosting,
    &self.i18n,  // 新增
);

// menu::handle_shortcuts — 不需要 i18n（无 UI 文本）

// menu::show_confirm_dialog — 加 &self.i18n
menu::show_confirm_dialog(&ctx, &mut self.state, &mut self.confirm, &self.i18n);

// settings_dialog::show_settings_dialog — 加 &mut self.i18n
settings_dialog::show_settings_dialog(
    &ctx,
    &mut self.app_config,
    &mut self.settings_dialog,
    &mut self.i18n,  // 新增（mut 因为需要 set_lang）
);

// tab_bar::show_tab_bar — 加 &self.i18n
tab_bar::show_tab_bar(ui, &mut self.state, &mut self.confirm, &self.i18n);

// outline_view::show_outline_panel — 加 &self.i18n
outline_view::show_outline_panel(ui, &mut self.state, &mut self.fold_state, &self.i18n);
```

- [ ] **步骤 5：main.rs — trigger_browse_image 翻译**

在 `trigger_browse_image` 中（在 menu.rs 中），状态消息需要翻译。但由于此函数在 menu.rs 中，将在任务 10 中处理。

在 main.rs 中 `// Ctrl+I 浏览插入图片` 部分调用了 `menu::trigger_browse_image`，传入 `&self.i18n`。

---

### 任务 10：zdown-app 集成 — menu.rs

**文件：**
- 修改：`crates/zdown-app/src/menu.rs`

- [ ] **步骤 1：添加 i18n import 和修改函数签名**

在文件顶部添加：
```rust
use i18n::I18n;
```

修改 `show_menu()` 签名，在末尾增加 `i18n: &I18n` 参数。

修改 `show_confirm_dialog()` 签名，增加 `i18n: &I18n` 参数。

修改 `trigger_export_pdf()`，增加 `i18n: &I18n` 参数。

修改 `trigger_export_html()`，增加 `i18n: &I18n` 参数。

修改 `trigger_browse_image()`，增加 `i18n: &I18n` 参数。

修改 `execute_action()`，增加 `i18n: &I18n` 参数。

- [ ] **步骤 2：替换菜单栏全部硬编码文本**

逐个替换 `show_menu()` 中的文本：

```rust
ui.menu_button(i18n.t("menu-file"), |ui| {         // was: "文件"
    if ui.button(i18n.t("menu-file-new")).clicked() { // was: "新建 (Ctrl+N)"
        state.new_file();
    }
    if ui.button(i18n.t("menu-file-open")).clicked() { // was: "打开... (Ctrl+O)"
        trigger_open(state);
    }
    // ... 依次替换所有菜单项
```

完整替换清单：
- `"文件"` → `i18n.t("menu-file")`
- `"新建 (Ctrl+N)"` → `i18n.t("menu-file-new")`
- `"打开... (Ctrl+O)"` → `i18n.t("menu-file-open")`
- `"保存 (Ctrl+S)"` → `i18n.t("menu-file-save")`
- `"另存为... (Ctrl+Shift+S)"` → `i18n.t("menu-file-save-as")`
- `"保存所有"` → `i18n.t("menu-file-save-all")`
- `"导出 PDF..."` → `i18n.t("menu-file-export-pdf")`
- `"导出 HTML..."` → `i18n.t("menu-file-export-html")`
- `"最近文件"` → `i18n.t("menu-file-recent")`
- `"(无)"` → `i18n.t("menu-file-recent-empty")`
- `"设置..."` → `i18n.t("menu-file-settings")`
- `"退出"` → `i18n.t("menu-file-quit")`
- `"编辑"` → `i18n.t("menu-edit")`
- `"撤销 (Ctrl+Z)"` → `i18n.t("menu-edit-undo")`
- `"重做 (Ctrl+Y)"` → `i18n.t("menu-edit-redo")`
- `"插入图片... (Ctrl+I)"` → `i18n.t("menu-edit-insert-image")`
- `"视图"` → `i18n.t("menu-view")`
- `"源码 (Ctrl+1)"` → `i18n.t("menu-view-source")`
- `"预览 (Ctrl+2)"` → `i18n.t("menu-view-preview")`
- `"Hybrid (Ctrl+3)"` → `i18n.t("menu-view-hybrid")`

主题切换标签：
```rust
let toggle_label = match theme {
    ThemeMode::Dark => i18n.t("menu-theme-light"),
    ThemeMode::Light => i18n.t("menu-theme-dark"),
};
```

- [ ] **步骤 3：替换状态消息**

```rust
// trigger_save_all 函数中的状态消息
// 原来：format!("保存完成：{saved} 个文件")
let mut args = FluentArgs::new();
args.set("saved", saved as i64);
state.status_message = i18n.tr("status-save-result", Some(&args));

// 原来：msg.push_str(&format!("，{skipped} 个未命名文件已跳过"));
if skipped > 0 {
    args.set("saved", saved as i64);
    args.set("skipped", skipped as i64);
    state.status_message = i18n.tr("status-save-skipped", Some(&args));
}
```

- [ ] **步骤 4：替换确认对话框文本**

```rust
let title = match &pending {
    PendingAction::Quit => i18n.t("confirm-unsaved-title-quit"),
    PendingAction::CloseTab(_) => i18n.t("confirm-unsaved-title-close"),
};
// ...
ui.label(i18n.t("confirm-unsaved-body"));
// ...
if ui.button(i18n.t("confirm-btn-save")).clicked() { ... }
if ui.button(i18n.t("confirm-btn-discard")).clicked() { ... }
if ui.button(i18n.t("confirm-btn-cancel")).clicked() { ... }
```

- [ ] **步骤 5：替换导出错误消息**

```rust
// trigger_export_pdf 中
state.status_message = i18n.tr("status-pdf-failed", ...);
// trigger_export_html 中
state.status_message = i18n.tr("status-html-failed", ...);
```

- [ ] **步骤 6：替换图片相关消息（trigger_browse_image）**

```rust
state.status_message = i18n.tr("status-image-read-failed", ...);
state.status_message = i18n.t("status-image-insert-failed");
state.status_message = i18n.tr("status-image-store-failed", ...);
```

- [ ] **步骤 7：更新文件对话框调用**

所有 `workspace::pick_*` 调用传入翻译后的标题：

```rust
fn trigger_open(state: &mut EditorState, i18n: &I18n) {
    let title = i18n.t("menu-file-open");  // "打开..." / "Open..."
    if let Some(path) = workspace::pick_open_file(&title) {
        let _ = state.open(&path);
    }
}
```

同样处理 `trigger_save_as`、`trigger_export_pdf`、`trigger_export_html`、`trigger_browse_image`。

---

### 任务 11：zdown-app 集成 — settings_dialog.rs + 语言选择器

**文件：**
- 修改：`crates/zdown-app/src/settings_dialog.rs`

- [ ] **步骤 1：添加 import 和修改函数签名**

```rust
use i18n::{I18n, Lang};
```

修改 `show_settings_dialog()` 签名，增加 `i18n: &mut I18n` 参数（需要 `&mut` 因为语言切换需要 `set_lang()`）。

- [ ] **步骤 2：替换窗口标题和标签页文本**

```rust
egui::Window::new(i18n.t("settings-title"))
    // ...
    ui.selectable_value(&mut dialog.active_tab, SettingsTab::Css, i18n.t("settings-tab-css"));
    ui.selectable_value(&mut dialog.active_tab, SettingsTab::Image, i18n.t("settings-tab-image"));
    ui.selectable_value(&mut dialog.active_tab, SettingsTab::Spell, i18n.t("settings-tab-spell"));
    ui.selectable_value(&mut dialog.active_tab, SettingsTab::Keybind, i18n.t("settings-tab-keybind"));
```

- [ ] **步骤 3：替换 CSS 标签页文本**

```rust
ui.label(i18n.t("settings-css-label"));
// ...
hint_text: i18n.t("settings-css-hint"),
```

- [ ] **步骤 4：替换图片标签页文本**

```rust
ui.label(i18n.t("settings-image-strategy-label"));
// ...
ui.selectable_value(&mut dialog.strategy_buffer, 0, i18n.t("settings-image-local"));
ui.selectable_value(&mut dialog.strategy_buffer, 1, i18n.t("settings-image-base64"));
ui.selectable_value(&mut dialog.strategy_buffer, 2, i18n.t("settings-image-smms"));
// ...
ui.label(i18n.t("settings-image-local-dir-label"));
// ...
ui.label(i18n.t("settings-image-smms-token-label"));
// ...
if ui.button(i18n.t("settings-image-get-token")).clicked() { ... }
// ...
ui.label(egui::RichText::new(i18n.t("settings-image-token-hint")).weak().size(12.0));
```

- [ ] **步骤 5：替换拼写标签页文本**

```rust
ui.label(i18n.t("settings-spell-label"));
// ...
ui.checkbox(&mut dialog.spell_check_buffer, i18n.t("settings-spell-enable"));
// ...
ui.label(egui::RichText::new(i18n.t("settings-spell-hint-save")).weak().size(12.0));
ui.label(egui::RichText::new(i18n.t("settings-spell-hint-underline")).weak().size(12.0));
ui.label(egui::RichText::new(i18n.t("settings-spell-dict")).weak().size(12.0));
```

- [ ] **步骤 6：替换快捷键标签页文本**

```rust
ui.label(i18n.t("settings-keybind-hint"));
// ...
if ui.button(i18n.t("settings-keybind-reset-all")).clicked() { ... }
// 表头
ui.label(egui::RichText::new(i18n.t("settings-keybind-header-action")).strong());
ui.label(egui::RichText::new(i18n.t("settings-keybind-header-shortcut")).strong());
// 单元格
let cell_text = if is_capturing {
    i18n.t("settings-keybind-capturing")
} else {
    &binding.display()
};
// ...
if ui.button(i18n.t("settings-keybind-reset")).on_hover_text(i18n.t("settings-keybind-restore-tooltip")).clicked() { ... }
// 冲突
egui::RichText::new(format!("{} {}", i18n.t("settings-keybind-conflict"), cell_text))
```

操作名称翻译（原来直接调用 `action.display_name()` 的地方）：
```rust
// 原来：ui.label(action.display_name());
// 改为：
ui.label(i18n.t(action.display_name()));
```

- [ ] **步骤 7：添加语言选择器**

在设置对话框的标签栏和分隔线之后、具体标签页内容之前，添加语言选择：

```rust
// 语言选择器（在所有标签页共享的顶部区域）
ui.horizontal(|ui| {
    ui.label(i18n.t("settings-language-label"));
    egui::ComboBox::from_id_salt("lang_selector")
        .selected_text(i18n.lang().display_name())
        .show_ui(ui, |ui| {
            for lang in &[Lang::ZhCN, Lang::EnUS] {
                let label = lang.display_name();
                if ui.selectable_label(i18n.lang() == *lang, label).clicked() {
                    i18n.set_lang(*lang);
                    // 同步更新 AppConfig 的字符串
                    app_config.lang = lang.as_str().to_string();
                }
            }
        });
});
ui.separator();
```

- [ ] **步骤 8：替换保存/取消按钮**

```rust
if ui.button(i18n.t("settings-btn-save")).clicked() { ... }
if ui.button(i18n.t("settings-btn-cancel")).clicked() { ... }
```

- [ ] **步骤 9：更新 settings_dialog 单元测试**

现有测试调用 `open_dialog` 并检查对话框状态。这些不需要改——它们不涉及文本渲染。

---

### 任务 12：zdown-app 集成 — tab_bar.rs

**文件：**
- 修改：`crates/zdown-app/src/tab_bar.rs`

- [ ] **步骤 1：添加 import 和修改函数签名**

```rust
use i18n::I18n;
```

修改 `show_tab_bar()` 签名，增加 `i18n: &I18n` 参数。

- [ ] **步骤 2：替换右键菜单文本**

```rust
response.context_menu(|ui| {
    if ui.button(i18n.t("tab-close-others")).clicked() {
        state.close_other_tabs(tab_idx);
        ui.close();
    }
    if state.tab_count() > tab_idx + 1 && ui.button(i18n.t("tab-close-right")).clicked() {
        state.close_tabs_to_right(tab_idx);
        ui.close();
    }
    if has_path_for_menu && ui.button(i18n.t("tab-copy-path")).clicked() {
        if let Some(ref path) = state.tabs()[tab_idx].path {
            ui.ctx().copy_text(path.display().to_string());
        }
        ui.close();
    }
});
```

注意：`"关闭其他"` → `i18n.t("tab-close-others")`，`"关闭右侧"` → `i18n.t("tab-close-right")`，`"复制路径"` → `i18n.t("tab-copy-path")`

---

### 任务 13：zdown-app 集成 — outline_view.rs

**文件：**
- 修改：`crates/zdown-app/src/outline_view.rs`

- [ ] **步骤 1：添加 import 和修改函数签名**

```rust
use i18n::I18n;
use fluent_bundle::FluentArgs;
```

修改 `show_outline_panel()` 签名，增加 `i18n: &I18n` 参数。

- [ ] **步骤 2：替换面板文本**

```rust
// 原来：ui.heading(format!("📑 大纲 ({})", items.len()));
let mut args = FluentArgs::new();
args.set("count", items.len() as i64);
ui.heading(i18n.tr("outline-heading", Some(&args)));

// 原来：ui.label(egui::RichText::new("（无标题）").weak());
ui.label(egui::RichText::new(i18n.t("outline-empty")).weak());
```

- [ ] **步骤 3：替换 inlines_to_plain 中的硬编码文本**

```rust
// 原来：text.push_str("[图片: ");
// 改为：
text.push('[');
text.push_str(&i18n.t("outline-image-prefix"));  // "图片:" / "Image:"
text.push(' ');
```

注意：`inlines_to_plain` 当前不接受 `i18n` 参数。需要改为：

```rust
fn inlines_to_plain(inlines: &[Inline], i18n: &I18n) -> String {
```

同时更新调用方：
- `extract_outline(doc: &Document, i18n: &I18n) -> Vec<OutlineItem>`
- 在 `show_outline_panel` 中调用 `extract_outline(&doc, i18n)`

- [ ] **步骤 4：替换空标题文本**

```rust
// 原来：let text = if text.is_empty() { "(空标题)".to_string() } else { text };
let text = if text.is_empty() {
    i18n.t("outline-empty-heading")
} else {
    text
};
```

---

### 任务 14：运行完整测试套件并修复

**文件：** 无特定文件，验证整体一致性。

- [ ] **步骤 1：编译整个工作区**

```bash
cargo check --workspace
```

预期：编译通过，无错误无警告。

- [ ] **步骤 2：运行全部测试**

```bash
cargo test --workspace
```

预期：全部测试通过。

- [ ] **步骤 3：运行 clippy**

```bash
cargo clippy --workspace -- -D warnings
```

预期：clippy clean。

- [ ] **步骤 4：运行 fmt**

```bash
cargo fmt -- --check
```

预期：格式化干净。

- [ ] **步骤 5：修复任何编译错误或测试失败**

逐个修复问题，确保全部通过。

- [ ] **步骤 6：Commit**

```bash
git add -A
git commit -m "feat(i18n): integrate i18n into zdown-app with full zh-CN/en-US support

- Add I18n instance to ZdownApp, pass to all UI functions
- Replace all hardcoded Chinese text with i18n.t() calls
- Add language selector in settings dialog with runtime hot-switch
- Update workspace dialog functions to accept translated titles
- Convert Action::display_name() and ViewMode::label() to FTL keys
- Add lang field to AppConfig for persistence

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 15：手动验证

- [ ] **步骤 1：编译 release**

```bash
cargo build --release
```

- [ ] **步骤 2：启动应用**

```bash
cargo run --release
```

验证清单：
- [ ] 默认中文界面，菜单栏显示"文件/编辑/视图"
- [ ] 打开设置对话框，看到"样式/图片/拼写/快捷键"四个标签页
- [ ] 切换到英文："Style/Image/Spell/Keybind"
- [ ] 保存设置
- [ ] 重启应用，确认语言设置持久化
- [ ] 未保存修改时退出，确认对话框显示正确语言
- [ ] 搜索栏文本随语言切换
- [ ] 大纲面板文本随语言切换
- [ ] 标签页右键菜单随语言切换
