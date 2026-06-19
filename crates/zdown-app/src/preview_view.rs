//! 预览模式视图：AST → egui 渲染。

use config;
use eframe::egui;
use markdown_renderer::RenderCache;

use crate::editor_state::EditorState;

/// 渲染预览视图。
pub fn show_preview_view(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    cache: &mut RenderCache,
    app_config: &config::ImageHostingConfig,
) {
    let working_dir = state.current_path().and_then(|p| p.parent().map(|d| d.to_path_buf()));
    crate::input::handle_dropped_images(ui.ctx(), state.editor_mut(), app_config, working_dir);

    egui::ScrollArea::vertical().show(ui, |ui| {
        let src = state.editor().to_string();
        let doc = cache.parse_cached(&src);
        markdown_renderer::render(ui, &doc);
    });
}
