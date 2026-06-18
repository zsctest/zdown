//! 源码编辑视图。
//!
//! 阶段 1（路径 B）：TextEdit::multiline 单色编辑 + 行号显示。
//! 高亮推到阶段 2 hybrid 模式。

use editor_engine::Editor;
use eframe::egui;

use crate::editor_state::EditorState;

/// 渲染源码编辑视图。
pub fn show_source_view(ui: &mut egui::Ui, state: &mut EditorState) {
    egui::CentralPanel::default().show_inside(ui, |ui| {
        let text_style = egui::TextStyle::Monospace;
        let row_height = ui.text_style_height(&text_style);

        let available = ui.available_size();
        let line_number_width = row_height * 4.0;

        ui.horizontal(|ui| {
            // 行号列
            ui.allocate_ui_with_layout(
                egui::vec2(line_number_width, available.y),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    let line_count = state.editor.buffer.len_lines();
                    for i in 0..line_count {
                        ui.label(
                            egui::RichText::new(format!("{:>3}", i + 1))
                                .monospace()
                                .weak(),
                        );
                    }
                },
            );

            ui.separator();

            // 编辑器
            let mut text = state.editor.to_string();
            let response = ui.add(
                egui::TextEdit::multiline(&mut text)
                    .desired_width(available.x - line_number_width - 8.0)
                    .desired_rows(20)
                    .font(egui::TextStyle::Monospace)
                    .code_editor(),
            );

            // 若文本变化，重建 editor（阶段 1 简化：整体替换）
            if response.changed() {
                let cursor = state.editor.cursor;
                state.editor = Editor::new(&text);
                let _ = state.editor.set_cursor(cursor);
            }
        });
    });
}
