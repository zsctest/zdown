//! 源码编辑视图。
//!
//! 阶段 2：完全自绘 + 行内语法高亮 + 事件监听增量编辑。
//!
//! 实现：
//! - ui.input(|i| i.events.clone()) 监听键盘事件
//! - 事件转 editor_engine::Command（Insert/Delete）推入历史
//! - ui.painter 绘制光标矩形（精确像素定位）

use std::cell::RefCell;
use std::collections::HashMap;

use eframe::egui;
use markdown_renderer::SourceHighlighter;
use spellcheck::SpellError;

use crate::editor_state::EditorState;
use crate::search_state::SearchState;
use editor_engine::Cursor;

// ---------------------------------------------------------------------------
// 高亮缓存：避免每帧 syntect 重解析整个文档
// ---------------------------------------------------------------------------

/// 缓存的高亮行：预计算的 (颜色, 字符串) 对。
type CachedLine = Vec<(egui::Color32, String)>;

thread_local! {
    static HIGHLIGHT_CACHE: RefCell<HashMap<u64, Vec<CachedLine>>> =
        RefCell::new(HashMap::new());
}

fn hash_src(src: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut hasher);
    hasher.finish()
}

/// 获取缓存的高亮行（缓存命中直接返回，否则调用 syntect 并缓存）。
fn get_cached_highlights(src: &str, highlighter: &SourceHighlighter) -> Vec<CachedLine> {
    let hash = hash_src(src);
    HIGHLIGHT_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.get(&hash) {
            return cached.clone();
        }
        // 缓存未命中：运行 syntect 高亮
        let lines: Vec<CachedLine> = highlighter
            .highlight(src, None)
            .into_iter()
            .map(|line| {
                line.into_iter()
                    .map(|(style, text)| {
                        let color = egui::Color32::from_rgb(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        );
                        (color, text.to_owned())
                    })
                    .collect()
            })
            .collect();
        // 限制缓存大小（文档编辑时旧 hash 无意义）
        if cache.len() > 10 {
            cache.clear();
        }
        cache.insert(hash, lines.clone());
        lines
    })
}

// ---------------------------------------------------------------------------
// 公开入口
// ---------------------------------------------------------------------------

/// 渲染源码编辑视图。
pub fn show_source_view(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    highlighter: Option<&SourceHighlighter>,
    search: &SearchState,
    app_config: &config::ImageHostingConfig,
) {
    let working_dir = state
        .current_path()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    crate::input::handle_dropped_images(
        ui.ctx(),
        state.editor_mut(),
        app_config,
        working_dir.clone(),
    );

    let src = state.editor().to_string();

    // 先处理输入事件（更新 editor），再渲染（避免一帧延迟）
    let ctx = ui.ctx().clone();
    let focus_id = egui::Id::new(("source_view_input", state.active_tab_index()));
    // 必须在 interact 之前消费方向键，否则 egui 会将其用于焦点导航
    crate::input::consume_arrow_keys(&ctx, state, focus_id);
    let input_response = ui.interact(ui.max_rect(), focus_id, egui::Sense::click_and_drag());
    // 显式焦点请求（new/open/切换标签页）：每帧持续请求直到实际获得焦点，
    // 避免因弹出层（菜单等）覆盖而提前消费 needs_focus 标志。
    if state.needs_focus {
        ctx.memory_mut(|m| m.request_focus(focus_id));
    }
    if input_response.has_focus() {
        // 清除 begin_pass 中基于 RawInput.events 设置的焦点导航方向，
        // 阻止 egui 将方向键用于焦点跳转（编辑器已自行处理方向键）。
        ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
        let wd = state
            .current_path()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        crate::input::handle_input(&ctx, state, app_config, wd);
        // 确认焦点已获得，消费 needs_focus
        state.needs_focus = false;
    }
    // 点击获取焦点，或全局无焦点时自动获取
    if input_response.clicked() || ctx.memory(|m| m.focused()).is_none() {
        ctx.memory_mut(|m| m.request_focus(focus_id));
    }

    let cursor_line = state.editor().cursor.line;
    let needs_scroll = state.needs_scroll_cursor;

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            // 行号列
            let line_count = src.lines().count().max(1);
            ui.vertical(|ui| {
                for i in 0..line_count {
                    let resp = ui.label(
                        egui::RichText::new(format!("{:>3}", i + 1))
                            .monospace()
                            .weak(),
                    );
                    if needs_scroll && i == cursor_line {
                        resp.scroll_to_me(Some(egui::Align::Center));
                    }
                }
            });

            ui.separator();

            // 高亮文本 + 光标
            ui.vertical(|ui| {
                render_text_with_cursor(
                    ui,
                    &src,
                    state.editor().cursor,
                    highlighter,
                    search,
                    &state.spell_errors,
                );
            });
        });
    });

    if needs_scroll {
        state.needs_scroll_cursor = false;
    }
}

/// 查找指定行中所有拼写错误的列范围。
fn find_line_spell_errors(
    src: &str,
    spell_errors: &[SpellError],
    line_idx: usize,
) -> Vec<(usize, usize)> {
    if spell_errors.is_empty() {
        return Vec::new();
    }
    // 使用 char_indices 计算每行的字节起始位置
    let mut line_starts: Vec<usize> = vec![0];
    for (idx, ch) in src.char_indices() {
        if ch == '\n' {
            line_starts.push(idx + 1); // 下一行起始在 \n 之后
        }
    }
    let line_starts_with_end = {
        let mut v = line_starts.clone();
        v.push(src.len());
        v
    };

    let line_start = match line_starts.get(line_idx) {
        Some(&s) => s,
        None => return Vec::new(),
    };
    let line_end = match line_starts_with_end.get(line_idx + 1) {
        Some(e) => *e,
        None => return Vec::new(),
    };

    let mut ranges = Vec::new();
    for err in spell_errors {
        let (err_start, err_end) = err.span;
        if err_start >= line_start && err_end <= line_end {
            let col_start = src[line_start..err_start].chars().count();
            let col_end = col_start + err.word.chars().count();
            ranges.push((col_start, col_end));
        }
    }
    ranges
}

/// 绘制红色波浪下划线。
fn paint_squiggly_underline(
    painter: &egui::Painter,
    start: egui::Pos2,
    end: egui::Pos2,
    color: egui::Color32,
) {
    let step = 3.0;
    let amp = 2.0;
    let y_base = start.y + 2.0;
    let mut points = Vec::new();
    let mut x = start.x;
    while x < end.x {
        let phase = ((x - start.x) / step) as i32;
        let y = y_base + if phase % 2 == 0 { -amp } else { amp };
        points.push(egui::pos2(x, y));
        x += step;
    }
    if points.len() >= 2 {
        painter.add(egui::Shape::line(points, egui::Stroke::new(1.0, color)));
    }
}

/// 渲染高亮文本 + 光标矩形（含高亮缓存，避免每帧 syntect 重解析）。
fn render_text_with_cursor(
    ui: &mut egui::Ui,
    src: &str,
    cursor: Cursor,
    highlighter: Option<&SourceHighlighter>,
    search: &SearchState,
    spell_errors: &[SpellError],
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
        // 使用缓存避免每帧 syntect 重解析（主要性能瓶颈）
        let cached_lines = get_cached_highlights(src, h);
        for (line_idx, line) in cached_lines.iter().enumerate() {
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

            // 绘制高亮文本（颜色已预计算，跳过 syntect）
            let mut x = rect.min.x;
            for (color, text) in line {
                let galley = ui
                    .ctx()
                    .fonts_mut(|f| f.layout_no_wrap(text.clone(), font_id.clone(), *color));
                ui.painter()
                    .galley(egui::pos2(x, rect.min.y), galley.clone(), *color);
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

            // 绘制拼写错误波浪线
            let spell_ranges = find_line_spell_errors(src, spell_errors, line_idx);
            for (col_start, col_end) in spell_ranges {
                let err_prefix: String = line
                    .iter()
                    .flat_map(|(_, t)| t.chars())
                    .take(col_start)
                    .collect();
                let err_text: String = line
                    .iter()
                    .flat_map(|(_, t)| t.chars())
                    .skip(col_start)
                    .take(col_end - col_start)
                    .collect();
                let err_prefix_galley = ui.ctx().fonts_mut(|f| {
                    f.layout_no_wrap(err_prefix, font_id.clone(), egui::Color32::WHITE)
                });
                let err_text_galley = ui.ctx().fonts_mut(|f| {
                    f.layout_no_wrap(err_text, font_id.clone(), egui::Color32::WHITE)
                });
                let squiggly_start =
                    egui::pos2(rect.min.x + err_prefix_galley.size().x, rect.max.y);
                let squiggly_end =
                    egui::pos2(squiggly_start.x + err_text_galley.size().x, rect.max.y);
                paint_squiggly_underline(
                    ui.painter(),
                    squiggly_start,
                    squiggly_end,
                    egui::Color32::from_rgb(224, 108, 117), // 红色 #e06c75
                );
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

            // 绘制拼写错误波浪线
            let spell_ranges = find_line_spell_errors(src, spell_errors, line_idx);
            for (col_start, col_end) in spell_ranges {
                let err_prefix: String = line.chars().take(col_start).collect();
                let err_text: String = line
                    .chars()
                    .skip(col_start)
                    .take(col_end - col_start)
                    .collect();
                let err_prefix_galley = ui.ctx().fonts_mut(|f| {
                    f.layout_no_wrap(err_prefix, font_id.clone(), egui::Color32::WHITE)
                });
                let err_text_galley = ui.ctx().fonts_mut(|f| {
                    f.layout_no_wrap(err_text, font_id.clone(), egui::Color32::WHITE)
                });
                let squiggly_start =
                    egui::pos2(rect.min.x + err_prefix_galley.size().x, rect.max.y);
                let squiggly_end =
                    egui::pos2(squiggly_start.x + err_text_galley.size().x, rect.max.y);
                paint_squiggly_underline(
                    ui.painter(),
                    squiggly_start,
                    squiggly_end,
                    egui::Color32::from_rgb(224, 108, 117),
                );
            }
        }
    }
}
