//! 设置对话框：自定义 CSS 编辑器 + 图片托管设置。
//!
//! 提供 `egui::Window` 模态对话框，允许用户：
//! - 编辑全局自定义 CSS（样式标签页）
//! - 配置图片存储策略、本地目录、SM.MS Token（图片标签页）
//!   保存后写入 `config::AppConfig::save()`。

use config::{AppConfig, EditorFontConfig, ImageHostingConfig, ImageStrategy};
use eframe::egui;

use crate::font_provider::FontProvider;

/// 设置对话框标签页。
#[derive(Debug, Clone, Copy, PartialEq)]
enum SettingsTab {
    Css,
    Font,
    Image,
}

/// 设置对话框状态。
#[derive(Debug, Clone)]
pub struct SettingsDialog {
    /// 对话框是否打开。
    pub open: bool,
    active_tab: SettingsTab,
    /// 用户正在编辑的 CSS 文本缓冲区。
    css_buffer: String,
    /// 字体设置缓冲区
    font_family_buffer: String,
    font_size_buffer: f32,
    /// 图片设置缓冲区
    local_dir_buffer: String,
    smms_token_buffer: String,
    strategy_buffer: usize, // 0=Local, 1=Base64, 2=SmMs
}

impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            active_tab: SettingsTab::Css,
            css_buffer: String::new(),
            font_family_buffer: "monospace".to_string(),
            font_size_buffer: 14.0,
            local_dir_buffer: "images".to_string(),
            smms_token_buffer: String::new(),
            strategy_buffer: 0,
        }
    }
}

impl SettingsDialog {
    /// 打开对话框，将当前配置填充到编辑缓冲区。
    pub fn open_dialog(
        &mut self,
        current_css: Option<&str>,
        image_config: &ImageHostingConfig,
        editor_font: &EditorFontConfig,
    ) {
        self.open = true;
        self.active_tab = SettingsTab::Css;
        self.css_buffer = current_css.unwrap_or("").to_string();
        self.local_dir_buffer = image_config.local_dir.clone();
        self.smms_token_buffer = image_config.smms.api_token.clone();
        self.strategy_buffer = match image_config.default_strategy {
            ImageStrategy::Local => 0,
            ImageStrategy::Base64 => 1,
            ImageStrategy::SmMs => 2,
        };
        self.font_family_buffer = editor_font.family.clone();
        self.font_size_buffer = editor_font.size;
    }
}

/// 字体列表的显示文本："monospace" 显示为 "系统默认等宽"。
fn font_display_name(family: &str) -> String {
    if family == "monospace" {
        "系统默认等宽".to_string()
    } else {
        family.to_string()
    }
}

/// 渲染设置对话框（若 `dialog.open` 为 true）。
pub fn show_settings_dialog(
    ctx: &egui::Context,
    app_config: &mut AppConfig,
    dialog: &mut SettingsDialog,
    available_fonts: &[String],
) {
    if !dialog.open {
        return;
    }

    let mut close_this = false;
    let mut new_css = dialog.css_buffer.clone();

    egui::Window::new("设置")
        .collapsible(false)
        .resizable(true)
        .min_size(egui::vec2(480.0, 350.0))
        .show(ctx, |ui| {
            // 标签栏
            ui.horizontal(|ui| {
                ui.selectable_value(&mut dialog.active_tab, SettingsTab::Css, "样式");
                ui.selectable_value(&mut dialog.active_tab, SettingsTab::Font, "字体");
                ui.selectable_value(&mut dialog.active_tab, SettingsTab::Image, "图片");
            });
            ui.separator();

            match dialog.active_tab {
                SettingsTab::Css => {
                    // === 原有 CSS 编辑 UI ===
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
                }
                SettingsTab::Font => {
                    let font_changed_before = (
                        dialog.font_family_buffer.clone(),
                        dialog.font_size_buffer,
                    );

                    ui.label("编辑器字体：");
                    egui::ComboBox::from_id_salt("editor_font_combo")
                        .width(300.0)
                        .selected_text(font_display_name(&dialog.font_family_buffer))
                        .show_ui(ui, |ui| {
                            for family in available_fonts {
                                let label = font_display_name(family);
                                if ui.selectable_label(false, label).clicked() {
                                    dialog.font_family_buffer = family.clone();
                                }
                            }
                        });

                    ui.add_space(8.0);

                    ui.label("字号：");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut dialog.font_size_buffer)
                                .range(8.0..=32.0)
                                .speed(1.0),
                        );
                        ui.label("pt");
                    });

                    ui.add_space(12.0);

                    // 预览
                    ui.label("预览：");
                    egui::Frame::group(ui.style())
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.monospace(
                                egui::RichText::new(
                                    "The quick brown fox jumps over the lazy dog. 0123456789\n\
                                     敏捷的棕狐狸跳过了那只懒狗。\n\
                                     fn main() { println!(\"Hello, zdown!\"); }",
                                )
                                .size(dialog.font_size_buffer),
                            );
                        });

                    // 字体/字号变化即预览
                    let font_changed_after = (
                        dialog.font_family_buffer.clone(),
                        dialog.font_size_buffer,
                    );
                    if font_changed_before != font_changed_after {
                        FontProvider::register_editor_font(
                            ctx,
                            &dialog.font_family_buffer,
                            dialog.font_size_buffer,
                        );
                    }
                }
                SettingsTab::Image => {
                    // === 图片设置 UI ===
                    ui.label("默认存储策略：");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut dialog.strategy_buffer, 0, "本地");
                        ui.selectable_value(&mut dialog.strategy_buffer, 1, "Base64");
                        ui.selectable_value(&mut dialog.strategy_buffer, 2, "SM.MS");
                    });
                    ui.add_space(8.0);

                    // 本地目录
                    ui.label("本地图片目录：");
                    ui.text_edit_singleline(&mut dialog.local_dir_buffer);
                    ui.add_space(8.0);

                    // SM.MS Token
                    ui.label("SM.MS API Token：");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut dialog.smms_token_buffer);
                        if ui.button("获取 Token").clicked() {
                            let _ = open::that("https://sm.ms/home/apitoken");
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("无 Token 也可上传，但有数量限制。注册后在网站获取。")
                            .weak()
                            .size(12.0),
                    );
                }
            }

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("保存").clicked() {
                    // CSS 设置
                    app_config.custom_css = if new_css.trim().is_empty() {
                        None
                    } else {
                        Some(new_css.clone())
                    };
                    // 字体设置
                    app_config.editor_font.family = dialog.font_family_buffer.clone();
                    app_config.editor_font.size = dialog.font_size_buffer;
                    // 图片设置
                    app_config.image_hosting.default_strategy = match dialog.strategy_buffer {
                        1 => ImageStrategy::Base64,
                        2 => ImageStrategy::SmMs,
                        _ => ImageStrategy::Local,
                    };
                    app_config.image_hosting.local_dir = dialog.local_dir_buffer.clone();
                    app_config.image_hosting.smms.api_token = dialog.smms_token_buffer.clone();

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
        dialog.open_dialog(
            Some("h1{color:red}"),
            &Default::default(),
            &EditorFontConfig::default(),
        );
        assert!(dialog.open);
        assert_eq!(dialog.css_buffer, "h1{color:red}");
        assert_eq!(dialog.local_dir_buffer, "images");
    }

    #[test]
    fn open_with_none_sets_empty_buffer() {
        let mut dialog = SettingsDialog::default();
        dialog.open_dialog(None, &Default::default(), &EditorFontConfig::default());
        assert!(dialog.open);
        assert_eq!(dialog.css_buffer, "");
        assert_eq!(dialog.local_dir_buffer, "images");
    }

    #[test]
    fn default_dialog_is_closed() {
        let dialog = SettingsDialog::default();
        assert!(!dialog.open);
        assert_eq!(dialog.css_buffer, "");
        assert_eq!(dialog.local_dir_buffer, "images");
        assert_eq!(dialog.strategy_buffer, 0);
    }

    #[test]
    fn open_dialog_populates_font_buffers() {
        let mut dialog = SettingsDialog::default();
        let font_config = EditorFontConfig {
            family: "Fira Code".into(),
            size: 16.0,
        };
        dialog.open_dialog(None, &Default::default(), &font_config);
        assert!(dialog.open);
        assert_eq!(dialog.font_family_buffer, "Fira Code");
        assert!((dialog.font_size_buffer - 16.0).abs() < f32::EPSILON);
    }

    #[test]
    fn open_dialog_default_font_buffers() {
        let mut dialog = SettingsDialog::default();
        dialog.open_dialog(None, &Default::default(), &EditorFontConfig::default());
        assert_eq!(dialog.font_family_buffer, "monospace");
        assert!((dialog.font_size_buffer - 14.0).abs() < f32::EPSILON);
    }
}
