//! egui 0.34 终端网格渲染。
//!
//! 将 alacritty_terminal 的 Grid<Cell> 逐格渲染为 egui 原生图形元素。
//! 适配 egui 0.34: 使用 `Painter::text()` 和 `Painter::rect_filled()`。

use alacritty_terminal::term::cell;
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use egui::{Modifiers, Pos2, Rect, Response, Vec2, Widget};

use crate::backend::{BackendCommand, Size, TerminalBackend};
use crate::bindings::{BindingAction, BindingsLayout, InputKind};
use crate::font::TerminalFont;
use crate::theme::TerminalTheme;

/// 终端视图内部状态（持久化在 egui memory 中）。
#[derive(Clone, Default)]
pub struct TerminalViewState {
    pub is_dragged: bool,
    pub scroll_pixels: f32,
}

/// 内部动作分发。
enum InputAction {
    BackendCmd(BackendCommand),
    Clipboard(String),
}

/// 终端视图 Widget。
///
/// 实现 egui::Widget trait，可被 `ui.add()` 使用。
/// 持有 `&mut TerminalBackend` 以便处理输入和同步终端状态。
pub struct TerminalView<'a> {
    id: egui::Id,
    backend: &'a mut TerminalBackend,
    font: TerminalFont,
    theme: TerminalTheme,
    bindings: BindingsLayout,
    available_size: Vec2,
}

impl<'a> TerminalView<'a> {
    pub fn new(
        ui: &mut egui::Ui,
        backend: &'a mut TerminalBackend,
        font: TerminalFont,
        theme: TerminalTheme,
    ) -> Self {
        let id = ui.make_persistent_id("terminal_view");
        let available = ui.available_size();
        Self {
            id,
            backend,
            font,
            theme,
            bindings: BindingsLayout::new(),
            available_size: available,
        }
    }

    /// 处理 egui 事件，转换为终端命令。
    fn process_event(
        &self,
        event: &egui::Event,
        modifiers: &Modifiers,
        response: &Response,
    ) -> Vec<InputAction> {
        match event {
            egui::Event::Text(text) => {
                vec![InputAction::BackendCmd(BackendCommand::Write(
                    text.as_bytes().to_vec(),
                ))]
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers: key_mods,
                ..
            } => {
                let combined = Modifiers {
                    ctrl: modifiers.ctrl || key_mods.ctrl,
                    shift: modifiers.shift || key_mods.shift,
                    alt: modifiers.alt || key_mods.alt,
                    mac_cmd: modifiers.mac_cmd || key_mods.mac_cmd,
                    ..Default::default()
                };
                let action = self.bindings.get_action(
                    InputKind::KeyCode(*key),
                    combined,
                    self.backend.last_content().terminal_mode,
                );
                match action {
                    BindingAction::Char(c) => {
                        let mut buf = [0u8; 4];
                        let s = c.encode_utf8(&mut buf);
                        vec![InputAction::BackendCmd(BackendCommand::Write(
                            s.as_bytes().to_vec(),
                        ))]
                    }
                    BindingAction::Esc(seq) => {
                        vec![InputAction::BackendCmd(BackendCommand::Write(
                            seq.as_bytes().to_vec(),
                        ))]
                    }
                    BindingAction::Copy => {
                        let text = self.backend.selectable_content();
                        vec![InputAction::Clipboard(text)]
                    }
                    _ => vec![],
                }
            }
            egui::Event::PointerButton {
                button: egui::PointerButton::Primary,
                pressed: true,
                pos,
                modifiers,
            } => {
                let rel = *pos - response.rect.min;
                if modifiers.ctrl || modifiers.mac_cmd {
                    // Ctrl+Click → 超链接打开（暂不处理）
                    let content = self.backend.last_content();
                    let _point = TerminalBackend::selection_point(
                        rel.x,
                        rel.y,
                        &content.terminal_size,
                        content.display_offset(),
                    );
                    vec![InputAction::BackendCmd(BackendCommand::ProcessLink(_point))]
                } else {
                    vec![InputAction::BackendCmd(BackendCommand::SelectStart(
                        alacritty_terminal::selection::SelectionType::Simple,
                        rel.x,
                        rel.y,
                    ))]
                }
            }
            egui::Event::PointerMoved(pos) => {
                let rel = *pos - response.rect.min;
                vec![InputAction::BackendCmd(BackendCommand::SelectUpdate(
                    rel.x, rel.y,
                ))]
            }
            egui::Event::MouseWheel { delta, .. } => {
                let lines = (delta.y * 3.0) as i32;
                vec![InputAction::BackendCmd(BackendCommand::Scroll(lines))]
            }
            _ => vec![],
        }
    }

    /// 渲染终端网格到 egui painter。
    fn render(&mut self, ui: &mut egui::Ui, response: &Response) {
        // 先同步终端状态
        let content = self.backend.sync();

        let painter = ui.painter();
        let layout_min = response.rect.min;
        let cell_width = content.terminal_size.cell_width as f32;
        let cell_height = content.terminal_size.cell_height as f32;

        if cell_width <= 0.0 || cell_height <= 0.0 {
            return;
        }

        // 背景
        let default_bg = self.theme.get_color(Color::Named(NamedColor::Background));
        painter.rect_filled(response.rect, egui::CornerRadius::ZERO, default_bg);

        let font_id = self.font.font_type();
        let display_offset = content.display_offset();

        for indexed in content.grid.display_iter() {
            let flags = indexed.cell.flags;
            if flags.contains(cell::Flags::WIDE_CHAR_SPACER) {
                continue;
            }

            let is_wide = flags.contains(cell::Flags::WIDE_CHAR);
            let is_inverse = flags.contains(cell::Flags::INVERSE);
            let is_dim = flags.intersects(cell::Flags::DIM | cell::Flags::DIM_BOLD);
            let is_selected = content
                .selectable_range
                .as_ref()
                .is_some_and(|r| r.contains(indexed.point));

            let col = indexed.point.column.0 as f32;
            let line = indexed.point.line.0 + display_offset as i32;

            let x = layout_min.x + cell_width * col;
            let y = layout_min.y + cell_height * line as f32;

            let mut fg = self.theme.get_color(indexed.fg);
            let mut bg = self.theme.get_color(indexed.bg);

            if is_dim {
                fg = fg.linear_multiply(0.7);
            }
            if is_inverse || is_selected {
                std::mem::swap(&mut fg, &mut bg);
            }

            let cell_w = if is_wide {
                cell_width * 2.0
            } else {
                cell_width
            };

            // 非默认背景才绘制背景矩形
            if bg != default_bg {
                painter.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(x, y),
                        Vec2::new(cell_w + 1.0, cell_height + 1.0),
                    ),
                    egui::CornerRadius::ZERO,
                    bg,
                );
            }

            // 光标
            if content.cursor_point == indexed.point {
                let cursor_color = self.theme.get_color(content.cursor.fg);
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(x, y), Vec2::new(cell_w.max(1.0), cell_height)),
                    egui::CornerRadius::ZERO,
                    cursor_color,
                );
            }

            // 绘制字符（跳过空格和制表符）
            if indexed.c != ' ' && indexed.c != '\t' && indexed.c != '\u{00a0}' {
                // 在 cell 中水平居中
                let text_x = x + cell_w / 2.0;
                painter.text(
                    Pos2::new(text_x, y),
                    egui::Align2::CENTER_TOP,
                    indexed.c,
                    font_id.clone(),
                    fg,
                );
            }
        }
    }
}

impl Widget for TerminalView<'_> {
    fn ui(mut self, ui: &mut egui::Ui) -> Response {
        let (rect, response) =
            ui.allocate_exact_size(self.available_size, egui::Sense::click_and_drag());

        let state = ui.memory_mut(|m| {
            m.data
                .get_temp::<TerminalViewState>(self.id)
                .unwrap_or_default()
        });

        // 处理 resize
        let font_size = self.font.cell_size(ui.ctx());
        self.backend.process_command(BackendCommand::Resize(
            Size::new(rect.width(), rect.height()),
            Size::new(font_size.0, font_size.1),
        ));

        // 处理输入
        if response.has_focus() {
            let events = ui.input(|i| i.events.clone());
            let modifiers = ui.input(|i| i.modifiers);
            for event in &events {
                let actions = self.process_event(event, &modifiers, &response);
                for action in actions {
                    match action {
                        InputAction::BackendCmd(cmd) => {
                            self.backend.process_command(cmd);
                        }
                        InputAction::Clipboard(text) => {
                            ui.ctx().copy_text(text);
                        }
                    }
                }
            }
        }

        // 渲染
        self.render(ui, &response);

        ui.memory_mut(|m| m.data.insert_temp(self.id, state));
        response
    }
}
