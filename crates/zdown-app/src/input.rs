//! 共享输入处理：将 egui 事件转为 editor_engine::Command。
//!
//! 被 source_view 和 hybrid_view 共用。

use std::path::PathBuf;

use editor_engine::{Command, Cursor, Editor};
use eframe::egui;

use crate::editor_state::EditorState;

/// 处理输入事件，转为 editor_engine::Command。
pub(crate) fn handle_input(
    ctx: &egui::Context,
    state: &mut EditorState,
    config: &config::ImageHostingConfig,
    working_dir: Option<PathBuf>,
) {
    let events = ctx.input(|i| i.events.clone());
    for event in events {
        match event {
            egui::Event::Paste(text) => {
                // 先尝试剪贴板图片粘贴
                if try_paste_image(state.editor_mut(), config, working_dir.clone()) {
                    // 图片已粘贴，跳过文本
                    continue;
                }
                // 无图片 → 文本粘贴
                if !text.is_empty() {
                    let cursor = state.editor().cursor;
                    let _ = state.apply(Command::Insert { pos: cursor, text });
                }
            }
            egui::Event::Text(text) => {
                if !text.is_empty() {
                    let cursor = state.editor().cursor;
                    let _ = state.apply(Command::Insert { pos: cursor, text });
                }
            }
            egui::Event::Key {
                key: egui::Key::Backspace,
                pressed: true,
                ..
            } => {
                let cursor = state.editor().cursor;
                if let Some(prev) = prev_cursor(&state.editor().buffer, cursor) {
                    let _ = state.apply(Command::Delete {
                        range: editor_engine::Selection::new(prev, cursor),
                    });
                    let _ = state.editor_mut().set_cursor(prev);
                }
            }
            egui::Event::Key {
                key: egui::Key::Delete,
                pressed: true,
                ..
            } => {
                let cursor = state.editor().cursor;
                if let Some(next) = next_cursor(&state.editor().buffer, cursor) {
                    let _ = state.apply(Command::Delete {
                        range: editor_engine::Selection::new(cursor, next),
                    });
                    let _ = state.editor_mut().set_cursor(next);
                }
            }
            egui::Event::Key {
                key: egui::Key::Enter,
                pressed: true,
                ..
            } => {
                let cursor = state.editor().cursor;
                if state
                    .apply(Command::Insert {
                        pos: cursor,
                        text: "\n".into(),
                    })
                    .is_ok()
                {
                    let _ = state
                        .editor_mut()
                        .set_cursor(Cursor::new(cursor.line + 1, 0));
                }
            }
            egui::Event::Key {
                key: egui::Key::ArrowLeft,
                pressed: true,
                ..
            } => {
                let cursor = state.editor().cursor;
                if let Some(prev) = prev_cursor(&state.editor().buffer, cursor) {
                    let _ = state.editor_mut().set_cursor(prev);
                }
            }
            egui::Event::Key {
                key: egui::Key::ArrowRight,
                pressed: true,
                ..
            } => {
                let cursor = state.editor().cursor;
                if let Some(next) = next_cursor(&state.editor().buffer, cursor) {
                    let _ = state.editor_mut().set_cursor(next);
                }
            }
            egui::Event::Key {
                key: egui::Key::ArrowUp,
                pressed: true,
                ..
            } => {
                let cursor = state.editor().cursor;
                if cursor.line > 0 {
                    let target_line = cursor.line - 1;
                    let max_col = state
                        .editor()
                        .buffer
                        .line_len_chars(target_line)
                        .unwrap_or(0);
                    let new_col = cursor.col.min(max_col);
                    let _ = state
                        .editor_mut()
                        .set_cursor(Cursor::new(target_line, new_col));
                }
            }
            egui::Event::Key {
                key: egui::Key::ArrowDown,
                pressed: true,
                ..
            } => {
                let cursor = state.editor().cursor;
                let line_count = state.editor().buffer.len_lines();
                if cursor.line + 1 < line_count {
                    let target_line = cursor.line + 1;
                    let max_col = state
                        .editor()
                        .buffer
                        .line_len_chars(target_line)
                        .unwrap_or(0);
                    let new_col = cursor.col.min(max_col);
                    let _ = state
                        .editor_mut()
                        .set_cursor(Cursor::new(target_line, new_col));
                }
            }
            egui::Event::Key {
                key: egui::Key::Tab,
                pressed: true,
                ..
            } => {
                // 阶段 2：拦截 Tab 不处理（避免焦点跳转），阶段 3 实现 Tab 缩进
            }
            _ => {}
        }
    }
}

/// 计算光标前一个位置。
pub(crate) fn prev_cursor(buffer: &editor_engine::Buffer, cursor: Cursor) -> Option<Cursor> {
    if cursor.col > 0 {
        Some(Cursor::new(cursor.line, cursor.col - 1))
    } else if cursor.line > 0 {
        let prev_line = cursor.line - 1;
        let len = buffer.line_len_chars(prev_line).ok()?;
        Some(Cursor::new(prev_line, len))
    } else {
        None
    }
}

/// 计算光标后一个位置。
pub(crate) fn next_cursor(buffer: &editor_engine::Buffer, cursor: Cursor) -> Option<Cursor> {
    let line_len = buffer.line_len_chars(cursor.line).ok()?;
    if cursor.col < line_len {
        Some(Cursor::new(cursor.line, cursor.col + 1))
    } else {
        let line_count = buffer.len_lines();
        if cursor.line + 1 < line_count {
            Some(Cursor::new(cursor.line + 1, 0))
        } else {
            None
        }
    }
}

/// 处理拖拽的图片文件，插入到编辑器。
/// 返回实际插入的图片数量。
pub(crate) fn handle_dropped_images(
    ctx: &egui::Context,
    editor: &mut Editor,
    config: &config::ImageHostingConfig,
    working_dir: Option<std::path::PathBuf>,
) -> usize {
    let dropped = ctx.input(|i| i.raw.dropped_files.clone());
    if dropped.is_empty() {
        return 0;
    }

    let storage = crate::image_hosting::create_storage(config, working_dir);
    let mut inserted = 0;

    for file in &dropped {
        let mime = file.mime.to_lowercase();
        if !mime.starts_with("image/") {
            continue;
        }
        let data = match &file.bytes {
            Some(b) => b.to_vec(),
            None => match &file.path {
                Some(p) => match std::fs::read(p) {
                    Ok(b) => b,
                    Err(_) => continue,
                },
                None => continue,
            },
        };
        let name = file
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.name.clone());

        let format = crate::image_hosting::ImageFormat::from_filename(&name);

        match storage.store(&data, &name, format) {
            Ok(url) => {
                let md_text = if inserted == 0 {
                    format!("![{name}]({url})")
                } else {
                    format!("\n![{name}]({url})")
                };
                let cursor = editor.cursor;
                let _ = editor.apply(Command::Insert {
                    pos: cursor,
                    text: md_text,
                });
                inserted += 1;
            }
            Err(_) => {
                // 跳过失败的图片，继续处理下一个
            }
        }
    }

    inserted
}

/// 尝试从剪贴板读取图片并插入到编辑器。
/// 返回 `true` 如果插入了图片（此时不应再处理文本粘贴）。
fn try_paste_image(
    editor: &mut Editor,
    config: &config::ImageHostingConfig,
    working_dir: Option<PathBuf>,
) -> bool {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(_) => return false,
    };

    let image_data = match clipboard.get_image() {
        Ok(img) => img,
        Err(_) => return false,
    };

    // RGBA → PNG 编码
    let png_bytes = match rgba_to_png(&image_data) {
        Some(b) => b,
        None => return false,
    };
    let storage = crate::image_hosting::create_storage(config, working_dir);
    let filename = "clipboard_image";
    let format = crate::image_hosting::ImageFormat::Png;

    match storage.store(&png_bytes, filename, format) {
        Ok(url) => {
            let md_text = format!("![image]({url})");
            let cursor = editor.cursor;
            let _ = editor.apply(Command::Insert {
                pos: cursor,
                text: md_text,
            });
            true
        }
        Err(_) => false,
    }
}

/// 将 arboard ImageData (RGBA) 编码为 PNG 字节。
fn rgba_to_png(img: &arboard::ImageData) -> Option<Vec<u8>> {
    let rgba = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.to_vec())?;
    let mut png_data = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut png_data, image::ImageFormat::Png)
        .ok()?;
    Some(png_data.into_inner())
}
