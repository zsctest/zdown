# 自定义快捷键映射 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 实现 GUI 内快捷键重绑定功能 — 用户可在设置对话框中查看和自定义所有菜单级快捷键。

**架构：** 在 config crate 新增 `keybinding` 模块（Action 枚举 + KeyBinding + Keymap delta 集合）。Keymap 作为 `AppConfig` 的可选字段持久化到 config.toml。zdown-app 层的 `handle_shortcuts` 从硬编码 if 改为查表驱动，设置对话框新增「快捷键」标签页含按键捕获交互。

**技术栈：** Rust 2024, egui (eframe), serde, toml

---

## 文件结构

| 文件 | 职责 |
|---|---|
| `crates/config/src/keybinding.rs` (新建) | Action 枚举、Modifiers、KeyBinding、Keymap 数据结构 + 默认绑定 + 序列化 |
| `crates/config/src/lib.rs` (修改) | 声明 keybinding 模块；AppConfig 增加 keymap 字段；重新导出 Keymap |
| `crates/zdown-app/src/menu.rs` (修改) | handle_shortcuts 改为查表驱动；新增 execute_action 分发函数；show_menu 签名增加 keymap |
| `crates/zdown-app/src/settings_dialog.rs` (修改) | 新增 Keybind 标签页 + 按键捕获逻辑 + KeybindingCapture 状态 |
| `crates/zdown-app/src/main.rs` (修改) | 传递 keymap 引用；移除 main.rs 中 view 快捷键（移入 handle_shortcuts） |

---

### 任务 1：创建 keybinding.rs 数据模型

**文件：**
- 创建：`crates/config/src/keybinding.rs`

- [ ] **步骤 1：编写失败的测试**

在 `crates/config/src/keybinding.rs` 底部编写测试模块：

```rust
#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn every_action_has_default_binding() {
        for action in Action::all() {
            let binding = action.default_binding();
            assert!(!binding.key_name.is_empty(), "{action:?} 默认绑定 key_name 为空");
        }
    }

    #[test]
    fn save_default_is_ctrl_s() {
        let binding = Action::Save.default_binding();
        assert!(binding.modifiers.ctrl);
        assert!(!binding.modifiers.shift);
        assert_eq!(binding.key_name, "S");
    }

    #[test]
    fn save_as_default_is_ctrl_shift_s() {
        let binding = Action::SaveAs.default_binding();
        assert!(binding.modifiers.ctrl);
        assert!(binding.modifiers.shift);
        assert_eq!(binding.key_name, "S");
    }

    #[test]
    fn keymap_resolve_returns_default_when_no_override() {
        let keymap = Keymap::default();
        let binding = keymap.resolve(&Action::Save);
        assert_eq!(binding, Action::Save.default_binding());
    }

    #[test]
    fn keymap_resolve_returns_override_when_set() {
        let mut keymap = Keymap::default();
        let custom = KeyBinding {
            modifiers: Modifiers { ctrl: false, shift: false, alt: true },
            key_name: "X".into(),
        };
        keymap.set_override(Action::Save, custom.clone());
        assert_eq!(keymap.resolve(&Action::Save), custom);
    }

    #[test]
    fn keymap_clear_override_restores_default() {
        let mut keymap = Keymap::default();
        let custom = KeyBinding {
            modifiers: Modifiers { ctrl: false, shift: false, alt: true },
            key_name: "X".into(),
        };
        keymap.set_override(Action::Save, custom);
        keymap.clear_override(&Action::Save);
        assert_eq!(keymap.resolve(&Action::Save), Action::Save.default_binding());
    }

    #[test]
    fn keymap_detect_conflict_returns_conflicting_action() {
        let mut keymap = Keymap::default();
        let binding = Action::Save.default_binding(); // Ctrl+S
        // Save 用 Ctrl+S，SaveAs 默认 Ctrl+Shift+S → 不冲突
        // 把 SaveAs 也改成 Ctrl+S 模拟冲突
        keymap.set_override(Action::SaveAs, binding.clone());
        // 查询 Save 的默认绑定是否与已存在的 override 冲突
        let existing = keymap.resolve(&Action::SaveAs);
        assert_eq!(existing, binding);
        // detect_conflict(当前action, 新binding)
        assert_eq!(keymap.detect_conflict(&Action::Save, &binding), Some(Action::SaveAs));
    }

    #[test]
    fn keymap_detect_conflict_same_action_not_conflict() {
        let mut keymap = Keymap::default();
        let custom = KeyBinding {
            modifiers: Modifiers { ctrl: false, shift: false, alt: true },
            key_name: "S".into(),
        };
        keymap.set_override(Action::Save, custom.clone());
        // 同一个 action 换绑定不视为冲突
        assert_eq!(keymap.detect_conflict(&Action::Save, &custom), None);
    }

    #[test]
    fn keymap_detect_conflict_no_conflict() {
        let keymap = Keymap::default();
        let unique = KeyBinding {
            modifiers: Modifiers { ctrl: true, shift: true, alt: true },
            key_name: "Q".into(),
        };
        assert_eq!(keymap.detect_conflict(&Action::Save, &unique), None);
    }

    #[test]
    fn keymap_serialize_roundtrip() {
        let mut keymap = Keymap::default();
        keymap.set_override(Action::Save, KeyBinding {
            modifiers: Modifiers { ctrl: false, shift: false, alt: true },
            key_name: "X".into(),
        });
        let toml_str = toml::to_string_pretty(&keymap).expect("serialize");
        let restored: Keymap = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(restored.resolve(&Action::Save), keymap.resolve(&Action::Save));
        // 未覆盖的 action 走默认
        assert_eq!(restored.resolve(&Action::Open), Action::Open.default_binding());
    }

    #[test]
    fn keymap_default_is_empty_overrides() {
        let keymap = Keymap::default();
        assert!(keymap.overrides.is_empty());
    }

    #[test]
    fn keymap_clear_all_removes_all_overrides() {
        let mut keymap = Keymap::default();
        keymap.set_override(Action::Save, KeyBinding {
            modifiers: Modifiers { ctrl: false, shift: false, alt: true },
            key_name: "X".into(),
        });
        keymap.set_override(Action::Open, KeyBinding {
            modifiers: Modifiers { ctrl: false, shift: false, alt: true },
            key_name: "O".into(),
        });
        keymap.clear_all();
        assert!(keymap.overrides.is_empty());
    }

    #[test]
    fn action_all_contains_all_variants() {
        let all = Action::all();
        assert_eq!(all.len(), 15);
    }

    #[test]
    fn action_display_name_non_empty() {
        for action in Action::all() {
            let name = action.display_name();
            assert!(!name.is_empty(), "{action:?} display_name 为空");
        }
    }

    #[test]
    fn modifiers_display_formats_correctly() {
        let m = Modifiers { ctrl: true, shift: false, alt: false };
        assert_eq!(m.display(), "Ctrl");
        let m = Modifiers { ctrl: true, shift: true, alt: false };
        assert_eq!(m.display(), "Ctrl+Shift");
        let m = Modifiers { ctrl: true, shift: false, alt: true };
        assert_eq!(m.display(), "Ctrl+Alt");
        let m = Modifiers { ctrl: true, shift: true, alt: true };
        assert_eq!(m.display(), "Ctrl+Shift+Alt");
        let m = Modifiers { ctrl: false, shift: false, alt: false };
        assert_eq!(m.display(), "");
    }

    #[test]
    fn keybinding_display_formats_correctly() {
        let kb = KeyBinding {
            modifiers: Modifiers { ctrl: true, shift: false, alt: false },
            key_name: "S".into(),
        };
        assert_eq!(kb.display(), "Ctrl+S");
        let kb = KeyBinding {
            modifiers: Modifiers { ctrl: true, shift: true, alt: false },
            key_name: "Tab".into(),
        };
        assert_eq!(kb.display(), "Ctrl+Shift+Tab");
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

```bash
cargo test -p config -- keybinding
```
预期：失败 — 模块不存在。

- [ ] **步骤 3：编写 keybinding.rs 数据模型**

```rust
//! 快捷键映射数据模型。
//!
//! 定义可配置的编辑器操作（Action）、按键绑定（KeyBinding）
//! 以及 delta 模式的快捷键映射表（Keymap）。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 所有可绑定快捷键的操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    Save,
    SaveAs,
    NewFile,
    Open,
    CloseTab,
    NextTab,
    PrevTab,
    MoveTabLeft,
    MoveTabRight,
    Undo,
    Redo,
    ViewSource,
    ViewPreview,
    ViewHybrid,
    ToggleTheme,
}

impl Action {
    /// 返回所有 Action 变体的切片。
    pub fn all() -> &'static [Action] {
        &[
            Action::Save,
            Action::SaveAs,
            Action::NewFile,
            Action::Open,
            Action::CloseTab,
            Action::NextTab,
            Action::PrevTab,
            Action::MoveTabLeft,
            Action::MoveTabRight,
            Action::Undo,
            Action::Redo,
            Action::ViewSource,
            Action::ViewPreview,
            Action::ViewHybrid,
            Action::ToggleTheme,
        ]
    }

    /// 返回该操作的默认快捷键绑定。
    pub fn default_binding(&self) -> KeyBinding {
        let (modifiers, key_name) = match self {
            Action::Save => (
                Modifiers { ctrl: true, shift: false, alt: false },
                "S",
            ),
            Action::SaveAs => (
                Modifiers { ctrl: true, shift: true, alt: false },
                "S",
            ),
            Action::NewFile => (
                Modifiers { ctrl: true, shift: false, alt: false },
                "N",
            ),
            Action::Open => (
                Modifiers { ctrl: true, shift: false, alt: false },
                "O",
            ),
            Action::CloseTab => (
                Modifiers { ctrl: true, shift: false, alt: false },
                "W",
            ),
            Action::NextTab => (
                Modifiers { ctrl: true, shift: false, alt: false },
                "Tab",
            ),
            Action::PrevTab => (
                Modifiers { ctrl: true, shift: true, alt: false },
                "Tab",
            ),
            Action::MoveTabLeft => (
                Modifiers { ctrl: true, shift: true, alt: false },
                "ArrowLeft",
            ),
            Action::MoveTabRight => (
                Modifiers { ctrl: true, shift: true, alt: false },
                "ArrowRight",
            ),
            Action::Undo => (
                Modifiers { ctrl: true, shift: false, alt: false },
                "Z",
            ),
            Action::Redo => (
                Modifiers { ctrl: true, shift: false, alt: false },
                "Y",
            ),
            Action::ViewSource => (
                Modifiers { ctrl: true, shift: false, alt: false },
                "Num1",
            ),
            Action::ViewPreview => (
                Modifiers { ctrl: true, shift: false, alt: false },
                "Num2",
            ),
            Action::ViewHybrid => (
                Modifiers { ctrl: true, shift: false, alt: false },
                "Num3",
            ),
            Action::ToggleTheme => (
                Modifiers { ctrl: true, shift: false, alt: false },
                "T",
            ),
        };
        KeyBinding {
            modifiers,
            key_name: key_name.into(),
        }
    }

    /// 返回操作的中文显示名称。
    pub fn display_name(&self) -> &'static str {
        match self {
            Action::Save => "保存",
            Action::SaveAs => "另存为",
            Action::NewFile => "新建",
            Action::Open => "打开",
            Action::CloseTab => "关闭标签页",
            Action::NextTab => "下一标签页",
            Action::PrevTab => "上一标签页",
            Action::MoveTabLeft => "标签页左移",
            Action::MoveTabRight => "标签页右移",
            Action::Undo => "撤销",
            Action::Redo => "重做",
            Action::ViewSource => "源码视图",
            Action::ViewPreview => "预览视图",
            Action::ViewHybrid => "Hybrid 视图",
            Action::ToggleTheme => "切换主题",
        }
    }
}

/// 修饰键组合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Modifiers {
    /// 从 egui Modifiers 构建。
    pub fn from_egui(mods: &egui::Modifiers) -> Self {
        Self {
            ctrl: mods.ctrl,
            shift: mods.shift,
            alt: mods.alt,
        }
    }

    /// 转为人类可读字符串，如 "Ctrl+Shift"。
    /// 无修饰键时返回空字符串。
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        parts.join("+")
    }
}

/// 一个快捷键绑定：修饰键 + 按键名。
///
/// `key_name` 存储 egui::Key 的字符串表示，
/// 如 "S", "Tab", "ArrowLeft"。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBinding {
    pub modifiers: Modifiers,
    /// egui::Key 对应的字符串名。
    pub key_name: String,
}

impl KeyBinding {
    /// 格式化显示，如 "Ctrl+Shift+S"。
    pub fn display(&self) -> String {
        let mod_str = self.modifiers.display();
        if mod_str.is_empty() {
            self.key_name.clone()
        } else {
            format!("{mod_str}+{}", self.key_name)
        }
    }
}

/// 快捷键映射表（delta 模式）。
///
/// 仅存储用户覆盖的绑定。未覆盖的 action 使用
/// `Action::default_binding()` 获取默认值。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Keymap {
    /// 用户覆盖的绑定映射。空表示全部使用默认。
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub overrides: HashMap<Action, KeyBinding>,
}

impl Keymap {
    /// 解析 action 的当前生效绑定：
    /// 有 override 返回 override，无则返回默认。
    pub fn resolve(&self, action: &Action) -> KeyBinding {
        self.overrides
            .get(action)
            .cloned()
            .unwrap_or_else(|| action.default_binding())
    }

    /// 设置一个覆盖绑定。
    pub fn set_override(&mut self, action: Action, binding: KeyBinding) {
        self.overrides.insert(action, binding);
    }

    /// 清除单个 action 的覆盖，恢复默认。
    pub fn clear_override(&mut self, action: &Action) {
        self.overrides.remove(action);
    }

    /// 清除所有覆盖，恢复全部默认。
    pub fn clear_all(&mut self) {
        self.overrides.clear();
    }

    /// 检测冲突：返回与给定绑定冲突的 action（存在且非自身）。
    /// 无冲突返回 None。
    pub fn detect_conflict(&self, current_action: &Action, binding: &KeyBinding) -> Option<Action> {
        self.overrides.iter().find_map(|(action, existing)| {
            if action != current_action && existing == binding {
                Some(*action)
            } else {
                None
            }
        })
    }
}
```

- [ ] **步骤 4：运行测试验证通过**

```bash
cargo test -p config -- keybinding
```
预期：全部 15 个测试 PASS。

- [ ] **步骤 5：Commit**

```bash
git add crates/config/src/keybinding.rs
git commit -m "feat(config): add keybinding data model (Action, KeyBinding, Keymap)"
```

---

### 任务 2：集成 Keymap 到 AppConfig

**文件：**
- 修改：`crates/config/src/lib.rs` (在 `mod` 声明处、`AppConfig` 结构体、`Default` impl、测试)

- [ ] **步骤 1：更新 config/src/lib.rs**

在文件顶部 `use` 之后添加模块声明和重新导出：

```rust
// 在现有 use 语句之后，ThemeMode 定义之前插入：
pub mod keybinding;
pub use keybinding::{Action, KeyBinding, Keymap};
```

在 `AppConfig` 结构体中增加 `keymap` 字段（放在 `spell_check_enabled` 之后）：

```rust
/// zdown 应用配置。
///
/// `#[serde(default)]` 确保向后兼容：旧版本配置文件
/// 缺少新增字段时自动使用 Default 值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// 自定义 CSS，追加到 HTML 导出的内置样式之后。
    /// `None` 表示不添加自定义样式。
    pub custom_css: Option<String>,

    /// UI 主题：暗色或亮色。默认暗色。
    pub theme: ThemeMode,
    /// 图片托管配置。
    pub image_hosting: ImageHostingConfig,

    /// 拼写检查开关。默认启用。
    #[serde(default = "default_spell_check")]
    pub spell_check_enabled: bool,

    /// 快捷键映射（delta 模式：仅存储用户覆盖）。
    #[serde(default)]
    pub keymap: Keymap,
}
```

更新 `Default` impl：

```rust
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            custom_css: None,
            theme: ThemeMode::Dark,
            image_hosting: ImageHostingConfig::default(),
            spell_check_enabled: true,
            keymap: Keymap::default(),
        }
    }
}
```

- [ ] **步骤 2：添加集成测试**

在 `crates/config/src/lib.rs` 的 tests 模块末尾添加：

```rust
#[test]
fn keymap_default_is_empty() {
    let config = AppConfig::default();
    assert!(config.keymap.overrides.is_empty());
}

#[test]
fn keymap_roundtrip_with_override() {
    let path = temp_path("keymap");
    cleanup(&path);

    let mut config = AppConfig::default();
    config.keymap.set_override(
        keybinding::Action::Save,
        keybinding::KeyBinding {
            modifiers: keybinding::Modifiers { ctrl: false, shift: false, alt: true },
            key_name: "X".into(),
        },
    );
    config.save_to(&path).expect("save");
    let loaded = AppConfig::load_from(&path).expect("load");
    assert_eq!(
        loaded.keymap.resolve(&keybinding::Action::Save),
        config.keymap.resolve(&keybinding::Action::Save)
    );
    cleanup(&path);
}

#[test]
fn keymap_old_config_without_keymap_defaults_empty() {
    let path = temp_path("old_keymap_config");
    cleanup(&path);
    // 模拟旧版本 TOML（无 keymap 字段）
    std::fs::write(&path, "custom_css = \"h1 { color: red; }\"\n").expect("write");
    let loaded = AppConfig::load_from(&path).expect("load");
    assert!(loaded.keymap.overrides.is_empty());
    cleanup(&path);
}

#[test]
fn keymap_serialize_produces_toml_with_overrides() {
    let mut config = AppConfig::default();
    config.keymap.set_override(
        keybinding::Action::Save,
        keybinding::KeyBinding {
            modifiers: keybinding::Modifiers { ctrl: false, shift: false, alt: true },
            key_name: "X".into(),
        },
    );
    let toml_str = toml::to_string_pretty(&config).expect("serialize");
    // 应包含 keymap 段
    assert!(toml_str.contains("[keymap]") || toml_str.contains("keymap"));
    // 应有 Save 的 override
    assert!(toml_str.contains("Save"));
}
```

- [ ] **步骤 3：运行测试**

```bash
cargo test -p config
```
预期：全部 PASS。

- [ ] **步骤 4：Commit**

```bash
git add crates/config/src/lib.rs
git commit -m "feat(config): integrate Keymap into AppConfig with TOML persistence"
```

---

### 任务 3：重构 handle_shortcuts 为查表驱动

**文件：**
- 修改：`crates/zdown-app/src/menu.rs` (handle_shortcuts 函数, 新增 execute_action, show_menu 签名)
- 修改：`crates/zdown-app/src/main.rs` (移除硬编码 view 快捷键，传递 view_mode/theme 到 handle_shortcuts)

- [ ] **步骤 1：重构 menu.rs handle_shortcuts**

将现有的 `handle_shortcuts` 函数（line 344-421）替换为查表驱动版本：

```rust
/// 处理快捷键（查表驱动）。
pub fn handle_shortcuts(
    ctx: &egui::Context,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
    view_mode: &mut ViewMode,
    theme: &mut ThemeMode,
    app_config: &AppConfig,
) {
    let mods = ctx.input(|i| i.modifiers);

    for action in config::Action::all() {
        let binding = app_config.keymap.resolve(action);
        if !mods_match(&mods, &binding.modifiers) {
            continue;
        }
        if let Some(key) = key_from_name(&binding.key_name) {
            if ctx.input(|i| i.key_pressed(key)) {
                execute_action(action, state, confirm, view_mode, theme, app_config);
            }
        }
    }
}

/// 判断 egui 修饰键是否匹配绑定。
fn mods_match(actual: &egui::Modifiers, expected: &config::Modifiers) -> bool {
    actual.ctrl == expected.ctrl
        && actual.shift == expected.shift
        && actual.alt == expected.alt
}

/// 将 key_name 字符串转换为 egui::Key。
fn key_from_name(name: &str) -> Option<egui::Key> {
    use egui::Key;
    match name {
        "S" => Some(Key::S),
        "N" => Some(Key::N),
        "O" => Some(Key::O),
        "W" => Some(Key::W),
        "Z" => Some(Key::Z),
        "Y" => Some(Key::Y),
        "T" => Some(Key::T),
        "Tab" => Some(Key::Tab),
        "ArrowLeft" => Some(Key::ArrowLeft),
        "ArrowRight" => Some(Key::ArrowRight),
        "Num1" => Some(Key::Num1),
        "Num2" => Some(Key::Num2),
        "Num3" => Some(Key::Num3),
        _ => {
            // 尝试 egui::Key 的字符串解析（自定义键名）
            None
        }
    }
}

/// 根据 action 分发执行具体操作。
fn execute_action(
    action: &config::Action,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
    view_mode: &mut ViewMode,
    theme: &mut ThemeMode,
    app_config: &AppConfig,
) {
    match action {
        config::Action::Save => {
            if state.current_path().is_some() {
                let _ = state.save();
            } else {
                trigger_save_as(state);
            }
            state.run_spell_check(app_config.spell_check_enabled);
        }
        config::Action::SaveAs => {
            trigger_save_as(state);
            state.run_spell_check(app_config.spell_check_enabled);
        }
        config::Action::NewFile => {
            state.new_file();
        }
        config::Action::Open => {
            trigger_open(state);
        }
        config::Action::CloseTab => {
            if state.tab_count() > 1 {
                let idx = state.active_tab_index();
                if state.tab_is_dirty(idx) {
                    confirm.pending = Some(PendingAction::CloseTab(idx));
                } else {
                    let removed = state.close_tab(idx);
                    if !removed {
                        state.new_file();
                    }
                }
            }
        }
        config::Action::NextTab => {
            state.next_tab();
        }
        config::Action::PrevTab => {
            state.prev_tab();
        }
        config::Action::MoveTabLeft => {
            let idx = state.active_tab_index();
            if idx > 0 {
                state.move_tab(idx, idx - 1);
            }
        }
        config::Action::MoveTabRight => {
            let idx = state.active_tab_index();
            state.move_tab(idx, idx + 1);
        }
        config::Action::Undo => {
            let _ = state.undo();
        }
        config::Action::Redo => {
            let _ = state.redo();
        }
        config::Action::ViewSource => {
            *view_mode = ViewMode::Source;
        }
        config::Action::ViewPreview => {
            *view_mode = ViewMode::Preview;
        }
        config::Action::ViewHybrid => {
            *view_mode = ViewMode::Hybrid;
        }
        config::Action::ToggleTheme => {
            *theme = match theme {
                ThemeMode::Dark => ThemeMode::Light,
                ThemeMode::Light => ThemeMode::Dark,
            };
        }
    }
}
```

注意：保留 `trigger_open`、`trigger_save_as` 等私有辅助函数不变。确保 `menu.rs` 顶部引入 `config` 模块类型 (`use config::ThemeMode;` 已存在)。

- [ ] **步骤 2：更新 main.rs — 移除硬编码 view 快捷键，更新 handle_shortcuts 调用**

将 `main.rs` 中的 `handle_shortcuts` 调用（line 146）更新为：

```rust
menu::handle_shortcuts(
    &ctx,
    &mut self.state,
    &mut self.confirm,
    &mut self.view_mode,
    &mut self.theme,
    &self.app_config,
);
```

删除 main.rs 中硬编码的 view 模式快捷键（lines 177-188）：

```rust
// 删除以下代码块：
if mods.ctrl && !mods.shift {
    if ctx.input(|i| i.key_pressed(egui::Key::Num1)) {
        self.view_mode = ViewMode::Source;
        tracing::info!("切换到源码模式");
    } else if ctx.input(|i| i.key_pressed(egui::Key::Num2)) {
        self.view_mode = ViewMode::Preview;
        tracing::info!("切换到预览模式");
    } else if ctx.input(|i| i.key_pressed(egui::Key::Num3)) {
        self.view_mode = ViewMode::Hybrid;
        tracing::info!("切换到 Hybrid 模式");
    }
}
```

- [ ] **步骤 3：编译验证**

```bash
cargo build -p zdown-app
```
预期：编译通过，无 warning（clippy 级别）。

- [ ] **步骤 4：运行 clippy + 测试**

```bash
cargo clippy --all-targets
cargo test -p zdown-app
cargo test -p config
```
预期：clippy clean，全部测试 PASS。

- [ ] **步骤 5：Commit**

```bash
git add crates/zdown-app/src/menu.rs crates/zdown-app/src/main.rs
git commit -m "refactor(menu): replace hardcoded shortcuts with table-driven dispatch"
```

---

### 任务 4：设置对话框新增快捷键标签页

**文件：**
- 修改：`crates/zdown-app/src/settings_dialog.rs`
- 修改：`crates/zdown-app/src/menu.rs` (`show_menu` 中 `open_dialog` 调用传 keymap)

- [ ] **步骤 1：更新 open_dialog 方法签名和实现**

在 `settings_dialog.rs` 中修改 `SettingsDialog` 结构体，增加 keybinding 相关状态字段：

```rust
/// 按键捕获状态。
#[derive(Debug, Clone)]
struct KeybindingCapture {
    /// 正在重新绑定的 action。
    action: config::Action,
    /// 冲突的 action（若有）。
    conflict_with: Option<config::Action>,
}

/// 设置对话框状态。
#[derive(Debug, Clone)]
pub struct SettingsDialog {
    /// 对话框是否打开。
    pub open: bool,
    active_tab: SettingsTab,
    /// 用户正在编辑的 CSS 文本缓冲区。
    css_buffer: String,
    /// 图片设置缓冲区
    local_dir_buffer: String,
    smms_token_buffer: String,
    strategy_buffer: usize, // 0=Local, 1=Base64, 2=SmMs
    /// 拼写检查开关缓冲区。
    spell_check_buffer: bool,
    /// 快捷键映射的可变副本（用于编辑缓冲）。
    keymap_buffer: config::Keymap,
    /// 当前按键捕获状态。
    key_capture: Option<KeybindingCapture>,
}
```

更新 Default：

```rust
impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            active_tab: SettingsTab::Css,
            css_buffer: String::new(),
            local_dir_buffer: "images".to_string(),
            smms_token_buffer: String::new(),
            strategy_buffer: 0,
            spell_check_buffer: true,
            keymap_buffer: config::Keymap::default(),
            key_capture: None,
        }
    }
}
```

更新 tabs 枚举：

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum SettingsTab {
    Css,
    Image,
    Spell,
    Keybind,
}
```

更新 `open_dialog` 方法签名和实现：

```rust
impl SettingsDialog {
    /// 打开对话框，将当前配置填充到编辑缓冲区。
    pub fn open_dialog(
        &mut self,
        current_css: Option<&str>,
        image_config: &config::ImageHostingConfig,
        spell_check_enabled: bool,
        keymap: &config::Keymap,
    ) {
        self.open = true;
        self.active_tab = SettingsTab::Css;
        self.css_buffer = current_css.unwrap_or("").to_string();
        self.local_dir_buffer = image_config.local_dir.clone();
        self.smms_token_buffer = image_config.smms.api_token.clone();
        self.strategy_buffer = match image_config.default_strategy {
            config::ImageStrategy::Local => 0,
            config::ImageStrategy::Base64 => 1,
            config::ImageStrategy::SmMs => 2,
        };
        self.spell_check_buffer = spell_check_enabled;
        self.keymap_buffer = keymap.clone();
        self.key_capture = None;
    }
}
```

- [ ] **步骤 2：实现 key_from_egui 辅助函数和按键捕获逻辑**

在 `settings_dialog.rs` 顶部（impl 块之前）添加：

```rust
// 这些函数在 `pub fn show_settings_dialog` 之外定义

/// 将 egui::Key 转为 key_name 字符串（key_from_name 的逆向）。
fn key_name_from_egui(key: egui::Key) -> Option<String> {
    use egui::Key;
    let name = match key {
        Key::A => "A", Key::B => "B", Key::C => "C", Key::D => "D",
        Key::E => "E", Key::F => "F", Key::G => "G", Key::H => "H",
        Key::I => "I", Key::J => "J", Key::K => "K", Key::L => "L",
        Key::M => "M", Key::N => "N", Key::O => "O", Key::P => "P",
        Key::Q => "Q", Key::R => "R", Key::S => "S", Key::T => "T",
        Key::U => "U", Key::V => "V", Key::W => "W", Key::X => "X",
        Key::Y => "Y", Key::Z => "Z",
        Key::Num0 => "Num0", Key::Num1 => "Num1", Key::Num2 => "Num2",
        Key::Num3 => "Num3", Key::Num4 => "Num4", Key::Num5 => "Num5",
        Key::Num6 => "Num6", Key::Num7 => "Num7", Key::Num8 => "Num8",
        Key::Num9 => "Num9",
        Key::Tab => "Tab",
        Key::Space => "Space",
        Key::Enter => "Enter",
        Key::Backspace => "Backspace",
        Key::Delete => "Delete",
        Key::Escape => "Escape",
        Key::ArrowUp => "ArrowUp",
        Key::ArrowDown => "ArrowDown",
        Key::ArrowLeft => "ArrowLeft",
        Key::ArrowRight => "ArrowRight",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::F1 => "F1", Key::F2 => "F2", Key::F3 => "F3",
        Key::F4 => "F4", Key::F5 => "F5", Key::F6 => "F6",
        Key::F7 => "F7", Key::F8 => "F8", Key::F9 => "F9",
        Key::F10 => "F10", Key::F11 => "F11", Key::F12 => "F12",
        Key::Minus => "Minus",
        Key::Equals => "Equals",
        Key::Comma => "Comma",
        Key::Period => "Period",
        Key::Slash => "Slash",
        Key::Backslash => "Backslash",
        Key::OpenBracket => "OpenBracket",
        Key::CloseBracket => "CloseBracket",
        Key::Semicolon => "Semicolon",
        Key::Quote => "Quote",
        _ => return None,
    };
    Some(name.into())
}

/// 处理按键捕获：在设置对话框快捷键标签页中消费按键事件。
fn handle_keybinding_capture(
    ctx: &egui::Context,
    dialog: &mut SettingsDialog,
) {
    if dialog.key_capture.is_none() {
        return;
    }
    let capture = dialog.key_capture.as_ref().expect("checked is_some");
    let mods = ctx.input(|i| i.modifiers);

    // 需要至少一个修饰键
    if !mods.ctrl && !mods.shift && !mods.alt {
        // 检查 Esc 取消
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            dialog.key_capture = None;
        }
        return;
    }

    // 扫描按键事件
    let events = ctx.input(|i| i.events.clone());
    for event in &events {
        if let egui::Event::Key {
            key,
            pressed: true,
            modifiers: _,
            ..
        } = event
        {
            // 忽略纯修饰键
            if matches!(
                key,
                egui::Key::Shift
                    | egui::Key::Control
                    | egui::Key::Alt
                    | egui::Key::Escape
            ) {
                if *key == egui::Key::Escape {
                    dialog.key_capture = None;
                }
                continue;
            }

            if let Some(key_name) = key_name_from_egui(*key) {
                // 如果没有任何修饰键，忽略（要求至少一个修饰键）
                if !mods.ctrl && !mods.shift && !mods.alt {
                    continue;
                }

                let new_binding = config::KeyBinding {
                    modifiers: config::Modifiers {
                        ctrl: mods.ctrl,
                        shift: mods.shift,
                        alt: mods.alt,
                    },
                    key_name,
                };

                // 冲突检测
                let conflict = dialog.keymap_buffer.detect_conflict(
                    &capture.action,
                    &new_binding,
                );

                dialog.keymap_buffer.set_override(capture.action, new_binding);
                dialog.key_capture = Some(KeybindingCapture {
                    action: capture.action,
                    conflict_with: conflict,
                });
            }
            break;
        }
    }
}
```

- [ ] **步骤 3：更新 show_settings_dialog 渲染逻辑**

在 `show_settings_dialog` 函数中：

1. 在函数开头调用按键捕获处理：

```rust
pub fn show_settings_dialog(
    ctx: &egui::Context,
    app_config: &mut AppConfig,
    dialog: &mut SettingsDialog,
) {
    if !dialog.open {
        return;
    }

    // 处理按键捕获（在渲染前消费按键事件）
    handle_keybinding_capture(ctx, dialog);

    let mut close_this = false;
    let mut new_css = dialog.css_buffer.clone();
    // ... rest
```

2. 在标签栏增加「快捷键」标签：

```rust
ui.horizontal(|ui| {
    ui.selectable_value(&mut dialog.active_tab, SettingsTab::Css, "样式");
    ui.selectable_value(&mut dialog.active_tab, SettingsTab::Image, "图片");
    ui.selectable_value(&mut dialog.active_tab, SettingsTab::Spell, "拼写");
    ui.selectable_value(&mut dialog.active_tab, SettingsTab::Keybind, "快捷键");
});
```

3. 在 `SettingsTab::Spell` 分支之后添加 `SettingsTab::Keybind` 分支：

```rust
SettingsTab::Keybind => {
    ui.horizontal(|ui| {
        ui.label("💡 点击快捷键单元格后按下新组合键，Esc 取消");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("恢复全部默认").clicked() {
                dialog.keymap_buffer.clear_all();
                dialog.key_capture = None;
            }
        });
    });
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .max_height(320.0)
        .show(ui, |ui| {
            egui::Frame::group(ui.style())
                .show(ui, |ui| {
                    egui::Grid::new("keybind_grid")
                        .striped(true)
                        .min_col_width(120.0)
                        .show(ui, |ui| {
                            // 表头
                            ui.label(egui::RichText::new("操作").strong());
                            ui.label(egui::RichText::new("快捷键").strong());
                            ui.label("");
                            ui.end_row();

                            for action in config::Action::all() {
                                let binding = dialog.keymap_buffer.resolve(action);
                                let is_capturing = dialog
                                    .key_capture
                                    .as_ref()
                                    .is_some_and(|c| c.action == *action);

                                ui.label(action.display_name());

                                // 快捷键单元格
                                let cell_text = if is_capturing {
                                    "⏳ 按下新快捷键..."
                                } else {
                                    &binding.display()
                                };

                                let cell_style = if is_capturing {
                                    egui::RichText::new(cell_text)
                                        .color(egui::Color32::from_rgb(100, 200, 255))
                                        .strong()
                                } else if dialog.key_capture.as_ref().is_some_and(|c| {
                                    c.conflict_with.is_some()
                                        && dialog
                                            .keymap_buffer
                                            .resolve(action)
                                            == binding
                                }) {
                                    // 检查此行是否有冲突
                                    let has_conflict = dialog.keymap_buffer.detect_conflict(
                                        action,
                                        &binding,
                                    ).is_some();
                                    if has_conflict {
                                        egui::RichText::new(format!("⚠ {}", cell_text))
                                            .color(egui::Color32::RED)
                                    } else {
                                        egui::RichText::new(cell_text)
                                            .monospace()
                                    }
                                } else {
                                    egui::RichText::new(cell_text)
                                        .monospace()
                                };

                                let cell_response = ui.add(
                                    egui::Button::new(cell_style)
                                        .min_size(egui::vec2(160.0, 0.0))
                                );

                                if cell_response.clicked() {
                                    dialog.key_capture = Some(KeybindingCapture {
                                        action: *action,
                                        conflict_with: None,
                                    });
                                }

                                // 恢复按钮
                                let is_modified = dialog.keymap_buffer.overrides.contains_key(action);
                                if is_modified {
                                    if ui.button("↺").on_hover_text("恢复默认").clicked() {
                                        dialog.keymap_buffer.clear_override(action);
                                        if is_capturing {
                                            dialog.key_capture = None;
                                        }
                                    }
                                } else {
                                    ui.label("");
                                }

                                ui.end_row();
                            }
                        });
                });
        });
}
```

4. 在保存按钮中增加 keymap 写入：

```rust
if ui.button("保存").clicked() {
    // CSS 设置
    app_config.custom_css = if new_css.trim().is_empty() {
        None
    } else {
        Some(new_css.clone())
    };
    // 图片设置
    app_config.image_hosting.default_strategy = match dialog.strategy_buffer {
        1 => config::ImageStrategy::Base64,
        2 => config::ImageStrategy::SmMs,
        _ => config::ImageStrategy::Local,
    };
    app_config.image_hosting.local_dir = dialog.local_dir_buffer.clone();
    app_config.image_hosting.smms.api_token = dialog.smms_token_buffer.clone();

    // 拼写检查设置
    app_config.spell_check_enabled = dialog.spell_check_buffer;

    // 快捷键设置
    app_config.keymap = dialog.keymap_buffer.clone();

    if let Err(e) = app_config.save() {
        tracing::error!("配置保存失败: {e}");
    } else {
        tracing::info!("配置已保存");
    }
    close_this = true;
}
```

- [ ] **步骤 4：更新 menu.rs 中 open_dialog 调用**

在 `menu.rs` 的 `show_menu` 函数中（line 100-106），更新 `open_dialog` 调用：

```rust
if ui.button("设置...").clicked() {
    settings_dialog.open_dialog(
        app_config.custom_css.as_deref(),
        &app_config.image_hosting,
        app_config.spell_check_enabled,
        &app_config.keymap,
    );
    ui.close();
}
```

- [ ] **步骤 5：编译验证**

```bash
cargo build -p zdown-app
```
预期：编译通过。

- [ ] **步骤 6：更新测试 + 运行**

更新 `settings_dialog.rs` 测试中的 `open_dialog` 调用（传入 `&Keymap::default()`）：

```rust
#[test]
fn open_populates_buffer() {
    let mut dialog = SettingsDialog::default();
    dialog.open_dialog(Some("h1{color:red}"), &Default::default(), true, &config::Keymap::default());
    assert!(dialog.open);
    assert_eq!(dialog.css_buffer, "h1{color:red}");
    assert_eq!(dialog.local_dir_buffer, "images");
}

#[test]
fn open_with_none_sets_empty_buffer() {
    let mut dialog = SettingsDialog::default();
    dialog.open_dialog(None, &Default::default(), true, &config::Keymap::default());
    assert!(dialog.open);
    assert_eq!(dialog.css_buffer, "");
    assert_eq!(dialog.local_dir_buffer, "images");
}

#[test]
fn default_dialog_is_closed() {
    let dialog = SettingsDialog::default();
    assert!(!dialog.open);
    assert!(dialog.keymap_buffer.overrides.is_empty());
}

#[test]
fn open_dialog_populates_keymap() {
    let mut dialog = SettingsDialog::default();
    let mut keymap = config::Keymap::default();
    keymap.set_override(
        config::Action::Save,
        config::KeyBinding {
            modifiers: config::Modifiers { ctrl: false, shift: false, alt: true },
            key_name: "X".into(),
        },
    );
    dialog.open_dialog(None, &Default::default(), true, &keymap);
    assert!(dialog.open);
    assert_eq!(
        dialog.keymap_buffer.resolve(&config::Action::Save).key_name,
        "X"
    );
}
```

```bash
cargo clippy --all-targets
cargo test -p zdown-app
cargo test -p config
```
预期：clippy clean，全部测试 PASS。

- [ ] **步骤 7：Commit**

```bash
git add crates/zdown-app/src/settings_dialog.rs crates/zdown-app/src/menu.rs
git commit -m "feat(settings): add keybinding configuration tab with key capture"
```

---

## 验证检查清单

- [ ] `cargo fmt` 通过
- [ ] `cargo clippy --all-targets` 无 warning
- [ ] `cargo test` 全部通过
- [ ] `cargo build --release` 成功
