# 拼写检查 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 zdown 源码视图添加英文拼写检查——保存时触发，错误单词显示红色波浪下划线。

**架构：** 新增 `crates/spellcheck` crate（基于 spellbook + 嵌入 en_US 词典），集成到 `EditorState` 和 `source_view` 的逐行渲染管线中。

**技术栈：** Rust 2024, spellbook 0.4, egui 0.34, ropey

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `crates/spellcheck/Cargo.toml` | 新增 | crate 元数据，spellbook 依赖 |
| `crates/spellcheck/src/lib.rs` | 新增 | SpellError 类型 + SpellChecker 公开接口 |
| `crates/spellcheck/src/checker.rs` | 新增 | check() 实现（词法分析 + token 过滤 + 拼写查询） |
| `crates/spellcheck/src/dict/en_US.aff` | 新增 | Hunspell 词缀规则（嵌入） |
| `crates/spellcheck/src/dict/en_US.dic` | 新增 | 英文词表（嵌入） |
| `crates/config/src/lib.rs` | 修改 | AppConfig 新增 spell_check_enabled 字段 |
| `crates/zdown-app/Cargo.toml` | 修改 | 添加 spellcheck 依赖 |
| `crates/zdown-app/src/editor_state.rs` | 修改 | EditorState 新增 SpellChecker + spell_errors + run_spell_check() |
| `crates/zdown-app/src/settings_dialog.rs` | 修改 | 新增 SettingsTab::Spell + UI |
| `crates/zdown-app/src/menu.rs` | 修改 | 保存后触发 run_spell_check() |
| `crates/zdown-app/src/source_view.rs` | 修改 | render_text_with_cursor 新增 spell_errors 参数 + 波浪线绘制 |

---

### 任务 1：创建 spellcheck crate 骨架

**文件：**
- 创建：`crates/spellcheck/Cargo.toml`
- 创建：`crates/spellcheck/src/lib.rs`

- [ ] **步骤 1：编写 Cargo.toml**

```toml
[package]
name = "spellcheck"
version = "0.1.0"
edition = "2024"

[dependencies]
spellbook = "0.4"
```

- [ ] **步骤 2：编写 lib.rs（类型定义 + 桩代码）**

```rust
//! 英文拼写检查。基于 spellbook + 嵌入 en_US Hunspell 词典。

mod checker;

/// 拼写错误信息。
#[derive(Debug, Clone, PartialEq)]
pub struct SpellError {
    /// 错误单词。
    pub word: String,
    /// 在原文中的字节偏移 (start_byte, end_byte)。
    pub span: (usize, usize),
}

/// 拼写检查器。
pub struct SpellChecker {
    dict: spellbook::Dictionary,
}

impl SpellChecker {
    /// 从嵌入的 en_US 词典构建。
    /// 词典文件通过 include_str! 编译时嵌入。
    pub fn new() -> Result<Self, SpellcheckError> {
        let aff = include_str!("dict/en_US.aff");
        let dic = include_str!("dict/en_US.dic");
        let dict = spellbook::Dictionary::new(aff, dic)
            .map_err(|e| SpellcheckError::Parse(e.to_string()))?;
        Ok(Self { dict })
    }

    /// 检查单个单词。
    pub fn check_word(&self, word: &str) -> bool {
        self.dict.check(word)
    }

    /// 检查整段文本，返回所有拼写错误。
    /// 调用 checker::check() 执行完整的词法分析与过滤逻辑。
    pub fn check(&self, text: &str) -> Vec<SpellError> {
        checker::check(self, text)
    }
}

/// 拼写检查错误类型。
#[derive(Debug)]
pub enum SpellcheckError {
    /// 词典解析失败。
    Parse(String),
}

impl std::fmt::Display for SpellcheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpellcheckError::Parse(msg) => write!(f, "词典解析失败: {msg}"),
        }
    }
}
```

- [ ] **步骤 3：验证编译通过**

```bash
cargo check -p spellcheck
```

预期：编译失败（checker.rs 尚未创建），这是预期的。

- [ ] **步骤 4：Commit**

```bash
git add crates/spellcheck/
git commit -m "feat(spellcheck): add crate skeleton with SpellError and SpellChecker types"
```

---

### 任务 2：获取并嵌入 en_US 词典

**文件：**
- 创建：`crates/spellcheck/src/dict/en_US.aff`
- 创建：`crates/spellcheck/src/dict/en_US.dic`

- [ ] **步骤 1：获取 en_US Hunspell 词典**

英文 Hunspell 词典源自 SCOWL (Spell Checker Oriented Word Lists) 项目。
我们需要中等规模的词典（约 50K 词条，~500KB）。

从 LibreOffice 词典镜像下载：

```bash
# 下载 en_US 词典（使用 curl 从 GitHub raw）
curl -L -o crates/spellcheck/src/dict/en_US.aff \
  "https://raw.githubusercontent.com/LibreOffice/dictionaries/master/en/en_US.aff"
curl -L -o crates/spellcheck/src/dict/en_US.dic \
  "https://raw.githubusercontent.com/LibreOffice/dictionaries/master/en/en_US.dic"
```

如果网络不可用，备选方案：从任何 Hunspell 词典目录复制，或使用更小的词表。

- [ ] **步骤 2：验证文件存在且非空**

```bash
wc -c crates/spellcheck/src/dict/en_US.aff crates/spellcheck/src/dict/en_US.dic
```

- [ ] **步骤 3：验证编译**

```bash
cargo check -p spellcheck
```

预期：`checker` 模块未找到，报编译错误。词典文件自身不需要编译（include_str! 仅在运行时使用）。

- [ ] **步骤 4：Commit**

```bash
git add crates/spellcheck/src/dict/
git commit -m "feat(spellcheck): add en_US Hunspell dictionary files"
```

---

### 任务 3：实现 checker.rs 核心逻辑

**文件：**
- 创建：`crates/spellcheck/src/checker.rs`

- [ ] **步骤 1：编写 checker.rs**

```rust
//! check() 实现：词法分析 + token 过滤 + 拼写查询。

use crate::{SpellChecker, SpellError};

/// 检查全文，返回拼写错误列表。
///
/// # 算法
/// 1. 逐字节遍历，识别代码围栏块（```）和行内代码（`）并跳过
/// 2. 在非代码文本中，按非字母字符拆分提取单词 token
/// 3. 过滤：跳过长度 ≤ 1、纯数字、URL
/// 4. 对保留的 token 调用 spellbook check()
pub fn check(checker: &SpellChecker, text: &str) -> Vec<SpellError> {
    let mut errors = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_fenced_code = false;
    let mut line_start = 0usize;

    while i < len {
        // 检测行首
        if i == 0 || bytes[i - 1] == b'\n' {
            line_start = i;
            // 检测围栏代码块的开始/结束
            if i + 2 < len && bytes[i] == b'`' && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
                in_fenced_code = !in_fenced_code;
                // 跳过本行剩余
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
        }

        if in_fenced_code {
            i += 1;
            continue;
        }

        // 检测行内代码 `...`
        if bytes[i] == b'`' {
            i += 1;
            while i < len && bytes[i] != b'`' {
                i += 1;
            }
            if i < len {
                i += 1; // 跳过闭合 `
            }
            continue;
        }

        // 检测单词起始（字母或撇号，不包括数字）
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'\'' {
            let start = i;
            i += 1;
            while i < len && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'\'') {
                i += 1;
            }
            let word_bytes = &bytes[start..i];
            let word = std::str::from_utf8(word_bytes).unwrap_or("");

            // 过滤规则
            if should_check(word) && !checker.dict.check(word) {
                errors.push(SpellError {
                    word: word.to_string(),
                    span: (start, i),
                });
            }
        } else {
            i += 1;
        }
    }

    errors
}

/// 判断一个 token 是否应该被拼写检查。
fn should_check(word: &str) -> bool {
    // 跳过空或单字符
    if word.len() <= 1 {
        return false;
    }
    // 跳过纯数字
    if word.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    // 跳过 URL 片段
    if word.contains("://") {
        return false;
    }
    // 跳过全大写缩写（如 HTML, CSS, API）
    if word.len() >= 2 && word.chars().all(|c| c.is_ascii_uppercase()) {
        return false;
    }
    // 跳过带数字的混合 token（如 "v2", "foo123"）
    if word.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_checker() -> SpellChecker {
        SpellChecker::new().expect("load dict")
    }

    #[test]
    fn check_hello_is_correct() {
        let c = make_checker();
        assert!(c.check_word("hello"));
    }

    #[test]
    fn check_misspelling_is_wrong() {
        let c = make_checker();
        assert!(!c.check_word("helo"));
    }

    #[test]
    fn check_all_correct_returns_empty() {
        let c = make_checker();
        let errors = c.check("hello world");
        assert!(errors.is_empty());
    }

    #[test]
    fn check_misspelled_returns_errors() {
        let c = make_checker();
        let errors = c.check("helo wrld");
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].word, "helo");
        assert_eq!(errors[1].word, "wrld");
    }

    #[test]
    fn skip_numbers() {
        let c = make_checker();
        let errors = c.check("123 456");
        assert!(errors.is_empty());
    }

    #[test]
    fn skip_single_char() {
        let c = make_checker();
        let errors = c.check("a b c");
        assert!(errors.is_empty());
    }

    #[test]
    fn skip_fenced_code_block() {
        let c = make_checker();
        let errors = c.check("```\nhelo\n```\nhelo");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].word, "helo");
    }

    #[test]
    fn skip_inline_code() {
        let c = make_checker();
        let errors = c.check("`helo` helo");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].word, "helo");
    }

    #[test]
    fn skip_all_caps_abbreviation() {
        let c = make_checker();
        let errors = c.check("HTML CSS API");
        assert!(errors.is_empty());
    }

    #[test]
    fn skip_mixed_digit_tokens() {
        let c = make_checker();
        let errors = c.check("v2 foo123 3d");
        assert!(errors.is_empty());
    }
}
```

- [ ] **步骤 2：验证编译**

```bash
cargo check -p spellcheck
```

预期：编译成功（WARNING 可能会有 dead_code 警告）。

- [ ] **步骤 3：运行单元测试**

```bash
cargo test -p spellcheck
```

预期：所有测试通过。

- [ ] **步骤 4：Commit**

```bash
git add crates/spellcheck/src/checker.rs crates/spellcheck/src/lib.rs
git commit -m "feat(spellcheck): implement check() with code block skipping and token filtering"
```

---

### 任务 4：AppConfig 新增 spell_check_enabled

**文件：**
- 修改：`crates/config/src/lib.rs`

- [ ] **步骤 1：在 AppConfig 中新增字段**

在 `crates/config/src/lib.rs` 中，找到 `pub struct AppConfig` 定义，在 `editor_font` 字段之后添加：

```rust
/// 拼写检查开关。默认启用。
#[serde(default = "default_spell_check")]
pub spell_check_enabled: bool,
```

在文件末尾（`#[cfg(test)]` 之前）添加默认值函数：

```rust
fn default_spell_check() -> bool {
    true
}
```

- [ ] **步骤 2：运行现有测试确认不破坏**

```bash
cargo test -p config
```

预期：所有现有测试通过。

- [ ] **步骤 3：添加配置序列化测试**

在 `crates/config/src/lib.rs` 的 `#[cfg(test)] mod tests` 中添加：

```rust
#[test]
fn spell_check_default_enabled() {
    let config = AppConfig::default();
    assert!(config.spell_check_enabled);
}

#[test]
fn spell_check_deserialize_missing_field_defaults_true() {
    let toml_str = r#"
theme = "dark"
[editor_font]
family = "monospace"
size = 14.0
"#;
    let config: AppConfig = toml::from_str(toml_str).expect("parse");
    assert!(config.spell_check_enabled);
}

#[test]
fn spell_check_roundtrip() {
    let mut config = AppConfig::default();
    config.spell_check_enabled = false;
    let toml_str = toml::to_string_pretty(&config).expect("serialize");
    let restored: AppConfig = toml::from_str(&toml_str).expect("deserialize");
    assert!(!restored.spell_check_enabled);
}
```

- [ ] **步骤 4：运行测试验证**

```bash
cargo test -p config
```

预期：全部通过（包括新增的 3 个测试）。

- [ ] **步骤 5：Commit**

```bash
git add crates/config/src/lib.rs
git commit -m "feat(config): add spell_check_enabled field to AppConfig"
```

---

### 任务 5：SettingsDialog 新增"拼写"标签页

**文件：**
- 修改：`crates/zdown-app/src/settings_dialog.rs`

- [ ] **步骤 1：更新 SettingsTab 枚举和 SettingsDialog 结构体**

在 `crates/zdown-app/src/settings_dialog.rs` 中：

修改 `SettingsTab` 枚举，新增 `Spell` 变体：
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum SettingsTab {
    Css,
    Font,
    Image,
    Spell,  // 新增
}
```

在 `SettingsDialog` 结构体中新增字段：
```rust
pub struct SettingsDialog {
    pub open: bool,
    active_tab: SettingsTab,
    css_buffer: String,
    font_family_buffer: String,
    font_size_buffer: f32,
    local_dir_buffer: String,
    smms_token_buffer: String,
    strategy_buffer: usize,
    spell_check_buffer: bool,  // 新增
}
```

在 `Default` 实现中添加默认值：
```rust
impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            active_tab: SettingsTab::Css,
            css_buffer: String::new(),
            font_family_buffer: "monospace".to_string(),
            font_size_buffer: 14.0,
            local_dir_buffer: "images".to_string(),
            smms_token_buffer: String::new(),
            strategy_buffer: 0,
            spell_check_buffer: true,  // 新增
        }
    }
}
```

- [ ] **步骤 2：更新 open_dialog 方法签名和实现**

修改 `open_dialog` 签名，新增 `spell_check_enabled: bool` 参数：

```rust
impl SettingsDialog {
    pub fn open_dialog(
        &mut self,
        current_css: Option<&str>,
        image_config: &ImageHostingConfig,
        editor_font: &EditorFontConfig,
        spell_check_enabled: bool,  // 新增
    ) {
        self.open = true;
        self.active_tab = SettingsTab::Css;
        self.css_buffer = current_css.unwrap_or("").to_string();
        self.local_dir_buffer = image_config.local_dir.clone();
        self.smms_token_buffer = image_config.smms.api_token.clone();
        self.strategy_buffer = match image_config.default_strategy {
            ImageStrategy::Local => 0,
            ImageStrategy::Base64 => 1,
            ImageStrategy::SmMs => 2,
        };
        self.font_family_buffer = editor_font.family.clone();
        self.font_size_buffer = editor_font.size;
        self.spell_check_buffer = spell_check_enabled;  // 新增
    }
}
```

- [ ] **步骤 3：在标签栏中添加"拼写"按钮**

在 `ui.horizontal(|ui| { ... })` 标签栏中添加：
```rust
ui.selectable_value(&mut dialog.active_tab, SettingsTab::Spell, "拼写");
```

- [ ] **步骤 4：在 match dialog.active_tab 中添加 Spell 分支**

在 `match dialog.active_tab` 块中，`SettingsTab::Image` 分支之后添加：

```rust
SettingsTab::Spell => {
    ui.label("英文拼写检查：");
    ui.add_space(4.0);

    ui.checkbox(&mut dialog.spell_check_buffer, "启用英文拼写检查");

    ui.add_space(8.0);

    ui.label(
        egui::RichText::new("拼写检查在保存文件时自动执行。")
            .weak()
            .size(12.0),
    );
    ui.label(
        egui::RichText::new("错误单词将以红色波浪下划线标记。")
            .weak()
            .size(12.0),
    );

    ui.add_space(8.0);

    ui.label(
        egui::RichText::new("词典：English (United States) — en_US")
            .weak()
            .size(12.0),
    );
}
```

- [ ] **步骤 5：在"保存"按钮处理中添加 spell_check 持久化**

在 `if ui.button("保存").clicked()` 块中，图片设置保存之后、`app_config.save()` 之前添加：

```rust
// 拼写检查设置
app_config.spell_check_enabled = dialog.spell_check_buffer;
```

- [ ] **步骤 6：更新所有 open_dialog 调用点**

需要找到所有调用 `open_dialog` 的地方，添加新参数。使用 grep 查找：

```bash
rg "open_dialog" crates/
```

预期找到的调用点在 `menu.rs`（或 `main.rs`）。
更新每个调用点，传入 `app_config.spell_check_enabled`。

- [ ] **步骤 7：更新 settings_dialog 测试**

更新现有测试中的 `open_dialog` 调用，添加新参数 `true`。新增测试：

```rust
#[test]
fn open_dialog_populates_spell_check_buffer() {
    let mut dialog = SettingsDialog::default();
    dialog.open_dialog(None, &Default::default(), &EditorFontConfig::default(), false);
    assert!(dialog.open);
    assert!(!dialog.spell_check_buffer);
}

#[test]
fn open_dialog_default_spell_check_enabled() {
    let mut dialog = SettingsDialog::default();
    dialog.open_dialog(None, &Default::default(), &EditorFontConfig::default(), true);
    assert!(dialog.spell_check_buffer);
}
```

- [ ] **步骤 8：运行测试 + 编译检查**

```bash
cargo test -p zdown-app
cargo clippy -p zdown-app
```

- [ ] **步骤 9：Commit**

```bash
git add crates/zdown-app/src/settings_dialog.rs crates/zdown-app/src/menu.rs
git commit -m "feat(settings): add spell check toggle tab in settings dialog"
```

---

### 任务 6：EditorState 集成 SpellChecker

**文件：**
- 修改：`crates/zdown-app/Cargo.toml`
- 修改：`crates/zdown-app/src/editor_state.rs`

- [ ] **步骤 1：添加 spellcheck 依赖**

在 `crates/zdown-app/Cargo.toml` 的 `[dependencies]` 中添加：

```toml
spellcheck = { path = "../spellcheck" }
```

- [ ] **步骤 2：EditorState 新增字段**

在 `crates/zdown-app/src/editor_state.rs` 中：

添加 import：
```rust
use spellcheck::{SpellChecker, SpellError};
```

在 `EditorState` 结构体中新增两个字段：
```rust
pub struct EditorState {
    tabs: Vec<DocumentTab>,
    active_tab: usize,
    pub recent: RecentFiles,
    workspace: Workspace,
    pub should_exit: bool,
    pub status_message: String,
    /// 拼写检查器（启动时构建一次，装载失败时降级为空词典）。
    pub spell_checker: SpellChecker,
    /// 最近一次拼写检查的结果（按活跃标签页的文本）。
    pub spell_errors: Vec<SpellError>,
}
```

- [ ] **步骤 3：更新 EditorState::new() 构造**

在 `EditorState::new()` 中初始化 spell_checker（优雅降级）：

```rust
pub fn new() -> Self {
    let tab = DocumentTab::empty();
    let spell_checker = SpellChecker::new().unwrap_or_else(|e| {
        tracing::warn!("拼写检查器初始化失败（已禁用）: {e}");
        // 降级：创建一个总是返回"正确"的 fallback
        SpellChecker::fallback()
    });
    Self {
        tabs: vec![tab],
        active_tab: 0,
        recent: RecentFiles::load(),
        workspace: Workspace::new(),
        should_exit: false,
        status_message: String::new(),
        spell_checker,
        spell_errors: Vec::new(),
    }
}
```

`SpellChecker::fallback()` 需要在 `crates/spellcheck/src/lib.rs` 中添加：

```rust
impl SpellChecker {
    /// 降级：总是返回"拼写正确"的 fallback（词典加载失败时使用）。
    pub fn fallback() -> Self {
        // 使用一个最小词典（仅含 "a" 保证 struct 可构造）
        // spellbook 不接受空词典，使用最小有效内容
        let aff = "SET UTF-8\nTRY esianrtolcdugmphbyfvkwzESIANRTOLCDUGMPHBYFVKWZ'\n";
        let dic = "1\na\n";
        let dict = spellbook::Dictionary::new(aff, dic)
            .expect("fallback dictionary must be valid");
        Self { dict }
    }
}
```

等等——`fallback()` 可能太 hacky。更好的方式是用 `Option<SpellChecker>` 或者让 check() 直接返回空。

重新设计：`EditorState` 中使用 `Option<SpellChecker>`：

```rust
pub spell_checker: Option<SpellChecker>,
pub spell_errors: Vec<SpellError>,
```

`new()` 中：
```rust
let spell_checker = match SpellChecker::new() {
    Ok(sc) => Some(sc),
    Err(e) => {
        tracing::warn!("拼写检查器初始化失败: {e}");
        None
    }
};
```

`run_spell_check()` 中：
```rust
pub fn run_spell_check(&mut self, enabled: bool) {
    if !enabled {
        self.spell_errors.clear();
        return;
    }
    if let Some(ref checker) = self.spell_checker {
        let text = self.editor().to_string();
        self.spell_errors = checker.check(&text);
    }
}
```

这样可以完全避免 fallback hack。

- [ ] **步骤 4：添加 run_spell_check 方法**

在 `impl EditorState` 块中添加：

```rust
/// 执行拼写检查（保存文件后由 UI 层调用）。
/// 若拼写检查未启用或检查器不可用，清空错误列表。
pub fn run_spell_check(&mut self, enabled: bool) {
    if !enabled {
        self.spell_errors.clear();
        return;
    }
    if let Some(ref checker) = self.spell_checker {
        let text = self.editor().to_string();
        self.spell_errors = checker.check(&text);
    }
}
```

- [ ] **步骤 5：验证编译**

```bash
cargo check -p zdown-app
```

- [ ] **步骤 6：Commit**

```bash
git add crates/zdown-app/Cargo.toml crates/zdown-app/src/editor_state.rs crates/spellcheck/src/lib.rs
git commit -m "feat(editor): integrate SpellChecker into EditorState with Option fallback"
```

---

### 任务 7：保存流程触发拼写检查

**文件：**
- 修改：`crates/zdown-app/src/menu.rs`

- [ ] **步骤 1：在 handle_shortcuts 的 Ctrl+S 分支添加检查**

找到 `handle_shortcuts` 中的 Ctrl+S 处理（约第 342 行）：

修改前：
```rust
if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::S)) {
    if state.current_path().is_some() {
        let _ = state.save();
    } else {
        trigger_save_as(state);
    }
}
```

修改后：
```rust
if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::S)) {
    let saved = if state.current_path().is_some() {
        state.save().is_ok()
    } else {
        trigger_save_as(state);
        state.current_path().is_some() // save_as 成功后返回 true
    };
    if saved {
        state.run_spell_check(app_config.spell_check_enabled);
    }
}
```

需要确认 `app_config` 在 `handle_shortcuts` 中可用。检查函数签名——当前签名是 `handle_shortcuts(ctx, state, confirm)`，需要新增 `app_config: &AppConfig` 参数。

- [ ] **步骤 2：在菜单按钮"保存"分支添加检查**

找到 `show_menu` 中的"保存 (Ctrl+S)"按钮处理：

```rust
if ui.button("保存 (Ctrl+S)").clicked() {
    let saved = if state.current_path().is_none() {
        trigger_save_as(state);
        state.current_path().is_some()
    } else {
        state.save().is_ok()
    };
    if saved {
        state.run_spell_check(app_config.spell_check_enabled);
    }
}
```

- [ ] **步骤 3：在"保存所有"中也触发检查**

```rust
if ui.button("保存所有").clicked() {
    let (saved, _) = state.save_all();
    if saved > 0 {
        state.run_spell_check(app_config.spell_check_enabled);
    }
}
```

- [ ] **步骤 4：更新 handle_shortcuts 的调用方**

在 `main.rs` 中查找 `handle_shortcuts` 调用，确保传入 `app_config`。

- [ ] **步骤 5：验证编译 + 测试**

```bash
cargo check -p zdown-app
cargo test -p zdown-app
```

- [ ] **步骤 6：Commit**

```bash
git add crates/zdown-app/src/menu.rs crates/zdown-app/src/main.rs
git commit -m "feat(menu): trigger spell check on save (Ctrl+S / menu save)"
```

---

### 任务 8：source_view 渲染波浪下划线

**文件：**
- 修改：`crates/zdown-app/src/source_view.rs`

- [ ] **步骤 1：show_source_view 传递 spell_errors**

在 `show_source_view` 中，找到 `render_text_with_cursor` 调用，新增参数：

```rust
render_text_with_cursor(
    ui,
    &src,
    state.editor().cursor,
    highlighter,
    search,
    &state.spell_errors,  // 新增
);
```

- [ ] **步骤 2：更新 render_text_with_cursor 签名**

```rust
fn render_text_with_cursor(
    ui: &mut egui::Ui,
    src: &str,
    cursor: Cursor,
    highlighter: Option<&SourceHighlighter>,
    search: &SearchState,
    spell_errors: &[SpellError],  // 新增
) {
```

添加 import（文件顶部）：
```rust
use spellcheck::SpellError;
```

- [ ] **步骤 3：添加 find_line_spell_errors 辅助函数**

在 `render_text_with_cursor` 之前添加：

```rust
/// 查找指定行中的所有拼写错误（按列范围）。
/// 返回 Vec<(col_start, col_end)>，col 为字符列。
fn find_line_spell_errors(
    src: &str,
    spell_errors: &[SpellError],
    line_idx: usize,
) -> Vec<(usize, usize)> {
    if spell_errors.is_empty() {
        return Vec::new();
    }
    // 计算每行的起始字节偏移
    let mut line_starts: Vec<usize> = Vec::new();
    let mut byte_pos = 0usize;
    for line in src.lines() {
        line_starts.push(byte_pos);
        byte_pos += line.len() + 1; // +1 for newline char
    }

    let line_start_byte = *line_starts.get(line_idx)?;
    let line_end_byte = if line_idx + 1 < line_starts.len() {
        line_starts[line_idx + 1] - 1 // exclude newline
    } else {
        src.len()
    };

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for err in spell_errors {
        let (err_start, err_end) = err.span;
        if err_start >= line_start_byte && err_end <= line_end_byte {
            // 转为行内列偏移（字符数）
            let line_slice = &src[line_start_byte..line_end_byte];
            let prefix = &src[line_start_byte..err_start];
            let err_text = &src[err_start..err_end];
            let col_start = prefix.chars().count();
            let col_end = col_start + err_text.chars().count();
            ranges.push((col_start, col_end));
        }
    }
    ranges
}
```

- [ ] **步骤 4：在行绘制末尾添加波浪线**

在 `render_text_with_cursor` 的每行绘制末尾（光标矩形绘制之后），新增：

```rust
// 绘制拼写错误波浪线
let spell_ranges = find_line_spell_errors(src, spell_errors, line_idx);
for (col_start, col_end) in spell_ranges {
    let err_prefix: String = line
        .iter()
        .flat_map(|(_, t)| t.chars())
        .take(col_start)
        .collect();
    let err_text: String = line
        .iter()
        .flat_map(|(_, t)| t.chars())
        .skip(col_start)
        .take(col_end - col_start)
        .collect();
    let err_prefix_galley = ui.ctx().fonts_mut(|f| {
        f.layout_no_wrap(err_prefix, font_id.clone(), egui::Color32::WHITE)
    });
    let err_text_galley = ui.ctx().fonts_mut(|f| {
        f.layout_no_wrap(err_text, font_id.clone(), egui::Color32::WHITE)
    });
    let squiggly_start = egui::pos2(
        rect.min.x + err_prefix_galley.size().x,
        rect.max.y,  // 行底部
    );
    let squiggly_end = egui::pos2(
        squiggly_start.x + err_text_galley.size().x,
        rect.max.y,
    );
    paint_squiggly_underline(
        ui.painter(),
        squiggly_start,
        squiggly_end,
        egui::Color32::from_rgb(224, 108, 117), // 红色 #e06c75
    );
}
```

**注意**：上述代码针对有语法高亮的分支。fallback 分支（无高亮）需要相同的波浪线绘制逻辑。

- [ ] **步骤 5：添加 paint_squiggly_underline 函数**

在 `render_text_with_cursor` 之后添加：

```rust
/// 绘制红色波浪下划线。
fn paint_squiggly_underline(
    painter: &egui::Painter,
    start: egui::Pos2,
    end: egui::Pos2,
    color: egui::Color32,
) {
    let step = 3.0;
    let amp = 2.0;
    let y_base = start.y + 2.0;
    let mut points = Vec::new();
    let mut x = start.x;
    while x < end.x {
        let phase = ((x - start.x) / step) as i32;
        let y = y_base + if phase % 2 == 0 { -amp } else { amp };
        points.push(egui::pos2(x, y));
        x += step;
    }
    if points.len() >= 2 {
        painter.add(egui::Shape::line(points, egui::Stroke::new(1.0, color)));
    }
}
```

- [ ] **步骤 6：处理 fallback 分支（无 highlighter 时）**

在 fallback 分支中对 `line` 做同样的处理（`line` 是 `&str`，非 `&[(Style, &str)]`）：

```rust
// fallback 分支中的拼写错误绘制
let spell_ranges = find_line_spell_errors(src, spell_errors, line_idx);
for (col_start, col_end) in spell_ranges {
    let err_prefix: String = line.chars().take(col_start).collect();
    let err_text: String = line.chars().skip(col_start).take(col_end - col_start).collect();
    let err_prefix_galley = ui.ctx().fonts_mut(|f| {
        f.layout_no_wrap(err_prefix, font_id.clone(), egui::Color32::WHITE)
    });
    let err_text_galley = ui.ctx().fonts_mut(|f| {
        f.layout_no_wrap(err_text, font_id.clone(), egui::Color32::WHITE)
    });
    let squiggly_start = egui::pos2(
        rect.min.x + err_prefix_galley.size().x,
        rect.max.y,
    );
    let squiggly_end = egui::pos2(
        squiggly_start.x + err_text_galley.size().x,
        rect.max.y,
    );
    paint_squiggly_underline(ui.painter(), squiggly_start, squiggly_end, egui::Color32::from_rgb(224, 108, 117));
}
```

- [ ] **步骤 7：验证编译**

```bash
cargo check -p zdown-app
```

- [ ] **步骤 8：Commit**

```bash
git add crates/zdown-app/src/source_view.rs
git commit -m "feat(source_view): render red squiggly underlines for misspelled words"
```

---

### 任务 9：全项目编译 + clippy + fmt + 测试

**文件：** 无新建

- [ ] **步骤 1：cargo fmt**

```bash
cargo fmt --all
```

- [ ] **步骤 2：cargo clippy**

```bash
cargo clippy --all-targets -- -D warnings
```

修复所有 clippy 警告。

- [ ] **步骤 3：全量测试**

```bash
cargo test --all
```

预期：所有测试通过。

- [ ] **步骤 4：cargo build --release**

```bash
cargo build --release
```

预期：Release 构建成功，可执行文件可正常运行。

- [ ] **步骤 5：Commit**

```bash
git add -A
git commit -m "chore: cargo fmt + clippy fixes for spell check integration"
```

---

## 验证清单

实现完成后，手动验证以下场景：

1. `cargo test --all` 全部通过
2. `cargo clippy --all-targets` 零警告
3. `cargo fmt --all` 无格式化变更
4. 手动测试：打开编辑器 → 输入 `helo wrld` → Ctrl+S → 源码视图显示红色波浪线
5. 手动测试：设置 → 拼写标签页 → 取消勾选 → 保存 → Ctrl+S → 波浪线消失
6. 手动测试：`代码块内的 helo` 不标记，`行内代码` helo` 外部的标记
