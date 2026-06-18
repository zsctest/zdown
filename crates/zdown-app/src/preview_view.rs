//! 预览模式视图：AST → egui 渲染。

use eframe::egui;

use crate::editor_state::EditorState;

/// 渲染预览视图。
pub fn show_preview_view(ui: &mut egui::Ui, state: &mut EditorState) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        let doc = state.current_doc();
        markdown_renderer::render(ui, &doc);
    });
}
