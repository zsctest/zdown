//! 共享输入处理：将 egui 事件转为 editor_engine::Command。
//!
//! 被 source_view 和 hybrid_view 共用。

use config;
use editor_engine::{Command, Cursor, Editor};
use eframe::egui;

use editor_engine::Editor;

/// 处理输入事件，转为 editor_engine::Command。
pub(crate) fn handle_input(ctx: &egui::Context, editor: &mut Editor) {
    let events = ctx.input(|i| i.events.clone());
    for event in events {
        match event {
            egui::Event::Text(text) => {
                if !text.is_empty() {
                    let cursor = editor.cursor;
                    let _ = editor.apply(Command::Insert { pos: cursor, text });
                }
            }
            egui::Event::Key {
                key: egui::Key::Backspace,
                pressed: true,
                ..
            } => {
                let cursor = editor.cursor;
                if let Some(prev) = prev_cursor(&editor.buffer, cursor) {
                    let _ = editor.apply(Command::Delete {
                        range: editor_engine::Selection::new(prev, cursor),
                    });
                    let _ = editor.set_cursor(prev);
                }
            }
            egui::Event::Key {
                key: egui::Key::Delete,
                pressed: true,
                ..
            } => {
                let cursor = editor.cursor;
                if let Some(next) = next_cursor(&editor.buffer, cursor) {
                    let _ = editor.apply(Command::Delete {
                        range: editor_engine::Selection::new(cursor, next),
                    });
                    let _ = editor.set_cursor(next);
                }
            }
            egui::Event::Key {
                key: egui::Key::Enter,
                pressed: true,
                ..
            } => {
                let cursor = editor.cursor;
                if editor
                    .apply(Command::Insert {
                        pos: cursor,
                        text: "\n".into(),
                    })
                    .is_ok()
                {
                    let _ = editor.set_cursor(Cursor::new(cursor.line + 1, 0));
                }
            }
            egui::Event::Key {
                key: egui::Key::ArrowLeft,
                pressed: true,
                ..
            } => {
                let cursor = editor.cursor;
                if let Some(prev) = prev_cursor(&editor.buffer, cursor) {
                    let _ = editor.set_cursor(prev);
                }
            }
            egui::Event::Key {
                key: egui::Key::ArrowRight,
                pressed: true,
                ..
            } => {
                let cursor = editor.cursor;
                if let Some(next) = next_cursor(&editor.buffer, cursor) {
                    let _ = editor.set_cursor(next);
                }
            }
            egui::Event::Key {
                key: egui::Key::ArrowUp,
                pressed: true,
                ..
            } => {
                let cursor = editor.cursor;
                if cursor.line > 0 {
                    let target_line = cursor.line - 1;
                    let max_col = editor.buffer.line_len_chars(target_line).unwrap_or(0);
                    let new_col = cursor.col.min(max_col);
                    let _ = editor.set_cursor(Cursor::new(target_line, new_col));
                }
            }
            egui::Event::Key {
                key: egui::Key::ArrowDown,
                pressed: true,
                ..
            } => {
                let cursor = editor.cursor;
                let line_count = editor.buffer.len_lines();
                if cursor.line + 1 < line_count {
                    let target_line = cursor.line + 1;
                    let max_col = editor.buffer.line_len_chars(target_line).unwrap_or(0);
                    let new_col = cursor.col.min(max_col);
                    let _ = editor.set_cursor(Cursor::new(target_line, new_col));
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
            None => {
                match &file.path {
                    Some(p) => match std::fs::read(p) {
                        Ok(b) => b,
                        Err(_) => continue,
                    },
                    None => continue,
                }
            }
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
                let _ = editor.apply(Command::Insert { pos: cursor, text: md_text });
                inserted += 1;
            }
            Err(_) => {
                // 跳过失败的图片，继续处理下一个
            }
        }
    }

    inserted
}
