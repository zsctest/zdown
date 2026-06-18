//! 源码编辑视图。
//!
//! 阶段 2：完全自绘 + 行内语法高亮 + 事件监听增量编辑。
//!
//! 实现：
//! - ui.input(|i| i.events.clone()) 监听键盘事件
//! - 事件转 editor_engine::Command（Insert/Delete）推入历史
//! - ui.painter 绘制光标矩形（精确像素定位）

use eframe::egui;
use markdown_renderer::SourceHighlighter;

use crate::editor_state::EditorState;
use editor_engine::Cursor;

/// 渲染源码编辑视图。
pub fn show_source_view(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    highlighter: Option<&SourceHighlighter>,
) {
    let src = state.editor.to_string();

    // 先处理输入事件（更新 editor），再渲染（避免一帧延迟）
    let ctx = ui.ctx().clone();
    let input_response = ui.interact(
        ui.max_rect(),
        egui::Id::new("source_view_input"),
        egui::Sense::click_and_drag(),
    );
    if input_response.has_focus() {
        crate::input::handle_input(&ctx, state);
    }
    // 点击获取焦点
    if input_response.clicked() {
        ctx.memory_mut(|m| m.request_focus(egui::Id::new("source_view_input")));
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            // 行号列
            let line_count = src.lines().count().max(1);
            ui.vertical(|ui| {
                for i in 0..line_count {
                    ui.label(
                        egui::RichText::new(format!("{:>3}", i + 1))
                            .monospace()
                            .weak(),
                    );
                }
            });

            ui.separator();

            // 高亮文本 + 光标
            ui.vertical(|ui| {
                render_text_with_cursor(ui, &src, state.editor.cursor, highlighter);
            });
        });
    });
}

/// 渲染高亮文本 + 光标矩形。
fn render_text_with_cursor(
    ui: &mut egui::Ui,
    src: &str,
    cursor: Cursor,
    highlighter: Option<&SourceHighlighter>,
) {
    // 从 egui style 获取等宽字体字号，避免硬编码
    let font_id = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Monospace)
        .cloned()
        .unwrap_or_else(|| egui::FontId::monospace(14.0));
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);

    if let Some(h) = highlighter {
        let lines = h.highlight(src, None);
        for (line_idx, line) in lines.iter().enumerate() {
            let (rect, _) = ui.allocate_at_least(
                egui::vec2(ui.available_width(), row_height),
                egui::Sense::hover(),
            );

            // 绘制光标矩形（在光标所在行）
            if line_idx == cursor.line {
                // 计算光标 x 位置：光标前所有字符的宽度之和
                let prefix: String = line
                    .iter()
                    .flat_map(|(_, t)| t.chars())
                    .take(cursor.col)
                    .collect();
                let prefix_galley = ui.ctx().fonts_mut(|f| {
                    f.layout_no_wrap(prefix.clone(), font_id.clone(), egui::Color32::WHITE)
                });
                let cursor_x = rect.min.x + prefix_galley.size().x;
                let cursor_rect = egui::Rect::from_min_size(
                    egui::pos2(cursor_x, rect.min.y),
                    egui::vec2(2.0, row_height),
                );
                ui.painter()
                    .rect_filled(cursor_rect, 0.0, egui::Color32::from_rgb(200, 200, 200));
            }

            // 绘制高亮文本
            let mut x = rect.min.x;
            for (style, text) in line {
                let color = egui::Color32::from_rgb(
                    style.foreground.r,
                    style.foreground.g,
                    style.foreground.b,
                );
                let galley = ui
                    .ctx()
                    .fonts_mut(|f| f.layout_no_wrap((*text).to_string(), font_id.clone(), color));
                ui.painter()
                    .galley(egui::pos2(x, rect.min.y), galley.clone(), color);
                x += galley.size().x;
            }
        }
    } else {
        // fallback：不高亮
        for (line_idx, line) in src.lines().enumerate() {
            let (rect, _) = ui.allocate_at_least(
                egui::vec2(ui.available_width(), row_height),
                egui::Sense::hover(),
            );
            if line_idx == cursor.line {
                let prefix: String = line.chars().take(cursor.col).collect();
                let prefix_galley = ui
                    .ctx()
                    .fonts_mut(|f| f.layout_no_wrap(prefix, font_id.clone(), egui::Color32::WHITE));
                let cursor_x = rect.min.x + prefix_galley.size().x;
                let cursor_rect = egui::Rect::from_min_size(
                    egui::pos2(cursor_x, rect.min.y),
                    egui::vec2(2.0, row_height),
                );
                ui.painter()
                    .rect_filled(cursor_rect, 0.0, egui::Color32::from_rgb(200, 200, 200));
            }
            let galley = ui.ctx().fonts_mut(|f| {
                f.layout_no_wrap(line.to_string(), font_id.clone(), egui::Color32::WHITE)
            });
            ui.painter().galley(rect.min, galley, egui::Color32::WHITE);
        }
    }
}
