//! 键盘/鼠标绑定映射。
//!
//! 将 egui 键盘/鼠标事件转换为 ANSI/VT 转义序列或控制字符。

use alacritty_terminal::term::TermMode;
use egui::{Key, Modifiers, PointerButton};

/// 绑定动作。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingAction {
    Copy,
    Paste,
    Char(char),
    Esc(String),
    LinkOpen,
    Ignore,
}

/// 输入来源类型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputKind {
    KeyCode(Key),
    Mouse(PointerButton),
    Char(String),
}

/// 按键/鼠标绑定定义。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding<T> {
    pub target: T,
    pub modifiers: Modifiers,
    pub terminal_mode_include: TermMode,
    pub terminal_mode_exclude: TermMode,
}

pub type KeyboardBinding = Binding<InputKind>;

/// generate_bindings! 宏 — 声明式定义按键绑定表。
macro_rules! generate_bindings {
    (
        $binding_type:ident;
        $(
            $target:ident
            $(,$input_modifiers:expr)*
            $(,+$terminal_mode_include:expr)*
            $(,~$terminal_mode_exclude:expr)*
            ;$action:expr
        );*
        $(;)*
    ) => {{
        let mut v = Vec::new();
        $(
            let mut _mods = Modifiers::NONE;
            $(_mods = $input_modifiers;)*
            let mut _mode_include = TermMode::empty();
            $(_mode_include.insert($terminal_mode_include);)*
            let mut _mode_exclude = TermMode::empty();
            $(_mode_exclude.insert($terminal_mode_exclude);)*

            let binding = $binding_type {
                target: InputKind::KeyCode(Key::$target),
                modifiers: _mods,
                terminal_mode_include: _mode_include,
                terminal_mode_exclude: _mode_exclude,
            };

            v.push((binding, $action.into()));
        )*
        v
    }};
}

/// 按键绑定布局（有序列表，第一个匹配生效）。
#[derive(Clone, Debug)]
pub struct BindingsLayout {
    bindings: Vec<(Binding<InputKind>, BindingAction)>,
}

impl Default for BindingsLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl BindingsLayout {
    pub fn new() -> Self {
        let bindings = default_keyboard_bindings();
        Self { bindings }
    }

    /// 追加绑定（重复项替换为最后一个）。
    pub fn add_bindings(
        &mut self,
        additions: Vec<(Binding<InputKind>, BindingAction)>,
    ) {
        for (binding, action) in additions {
            if let Some(pos) =
                self.bindings.iter().position(|(b, _)| *b == binding)
            {
                self.bindings[pos] = (binding, action);
            } else {
                self.bindings.push((binding, action));
            }
        }
    }

    /// 查找匹配的绑定动作。
    pub fn get_action(
        &self,
        input: InputKind,
        modifiers: Modifiers,
        terminal_mode: TermMode,
    ) -> BindingAction {
        for (binding, action) in &self.bindings {
            if binding.target == input
                && modifiers.matches_exact(binding.modifiers)
                && terminal_mode.contains(binding.terminal_mode_include)
                && !terminal_mode.intersects(binding.terminal_mode_exclude)
            {
                return action.clone();
            }
        }
        BindingAction::Ignore
    }
}

fn default_keyboard_bindings() -> Vec<(Binding<InputKind>, BindingAction)> {
    // egui 0.34 没有 Modifiers::CTRL 常量，手动定义
    const CTRL: Modifiers = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    generate_bindings!(
        KeyboardBinding;
        Enter;     BindingAction::Char('\x0d');
        Backspace; BindingAction::Char('\x7f');
        Escape;    BindingAction::Char('\x1b');
        Tab;       BindingAction::Char('\x09');
        Insert;    BindingAction::Esc("\x1b[2~".into());
        Delete;    BindingAction::Esc("\x1b[3~".into());
        PageUp;    BindingAction::Esc("\x1b[5~".into());
        PageDown;  BindingAction::Esc("\x1b[6~".into());
        F1;        BindingAction::Esc("\x1bOP".into());
        F2;        BindingAction::Esc("\x1bOQ".into());
        F3;        BindingAction::Esc("\x1bOR".into());
        F4;        BindingAction::Esc("\x1bOS".into());
        F5;        BindingAction::Esc("\x1b[15~".into());
        F6;        BindingAction::Esc("\x1b[17~".into());
        F7;        BindingAction::Esc("\x1b[18~".into());
        F8;        BindingAction::Esc("\x1b[19~".into());
        F9;        BindingAction::Esc("\x1b[20~".into());
        F10;       BindingAction::Esc("\x1b[21~".into());
        F11;       BindingAction::Esc("\x1b[23~".into());
        F12;       BindingAction::Esc("\x1b[24~".into());
        // APP_CURSOR 排除
        End,        ~TermMode::APP_CURSOR; BindingAction::Esc("\x1b[F".into());
        Home,       ~TermMode::APP_CURSOR; BindingAction::Esc("\x1b[H".into());
        ArrowUp,    ~TermMode::APP_CURSOR; BindingAction::Esc("\x1b[A".into());
        ArrowDown,  ~TermMode::APP_CURSOR; BindingAction::Esc("\x1b[B".into());
        ArrowLeft,  ~TermMode::APP_CURSOR; BindingAction::Esc("\x1b[D".into());
        ArrowRight, ~TermMode::APP_CURSOR; BindingAction::Esc("\x1b[C".into());
        // APP_CURSOR 包含
        End,        +TermMode::APP_CURSOR; BindingAction::Esc("\x1bOF".into());
        Home,       +TermMode::APP_CURSOR; BindingAction::Esc("\x1BOH".into());
        ArrowUp,    +TermMode::APP_CURSOR; BindingAction::Esc("\x1bOA".into());
        ArrowDown,  +TermMode::APP_CURSOR; BindingAction::Esc("\x1bOB".into());
        ArrowLeft,  +TermMode::APP_CURSOR; BindingAction::Esc("\x1bOD".into());
        ArrowRight, +TermMode::APP_CURSOR; BindingAction::Esc("\x1bOC".into());
        // Ctrl 组合
        ArrowUp,    CTRL; BindingAction::Esc("\x1b[1;5A".into());
        ArrowDown,  CTRL; BindingAction::Esc("\x1b[1;5B".into());
        ArrowLeft,  CTRL; BindingAction::Esc("\x1b[1;5D".into());
        ArrowRight, CTRL; BindingAction::Esc("\x1b[1;5C".into());
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Key, Modifiers};
    use alacritty_terminal::term::TermMode;

    #[test]
    fn enter_maps_to_cr() {
        let layout = BindingsLayout::new();
        let action = layout.get_action(
            InputKind::KeyCode(Key::Enter),
            Modifiers::NONE,
            TermMode::empty(),
        );
        assert_eq!(action, BindingAction::Char('\x0d'));
    }

    #[test]
    fn escape_maps_to_esc() {
        let layout = BindingsLayout::new();
        let action = layout.get_action(
            InputKind::KeyCode(Key::Escape),
            Modifiers::NONE,
            TermMode::empty(),
        );
        assert_eq!(action, BindingAction::Char('\x1b'));
    }

    #[test]
    fn backspace_maps_to_del() {
        let layout = BindingsLayout::new();
        let action = layout.get_action(
            InputKind::KeyCode(Key::Backspace),
            Modifiers::NONE,
            TermMode::empty(),
        );
        assert_eq!(action, BindingAction::Char('\x7f'));
    }

    #[test]
    fn arrow_up_app_cursor_mode() {
        let layout = BindingsLayout::new();
        let action = layout.get_action(
            InputKind::KeyCode(Key::ArrowUp),
            Modifiers::NONE,
            TermMode::APP_CURSOR,
        );
        assert_eq!(action, BindingAction::Esc("\x1bOA".into()));
    }

    #[test]
    fn ctrl_modifier_matches() {
        let mods = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        assert!(mods.ctrl);
        assert!(!mods.shift);
    }

    #[test]
    fn arrow_up_normal_mode() {
        let layout = BindingsLayout::new();
        let action = layout.get_action(
            InputKind::KeyCode(Key::ArrowUp),
            Modifiers::NONE,
            TermMode::empty(),
        );
        assert_eq!(action, BindingAction::Esc("\x1b[A".into()));
    }

    #[test]
    fn unknown_key_returns_ignore() {
        let layout = BindingsLayout::new();
        let action = layout.get_action(
            InputKind::KeyCode(Key::F20),
            Modifiers::NONE,
            TermMode::empty(),
        );
        assert_eq!(action, BindingAction::Ignore);
    }
}
