//! 键盘快捷键绑定数据模型。
//!
//! 定义所有可配置的编辑器操作及其默认/自定义快捷键绑定。
//! 支持 delta 模式（仅存储与默认不同的覆盖）。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 所有可被快捷键触发的编辑器动作。
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
    /// 返回所有动作变体。
    pub fn all() -> &'static [Self] {
        &[
            Self::Save,
            Self::SaveAs,
            Self::NewFile,
            Self::Open,
            Self::CloseTab,
            Self::NextTab,
            Self::PrevTab,
            Self::MoveTabLeft,
            Self::MoveTabRight,
            Self::Undo,
            Self::Redo,
            Self::ViewSource,
            Self::ViewPreview,
            Self::ViewHybrid,
            Self::ToggleTheme,
        ]
    }

    /// 返回此动作的默认快捷键绑定。
    pub fn default_binding(&self) -> KeyBinding {
        match self {
            Self::Save => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
                key_name: "S".into(),
            },
            Self::SaveAs => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: true,
                    alt: false,
                },
                key_name: "S".into(),
            },
            Self::NewFile => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
                key_name: "N".into(),
            },
            Self::Open => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
                key_name: "O".into(),
            },
            Self::CloseTab => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
                key_name: "W".into(),
            },
            Self::NextTab => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
                key_name: "Tab".into(),
            },
            Self::PrevTab => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: true,
                    alt: false,
                },
                key_name: "Tab".into(),
            },
            Self::MoveTabLeft => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: true,
                    alt: false,
                },
                key_name: "ArrowLeft".into(),
            },
            Self::MoveTabRight => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: true,
                    alt: false,
                },
                key_name: "ArrowRight".into(),
            },
            Self::Undo => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
                key_name: "Z".into(),
            },
            Self::Redo => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
                key_name: "Y".into(),
            },
            Self::ViewSource => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
                key_name: "Num1".into(),
            },
            Self::ViewPreview => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
                key_name: "Num2".into(),
            },
            Self::ViewHybrid => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
                key_name: "Num3".into(),
            },
            Self::ToggleTheme => KeyBinding {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
                key_name: "T".into(),
            },
        }
    }

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
}

/// 修饰键组合。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Modifiers {
    /// 返回用户可读的修饰键组合字符串（如 "Ctrl+Shift"）。
    pub fn display(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
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

/// 快捷键绑定（修饰键 + 主键）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBinding {
    pub modifiers: Modifiers,
    pub key_name: String,
}

impl KeyBinding {
    /// 返回用户可读的快捷键字符串（如 "Ctrl+S"）。
    pub fn display(&self) -> String {
        let prefix = self.modifiers.display();
        if prefix.is_empty() {
            self.key_name.clone()
        } else {
            format!("{}+{}", prefix, self.key_name)
        }
    }
}

/// 快捷键映射表。
///
/// 使用 delta 模式：仅存储与默认不同的覆盖绑定。
/// `resolve()` 优先返回覆盖值，否则返回默认绑定。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Keymap {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub overrides: HashMap<Action, KeyBinding>,
}

impl Keymap {
    /// 解析指定动作的快捷键绑定（优先返回覆盖值）。
    pub fn resolve(&self, action: &Action) -> KeyBinding {
        self.overrides
            .get(action)
            .cloned()
            .unwrap_or_else(|| action.default_binding())
    }

    /// 设置指定动作的自定义快捷键绑定。
    pub fn set_override(&mut self, action: Action, binding: KeyBinding) {
        self.overrides.insert(action, binding);
    }

    /// 清除指定动作的自定义覆盖，恢复默认绑定。
    pub fn clear_override(&mut self, action: &Action) {
        self.overrides.remove(action);
    }

    /// 清除所有自定义覆盖。
    pub fn clear_all(&mut self) {
        self.overrides.clear();
    }

    /// 检测 `new_binding` 是否与 `action` 之外的某个动作冲突。
    ///
    /// 冲突来源：
    /// - 其他动作已经设置了与 `new_binding` 相同的自定义覆盖。
    /// - 其他动作的默认绑定与 `new_binding` 相同且该动作没有自定义覆盖。
    ///
    /// 返回第一个冲突的动作（如果有）。
    pub fn detect_conflict(&self, action: &Action, new_binding: &KeyBinding) -> Option<Action> {
        // 先检查其他动作的自定义覆盖是否有冲突
        if let Some(conflict) = self
            .overrides
            .iter()
            .filter(|(a, _)| *a != action)
            .find(|(_, b)| *b == new_binding)
            .map(|(a, _)| *a)
        {
            return Some(conflict);
        }
        // 再检查其他动作的默认绑定（不含覆盖）是否有冲突
        Action::all()
            .iter()
            .filter(|a| *a != action)
            .find(|a| {
                let default = a.default_binding();
                &default == new_binding && !self.overrides.contains_key(a)
            })
            .copied()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn every_action_has_default_binding() {
        for action in Action::all() {
            let binding = action.default_binding();
            assert!(
                !binding.key_name.is_empty(),
                "{action:?} 默认绑定 key_name 为空"
            );
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
            modifiers: Modifiers {
                ctrl: false,
                shift: false,
                alt: true,
            },
            key_name: "X".into(),
        };
        keymap.set_override(Action::Save, custom.clone());
        assert_eq!(keymap.resolve(&Action::Save), custom);
    }

    #[test]
    fn keymap_clear_override_restores_default() {
        let mut keymap = Keymap::default();
        let custom = KeyBinding {
            modifiers: Modifiers {
                ctrl: false,
                shift: false,
                alt: true,
            },
            key_name: "X".into(),
        };
        keymap.set_override(Action::Save, custom);
        keymap.clear_override(&Action::Save);
        assert_eq!(
            keymap.resolve(&Action::Save),
            Action::Save.default_binding()
        );
    }

    #[test]
    fn keymap_detect_conflict_returns_conflicting_action() {
        let mut keymap = Keymap::default();
        let binding = Action::Save.default_binding(); // Ctrl+S
        // 把 SaveAs 也改成 Ctrl+S 模拟冲突
        keymap.set_override(Action::SaveAs, binding.clone());
        // detect_conflict(当前action, 新binding)
        assert_eq!(
            keymap.detect_conflict(&Action::Save, &binding),
            Some(Action::SaveAs)
        );
    }

    #[test]
    fn keymap_detect_conflict_same_action_not_conflict() {
        let mut keymap = Keymap::default();
        let custom = KeyBinding {
            modifiers: Modifiers {
                ctrl: false,
                shift: false,
                alt: true,
            },
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
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                alt: true,
            },
            key_name: "Q".into(),
        };
        assert_eq!(keymap.detect_conflict(&Action::Save, &unique), None);
    }

    #[test]
    fn keymap_serialize_roundtrip() {
        let mut keymap = Keymap::default();
        keymap.set_override(
            Action::Save,
            KeyBinding {
                modifiers: Modifiers {
                    ctrl: false,
                    shift: false,
                    alt: true,
                },
                key_name: "X".into(),
            },
        );
        let toml_str = toml::to_string_pretty(&keymap).expect("serialize");
        let restored: Keymap = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(
            restored.resolve(&Action::Save),
            keymap.resolve(&Action::Save)
        );
        // 未覆盖的 action 走默认
        assert_eq!(
            restored.resolve(&Action::Open),
            Action::Open.default_binding()
        );
    }

    #[test]
    fn keymap_default_is_empty_overrides() {
        let keymap = Keymap::default();
        assert!(keymap.overrides.is_empty());
    }

    #[test]
    fn keymap_clear_all_removes_all_overrides() {
        let mut keymap = Keymap::default();
        keymap.set_override(
            Action::Save,
            KeyBinding {
                modifiers: Modifiers {
                    ctrl: false,
                    shift: false,
                    alt: true,
                },
                key_name: "X".into(),
            },
        );
        keymap.set_override(
            Action::Open,
            KeyBinding {
                modifiers: Modifiers {
                    ctrl: false,
                    shift: false,
                    alt: true,
                },
                key_name: "O".into(),
            },
        );
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
            assert!(!name.is_empty(), "{action:?} display_name is empty");
        }
    }

    #[test]
    fn display_name_returns_ftl_keys() {
        assert_eq!(Action::Save.display_name(), "action-save");
        assert_eq!(Action::Undo.display_name(), "action-undo");
        // 确保所有 action 都有对应的 FTL key
        for action in Action::all() {
            let key = action.display_name();
            assert!(
                key.starts_with("action-"),
                "Key {key} should start with 'action-'"
            );
        }
    }

    #[test]
    fn modifiers_display_formats_correctly() {
        let m = Modifiers {
            ctrl: true,
            shift: false,
            alt: false,
        };
        assert_eq!(m.display(), "Ctrl");
        let m = Modifiers {
            ctrl: true,
            shift: true,
            alt: false,
        };
        assert_eq!(m.display(), "Ctrl+Shift");
        let m = Modifiers {
            ctrl: true,
            shift: false,
            alt: true,
        };
        assert_eq!(m.display(), "Ctrl+Alt");
        let m = Modifiers {
            ctrl: true,
            shift: true,
            alt: true,
        };
        assert_eq!(m.display(), "Ctrl+Shift+Alt");
        let m = Modifiers {
            ctrl: false,
            shift: false,
            alt: false,
        };
        assert_eq!(m.display(), "");
    }

    #[test]
    fn keybinding_display_formats_correctly() {
        let kb = KeyBinding {
            modifiers: Modifiers {
                ctrl: true,
                shift: false,
                alt: false,
            },
            key_name: "S".into(),
        };
        assert_eq!(kb.display(), "Ctrl+S");
        let kb = KeyBinding {
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                alt: false,
            },
            key_name: "Tab".into(),
        };
        assert_eq!(kb.display(), "Ctrl+Shift+Tab");
    }
}
