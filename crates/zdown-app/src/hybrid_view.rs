//! Hybrid 视图：光标所在 block 源码 + 其余 block 渲染。
//!
//! 用 BlockWithSpan 的 span 查找光标所在 block，避免按行切割破坏多行结构。

use eframe::egui;
use markdown_renderer::SourceHighlighter;

use crate::editor_state::EditorState;

/// 渲染 hybrid 视图。
pub fn show_hybrid_view(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    highlighter: Option<&SourceHighlighter>,
    cache: &mut markdown_renderer::RenderCache,
) {
    let src = state.editor.to_string();
    let cursor_line = state.editor.cursor.line;

    // 先处理输入（复用 source_view 的 prev_cursor/next_cursor + 同样的键处理逻辑）
    let ctx = ui.ctx().clone();
    let input_response = ui.interact(
        ui.max_rect(),
        egui::Id::new("hybrid_view_input"),
        egui::Sense::click_and_drag(),
    );
    if input_response.has_focus() {
        crate::input::handle_input(&ctx, state);
    }
    if input_response.clicked() {
        ctx.memory_mut(|m| m.request_focus(egui::Id::new("hybrid_view_input")));
    }

    let doc = cache.parse_cached(&src);

    egui::ScrollArea::vertical().show(ui, |ui| {
        // 找光标所在 block 的索引
        let cursor_block_idx = doc
            .blocks
            .iter()
            .position(|b| cursor_line >= b.span.start_line && cursor_line <= b.span.end_line);

        match cursor_block_idx {
            Some(idx) => {
                // 光标 block 之前的 block：全渲染
                for bws in &doc.blocks[..idx] {
                    render_single_block(ui, &bws.block);
                }

                // 光标 block：源码高亮 + 光标
                let cursor_bws = &doc.blocks[idx];
                let cursor_block_src = extract_block_src(&src, cursor_bws.span);
                // 光标在 block 内的相对行号
                let relative_cursor_line = cursor_line - cursor_bws.span.start_line;
                render_source_block_with_cursor(
                    ui,
                    &cursor_block_src,
                    relative_cursor_line,
                    state.editor.cursor.col,
                    highlighter,
                );

                // 光标 block 之后的 block：全渲染
                for bws in &doc.blocks[idx + 1..] {
                    render_single_block(ui, &bws.block);
                }
            }
            None => {
                // 光标不在任何 block 内（如空文档末尾），全部渲染
                markdown_renderer::render(ui, &doc);
            }
        }
    });
}

/// 渲染单个 Block（用于非光标 block）。
fn render_single_block(ui: &mut egui::Ui, block: &document_model::ast::Block) {
    let doc = document_model::Document {
        blocks: vec![document_model::ast::BlockWithSpan {
            block: block.clone(),
            span: document_model::ast::Span {
                start_line: 0,
                end_line: 0,
            },
        }],
    };
    markdown_renderer::render(ui, &doc);
}

/// 提取指定 span 的源码片段。
fn extract_block_src(src: &str, span: document_model::ast::Span) -> String {
    src.lines()
        .skip(span.start_line)
        .take(span.end_line - span.start_line + 1)
        .map(|l| format!("{l}\n"))
        .collect()
}

/// 渲染源码 block + 光标（光标行用背景色标记）。
fn render_source_block_with_cursor(
    ui: &mut egui::Ui,
    block_src: &str,
    relative_cursor_line: usize,
    cursor_col: usize,
    highlighter: Option<&SourceHighlighter>,
) {
    if let Some(h) = highlighter {
        let lines = h.highlight(block_src, None);
        ui.vertical(|ui| {
            for (line_idx, line) in lines.iter().enumerate() {
                let is_cursor_line = line_idx == relative_cursor_line;
                ui.horizontal(|ui| {
                    let mut col = 0;
                    for (style, text) in line {
                        let color = egui::Color32::from_rgb(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        );
                        // 光标所在片段用背景色标记
                        let is_cursor_fragment = is_cursor_line
                            && col <= cursor_col
                            && cursor_col < col + text.chars().count();
                        let richtext = egui::RichText::new(*text).color(color).monospace();
                        if is_cursor_fragment {
                            ui.label(
                                richtext.background_color(egui::Color32::from_rgb(80, 80, 80)),
                            );
                        } else {
                            ui.label(richtext);
                        }
                        col += text.chars().count();
                    }
                });
            }
        });
    } else {
        ui.label(egui::RichText::new(block_src).monospace());
    }
}
