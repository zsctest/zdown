# 多语言/国际化 (i18n) 设计规格

**日期**: 2026-06-21
**状态**: 已确认
**范围**: 阶段 4.5 — fluent 集成，中英双语

---

## 1. 目标

为 zdown Markdown 编辑器添加运行时热切换的多语言支持，首期覆盖中文简体 (zh-CN) 和英语 (en-US)。

### 覆盖范围
- 菜单栏全部文本（文件/编辑/视图/帮助）
- 设置对话框（样式/图片/拼写/快捷键全部四个标签页）
- 标签页右键菜单、搜索栏、大纲面板、状态栏
- 确认对话框（未保存提示等）
- Action 显示名称、ViewMode 标签
- 文件对话框标题（rfd 的 set_title）

### 非覆盖范围
- 错误消息 (tracing) — 保持英文，面向开发者
- 日志输出 — 同上
- 富文本内容（Markdown 正文）— 不翻译

---

## 2. 架构

### 2.1 新增 crate: `crates/i18n`

```
crates/i18n/
├── Cargo.toml
├── locales/
│   ├── zh-CN/
│   │   ├── menu.ftl
│   │   ├── settings.ftl
│   │   ├── editor.ftl
│   │   └── actions.ftl
│   └── en-US/
│       ├── menu.ftl
│       ├── settings.ftl
│       ├── editor.ftl
│       └── actions.ftl
└── src/
    ├── lib.rs
    └── resource.rs
```

### 2.2 依赖

```toml
[dependencies]
fluent = "0.16"
fluent-bundle = "0.15"
unic-langid = { version = "0.9", features = ["macros"] }
intl-memoizer = "0.5"
serde = { workspace = true }
```

### 2.3 核心类型

```rust
/// 支持的语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lang {
    ZhCN,
    EnUS,
}

impl Lang {
    /// 转换为 unic-langid。
    pub fn to_langid(&self) -> unic_langid::LanguageIdentifier { ... }

    /// 用户可读的显示名。
    pub fn display_name(&self) -> &'static str { ... }
}

/// 国际化管理器。
pub struct I18n {
    lang: Lang,
    bundles: HashMap<Lang, Vec<FluentBundle<FluentResource, IntlLangMemoizer>>>,
}

impl I18n {
    pub fn new() -> Self { ... }
    pub fn lang(&self) -> Lang { ... }
    pub fn set_lang(&mut self, lang: Lang) { ... }
    pub fn tr(&self, key: &str, args: Option<FluentArgs>) -> String { ... }
    pub fn t(&self, key: &str) -> String { ... }
}
```

### 2.4 FTL 资源划分

| 文件 | 领域 | 内容 |
|---|---|---|
| `menu.ftl` | 菜单栏 | 文件/编辑/视图菜单项、确认对话框文本 |
| `settings.ftl` | 设置面板 | 四个标签页的所有标签、提示、按钮 |
| `editor.ftl` | 编辑器UI | 搜索栏、标签页、大纲面板、状态消息模板 |
| `actions.ftl` | 操作名称 | Action::display_name 和 ViewMode::label |

### 2.5 FTL 示例

```ftl
# zh-CN/menu.ftl
menu-file = 文件
menu-file-new = 新建 (Ctrl+N)
menu-file-open = 打开... (Ctrl+O)
menu-file-save = 保存 (Ctrl+S)
menu-file-save-as = 另存为... (Ctrl+Shift+S)
menu-file-save-all = 保存所有
confirm-unsaved-title-quit = 未保存修改 - 退出
confirm-unsaved-title-close = 未保存修改 - 关闭标签页
confirm-unsaved-body = 当前文档有未保存修改。是否保存?
confirm-btn-save = 保存
confirm-btn-discard = 不保存
confirm-btn-cancel = 取消

# en-US/menu.ftl
menu-file = File
menu-file-new = New (Ctrl+N)
menu-file-open = Open... (Ctrl+O)
menu-file-save = Save (Ctrl+S)
menu-file-save-as = Save As... (Ctrl+Shift+S)
menu-file-save-all = Save All
confirm-unsaved-title-quit = Unsaved Changes - Quit
confirm-unsaved-title-close = Unsaved Changes - Close Tab
confirm-unsaved-body = The document has unsaved changes. Save?
confirm-btn-save = Save
confirm-btn-discard = Discard
confirm-btn-cancel = Cancel
```

带参数的消息：
```ftl
# zh-CN/editor.ftl
status-save-result = 保存完成：{$saved} 个文件
status-save-skipped = 保存完成：{$saved} 个文件，{$skipped} 个未命名文件已跳过
outline-heading = 📑 大纲 ({$count})
outline-empty = （无标题）
outline-empty-heading = （空标题）
search-find = 查找:
search-replace = 替换:
search-replace-btn = 替换
search-replace-all = 全部
search-count = {$current}/{$total}

# en-US/editor.ftl
status-save-result = Saved: {$saved} file(s)
status-save-skipped = Saved: {$saved} file(s), {$skipped} unnamed file(s) skipped
outline-heading = 📑 Outline ({$count})
outline-empty = (No headings)
outline-empty-heading = (Empty heading)
search-find = Find:
search-replace = Replace:
search-replace-btn = Replace
search-replace-all = All
search-count = {$current}/{$total}
```

### 2.6 编译时嵌入

FTL 文件通过 `include_str!()` 在编译时嵌入二进制，打包为 `FluentResource`。无运行时文件 I/O。

```rust
// resource.rs
pub fn load_resources() -> HashMap<Lang, Vec<FluentResource>> {
    let mut map = HashMap::new();
    map.insert(Lang::ZhCN, vec![
        FluentResource::try_new(include_str!("../locales/zh-CN/menu.ftl").to_string()).unwrap(),
        FluentResource::try_new(include_str!("../locales/zh-CN/settings.ftl").to_string()).unwrap(),
        FluentResource::try_new(include_str!("../locales/zh-CN/editor.ftl").to_string()).unwrap(),
        FluentResource::try_new(include_str!("../locales/zh-CN/actions.ftl").to_string()).unwrap(),
    ]);
    map.insert(Lang::EnUS, vec![
        FluentResource::try_new(include_str!("../locales/en-US/menu.ftl").to_string()).unwrap(),
        // ...
    ]);
    map
}
```

---

## 3. App 集成

### 3.1 ZdownApp 变更

```rust
struct ZdownApp {
    // ... existing fields ...
    i18n: I18n,
}

impl Default for ZdownApp {
    fn default() -> Self {
        let app_config = config::AppConfig::load().unwrap_or_default();
        let lang = match app_config.lang.as_str() {
            "en-US" => i18n::Lang::EnUS,
            _ => i18n::Lang::ZhCN,     // 默认中文
        };
        Self {
            // ...
            i18n: I18n::with_lang(lang),
        }
    }
}
```

### 3.2 函数签名变化

所有渲染 UI 的函数增加 `i18n: &I18n` 参数：

| 函数 | 位置 | 涉及文本 |
|---|---|---|
| `menu::show_menu()` | `menu.rs` | 菜单栏全部约 30 项 |
| `menu::show_confirm_dialog()` | `menu.rs` | 确认对话框 |
| `settings_dialog::show_settings_dialog()` | `settings_dialog.rs` | 设置面板全部约 25 项 |
| `tab_bar::show_tab_bar()` | `tab_bar.rs` | 标签页右键菜单 3 项 |
| `outline_view::show_outline_panel()` | `outline_view.rs` | 大纲标题及状态文本 |
| `main.rs` 内联 UI | `main.rs` | 搜索栏标签、状态消息 |

### 3.3 跨 crate 处理

**`workspace/src/dialog.rs`** — 文件对话框标题
- 当前硬编码中文标题如 `set_title("打开 Markdown 文件")`
- 改为由调用方 (zdown-app) 传入翻译后的字符串
- 函数签名变为 `pick_open_file(title: &str) -> Option<PathBuf>`

**`config/src/keybinding.rs`** — `Action::display_name()`
- 保持返回 FTL key 字符串（如 `"action-save"`），不在此 crate 做翻译
- config crate 不依赖 i18n crate
- 翻译由 zdown-app 的 `settings_dialog` 在渲染时完成：`i18n.t(action.display_name())`

**依赖方向（关键）：**
- `i18n` → 无内部 crate 依赖（仅依赖 fluent/unic-langid 外部库）
- `config` → 不依赖 i18n（用 String 存储语言标识）
- `zdown-app` → 依赖 `config` + `i18n`，负责桥接

### 3.4 语言切换

设置对话框新增「语言」标签页或添加到「通用」设置：
- 下拉选择框显示 `Lang::ZhCN.display_name()` / `Lang::EnUS.display_name()`
- 选择后同步更新 `self.i18n.set_lang(new_lang)` 和 `self.app_config.lang`
- 保存时写入 `config.toml` 持久化

### 3.5 AppConfig 扩展

`AppConfig.lang` 用 `String` 存储语言标识符，避免 config crate 依赖 i18n：

```rust
pub struct AppConfig {
    // ... existing fields ...
    /// 界面语言，值为 "zh-CN" 或 "en-US"。默认 "zh-CN"。
    #[serde(default = "default_lang")]
    pub lang: String,
}

fn default_lang() -> String { "zh-CN".to_string() }
```

ZdownApp 初始化时解析：
```rust
let lang = match app_config.lang.as_str() {
    "en-US" => i18n::Lang::EnUS,
    _ => i18n::Lang::ZhCN,
};
self.i18n = I18n::with_lang(lang);
```

---

## 4. 数据流

```
config.toml (lang = "zh-CN")
        │
        ▼
AppConfig::load() ──→ ZdownApp::default() ──→ I18n::with_lang(Lang::ZhCN)
        │                    (解析 string → Lang)
        ▼
  I18n::t("menu-file") ──→ "文件"    (每帧 ui() 调用)
  I18n::t("menu-file") ──→ "File"    (set_lang(EnUS) 后)
        │
        ▼
  设置面板语言下拉 ──→ i18n.set_lang() ──→ app_config.save()
                        (热切换)          (持久化 "zh-CN" / "en-US" 到 toml)
```

---

## 5. 错误处理

- **FTL 文件加载失败**：`include_str!()` 在编译时执行，文件缺失会导致编译错误，不存在运行时加载失败
- **FluentResource 解析失败**：`FluentResource::try_new()` 返回 `Result`，使用 `expect()` 在初始化阶段处理（唯一允许 expect 的场景：静态资源编译期错误意味着代码 broken，不应 swallow）
- **翻译 key 缺失**：Fluent 默认返回 key 本身作为回退，即 `i18n.t("nonexistent")` 返回 `"nonexistent"`。开发时方便排查。
- **语言切换并发**：单线程 egui 框架，无并发问题

---

## 6. 性能考量

- `I18n::t()` 每次调用都执行 Fluent 的 message 解析和查找。由于 egui 是即时模式 GUI（每帧重绘），需要确保翻译缓存：
- 关键热点（如每帧多次调用的 `t("search-find")`）通过 Fluent 内部的 memoizer 自动缓存
- 复杂参数插值在每次调用时重新执行，开销可控
- 不引入额外全局状态或锁

---

## 7. 测试策略

### 7.1 单元测试 (`crates/i18n`)

- `I18n::new()` 加载后所有语言 bundle 非空
- `t()` 返回中文/英文正确 translation
- `tr()` 带参数插值正确
- 切换 `set_lang()` 后 `t()` 返回对应语言
- 缺失 key 返回 key 自身

### 7.2 集成测试 (`crates/zdown-app`)

- 覆盖至少一个 UI 路径的中/英文渲染结果差异
- 语言切换后菜单文本变更

### 7.3 覆盖率目标

i18n crate: >= 90%

---

## 8. 实现顺序

1. 创建 `crates/i18n` 目录骨架、Cargo.toml
2. 编写 `FluentResource` 加载逻辑 (`resource.rs`)
3. 编写 `I18n` 核心 API (`lib.rs`)
4. 编写全部 FTL 翻译文件（中文基线 + 英文翻译）
5. 编写 i18n 单元测试
6. 修改 `config` crate：`Lang` 枚举、`AppConfig.lang` 字段
7. 修改 `workspace` crate：文件对话框接受翻译标题
8. 修改 `zdown-app`：全链路集成，逐个文件替换硬编码文本
9. 运行完整测试套件并修复
10. 手动验证中英切换

---

## 9. 关键决策记录

| 决策 | 选项 | 理由 |
|---|---|---|
| 语言切换方式 | 运行时热切换 | 用户体验最优 |
| 资源管理 | 每领域双文件 | 按领域分 FTL，清晰可维护 |
| API 风格 | 传入 locale 参数 | 显式依赖，符合 Rust 惯例 |
| cross-crate 耦合 | config 不依赖 i18n | 最小化修改面，保持关注点分离 |
| 编译方式 | include_str! 嵌入 | 零文件 I/O，编译期检查 |
