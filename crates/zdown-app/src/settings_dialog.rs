//! 设置对话框：自定义 CSS 编辑器。
//!
//! 提供 `egui::Window` 模态对话框，允许用户编辑全局自定义 CSS。
//! 保存后写入 `config::AppConfig::save()`。

use config::AppConfig;
use eframe::egui;

/// 设置对话框状态。
#[derive(Debug, Clone, Default)]
pub struct SettingsDialog {
    /// 对话框是否打开。
    pub open: bool,
    /// 用户正在编辑的 CSS 文本缓冲区。
    css_buffer: String,
}

impl SettingsDialog {
    /// 打开对话框，将当前配置填充到编辑缓冲区。
    pub fn open_dialog(&mut self, current_css: Option<&str>) {
        self.open = true;
        self.css_buffer = current_css.unwrap_or("").to_string();
    }
}

/// 渲染设置对话框（若 `dialog.open` 为 true）。
pub fn show_settings_dialog(
    ctx: &egui::Context,
    app_config: &mut AppConfig,
    dialog: &mut SettingsDialog,
) {
    if !dialog.open {
        return;
    }

    let mut close_this = false;
    let mut new_css = dialog.css_buffer.clone();

    egui::Window::new("设置")
        .collapsible(false)
        .resizable(true)
        .min_size(egui::vec2(480.0, 300.0))
        .show(ctx, |ui| {
            ui.label("自定义 CSS（追加到内置样式之后，留空表示不使用）：");
            ui.add_space(4.0);

            ui.add_sized(
                [480.0, 300.0],
                egui::TextEdit::multiline(&mut new_css)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .hint_text(
                        "/* 在此输入自定义 CSS，例如: */\nh1 { color: #2196F3; }\nbody { max-width: 900px; }",
                    ),
            );

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("保存").clicked() {
                    app_config.custom_css = if new_css.trim().is_empty() {
                        None
                    } else {
                        Some(new_css.clone())
                    };
                    if let Err(e) = app_config.save() {
                        tracing::error!("配置保存失败: {e}");
                    } else {
                        tracing::info!("配置已保存");
                    }
                    close_this = true;
                }
                if ui.button("取消").clicked() {
                    close_this = true;
                }
            });
        });

    if close_this {
        dialog.open = false;
    } else {
        dialog.css_buffer = new_css;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_populates_buffer() {
        let mut dialog = SettingsDialog::default();
        dialog.open_dialog(Some("h1{color:red}"));
        assert!(dialog.open);
        assert_eq!(dialog.css_buffer, "h1{color:red}");
    }

    #[test]
    fn open_with_none_sets_empty_buffer() {
        let mut dialog = SettingsDialog::default();
        dialog.open_dialog(None);
        assert!(dialog.open);
        assert_eq!(dialog.css_buffer, "");
    }

    #[test]
    fn default_dialog_is_closed() {
        let dialog = SettingsDialog::default();
        assert!(!dialog.open);
        assert_eq!(dialog.css_buffer, "");
    }
}
