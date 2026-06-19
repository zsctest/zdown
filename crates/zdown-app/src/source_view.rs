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
use crate::search_state::SearchState;
use editor_engine::Cursor;

/// 渲染源码编辑视图。
pub fn show_source_view(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    highlighter: Option<&SourceHighlighter>,
    search: &SearchState,
    app_config: &config::ImageHostingConfig,
) {
    let working_dir = state.current_path().and_then(|p| p.parent().map(|d| d.to_path_buf()));
    crate::input::handle_dropped_images(ui.ctx(), state.editor_mut(), app_config, working_dir.clone());

    let src = state.editor().to_string();

    // 先处理输入事件（更新 editor），再渲染（避免一帧延迟）
    let ctx = ui.ctx().clone();
    let focus_id = egui::Id::new(("source_view_input", state.active_tab_index()));
    let input_response = ui.interact(ui.max_rect(), focus_id, egui::Sense::click_and_drag());
    if input_response.has_focus() {
        let wd = state.current_path().and_then(|p| p.parent().map(|d| d.to_path_buf()));
        crate::input::handle_input(&ctx, state, app_config, wd);
    }
    // 点击获取焦点
    if input_response.clicked() {
        ctx.memory_mut(|m| m.request_focus(focus_id));
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
                render_text_with_cursor(ui, &src, state.editor().cursor, highlighter, search);
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
    search: &SearchState,
) {
    // 从 egui style 获取等宽字体字号，避免硬编码
    let font_id = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Monospace)
        .cloned()
        .unwrap_or_else(|| egui::FontId::monospace(14.0));
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);

    // 收集当前行的匹配范围（用于高亮绘制）
    fn line_match_ranges(search: &SearchState, line_idx: usize) -> Vec<(usize, usize, bool)> {
        let mut ranges: Vec<(usize, usize, bool)> = Vec::new();
        if !search.visible || search.matches.is_empty() {
            return ranges;
        }
        let current_idx = search.current_match;
        for (i, m) in search.matches.iter().enumerate() {
            if m.line == line_idx {
                let is_current = current_idx == Some(i);
                ranges.push((m.col_start, m.col_end, is_current));
            }
        }
        ranges
    }

    if let Some(h) = highlighter {
        let lines = h.highlight(src, None);
        for (line_idx, line) in lines.iter().enumerate() {
            let match_ranges = line_match_ranges(search, line_idx);
            let (rect, _) = ui.allocate_at_least(
                egui::vec2(ui.available_width(), row_height),
                egui::Sense::hover(),
            );

            // 绘制匹配高亮背景（在文本之前，确保文本在背景之上）
            for &(col_start, col_end, is_current) in &match_ranges {
                let m_prefix: String = line
                    .iter()
                    .flat_map(|(_, t)| t.chars())
                    .take(col_start)
                    .collect();
                let m_text: String = line
                    .iter()
                    .flat_map(|(_, t)| t.chars())
                    .skip(col_start)
                    .take(col_end - col_start)
                    .collect();
                let m_prefix_galley = ui.ctx().fonts_mut(|f| {
                    f.layout_no_wrap(m_prefix, font_id.clone(), egui::Color32::WHITE)
                });
                let m_text_galley = ui
                    .ctx()
                    .fonts_mut(|f| f.layout_no_wrap(m_text, font_id.clone(), egui::Color32::WHITE));
                let bg_x = rect.min.x + m_prefix_galley.size().x;
                let bg_w = m_text_galley.size().x;
                let bg_color = if is_current {
                    egui::Color32::from_rgb(212, 133, 11) // 橙色 #d4850b
                } else {
                    egui::Color32::from_rgb(107, 76, 18) // 暗黄 #6b4c12
                };
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(bg_x, rect.min.y),
                        egui::vec2(bg_w, row_height),
                    ),
                    0.0,
                    bg_color,
                );
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

            // 绘制光标矩形（在光标所在行，在匹配高亮之上）
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
        }
    } else {
        // fallback：不高亮
        for (line_idx, line) in src.lines().enumerate() {
            let match_ranges = line_match_ranges(search, line_idx);
            let (rect, _) = ui.allocate_at_least(
                egui::vec2(ui.available_width(), row_height),
                egui::Sense::hover(),
            );

            // 绘制匹配高亮背景（在文本之前，确保文本在背景之上）
            for &(col_start, col_end, is_current) in &match_ranges {
                let m_prefix: String = line.chars().take(col_start).collect();
                let m_text: String = line
                    .chars()
                    .skip(col_start)
                    .take(col_end - col_start)
                    .collect();
                let m_prefix_galley = ui.ctx().fonts_mut(|f| {
                    f.layout_no_wrap(m_prefix, font_id.clone(), egui::Color32::WHITE)
                });
                let m_text_galley = ui
                    .ctx()
                    .fonts_mut(|f| f.layout_no_wrap(m_text, font_id.clone(), egui::Color32::WHITE));
                let bg_x = rect.min.x + m_prefix_galley.size().x;
                let bg_w = m_text_galley.size().x;
                let bg_color = if is_current {
                    egui::Color32::from_rgb(212, 133, 11)
                } else {
                    egui::Color32::from_rgb(107, 76, 18)
                };
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(bg_x, rect.min.y),
                        egui::vec2(bg_w, row_height),
                    ),
                    0.0,
                    bg_color,
                );
            }

            let galley = ui.ctx().fonts_mut(|f| {
                f.layout_no_wrap(line.to_string(), font_id.clone(), egui::Color32::WHITE)
            });
            ui.painter().galley(rect.min, galley, egui::Color32::WHITE);

            // 绘制光标矩形（在匹配高亮之上）
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
        }
    }
}
