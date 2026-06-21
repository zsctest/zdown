//! 终端面板集成测试。
//!
//! 验证各模块间的协作：字体度量 + 尺寸计算 + 主题颜色 + 按键绑定。

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use egui::{Key, Modifiers};
use terminal_panel::backend::{Size, TerminalSize, compute_size};
use terminal_panel::bindings::{BindingAction, BindingsLayout, InputKind};
use terminal_panel::theme::TerminalTheme;

#[test]
fn compute_size_maps_pixels_to_cols_lines() {
    // 800x400 layout, 10x18 font → 80 cols, 22 lines
    let size = compute_size(Size::new(800.0, 400.0), Size::new(10.0, 18.0));
    assert_eq!(size.num_cols, 80);
    assert_eq!(size.num_lines, 22);
    assert_eq!(size.cell_width, 10);
    assert_eq!(size.cell_height, 18);
}

#[test]
fn compute_size_handles_zero_input() {
    let size = compute_size(Size::new(0.0, 0.0), Size::new(10.0, 18.0));
    assert_eq!(size.num_cols, 80); // returns default
    assert_eq!(size.num_lines, 50); // returns default
}

#[test]
fn terminal_size_implements_dimensions() {
    let size = TerminalSize::default();
    assert_eq!(size.columns(), 80);
    assert_eq!(size.screen_lines(), 50);
    assert_eq!(size.total_lines(), 50);
}

#[test]
fn bindings_enter_and_backspace() {
    let layout = BindingsLayout::new();
    // Enter → \x0d
    assert_eq!(
        layout.get_action(
            InputKind::KeyCode(Key::Enter),
            Modifiers::NONE,
            TermMode::empty(),
        ),
        BindingAction::Char('\x0d')
    );
    // Backspace → \x7f
    assert_eq!(
        layout.get_action(
            InputKind::KeyCode(Key::Backspace),
            Modifiers::NONE,
            TermMode::empty(),
        ),
        BindingAction::Char('\x7f')
    );
}

#[test]
fn bindings_arrows_normal_vs_app_cursor() {
    let layout = BindingsLayout::new();
    // Normal mode
    assert_eq!(
        layout.get_action(
            InputKind::KeyCode(Key::ArrowUp),
            Modifiers::NONE,
            TermMode::empty(),
        ),
        BindingAction::Esc("\x1b[A".into())
    );
    // App cursor mode
    assert_eq!(
        layout.get_action(
            InputKind::KeyCode(Key::ArrowUp),
            Modifiers::NONE,
            TermMode::APP_CURSOR,
        ),
        BindingAction::Esc("\x1bOA".into())
    );
}

#[test]
fn theme_ansi_colors_are_consistent() {
    let theme = TerminalTheme::default();
    let fg = theme.get_color(Color::Named(NamedColor::Foreground));
    let bg = theme.get_color(Color::Named(NamedColor::Background));
    // Foreground should be light, background should be dark
    assert_ne!(fg, bg);
    // Both should be non-zero
    assert_ne!(bg, egui::Color32::TRANSPARENT);
    assert_ne!(fg, egui::Color32::TRANSPARENT);
}

#[test]
fn theme_monokai_has_distinct_colors() {
    let monokai = TerminalTheme::monokai();
    let red = monokai.get_color(Color::Named(NamedColor::Red));
    let green = monokai.get_color(Color::Named(NamedColor::Green));
    let blue = monokai.get_color(Color::Named(NamedColor::Blue));
    assert_ne!(red, green);
    assert_ne!(green, blue);
    assert_ne!(blue, red);
}

#[test]
fn theme_ansi256_index_bounds() {
    let theme = TerminalTheme::default();
    // Index 0 (black)
    let c0 = theme.get_color(Color::Indexed(0));
    assert!(c0 != egui::Color32::TRANSPARENT);
    // Index 15 (bright white)
    let c15 = theme.get_color(Color::Indexed(15));
    assert!(c15 != egui::Color32::TRANSPARENT);
    // Index 255 (grayscale max)
    let c255 = theme.get_color(Color::Indexed(255));
    assert!(c255 != egui::Color32::TRANSPARENT);
}

#[test]
fn bindings_ctrl_arrow_combos() {
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    let layout = BindingsLayout::new();
    let action = layout.get_action(InputKind::KeyCode(Key::ArrowUp), ctrl, TermMode::empty());
    assert_eq!(action, BindingAction::Esc("\x1b[1;5A".into()));
}
