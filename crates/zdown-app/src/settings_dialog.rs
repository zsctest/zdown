//! 设置对话框：自定义 CSS 编辑器 + 图片托管设置。
//!
//! 提供 `egui::Window` 模态对话框，允许用户：
//! - 编辑全局自定义 CSS（样式标签页）
//! - 配置图片存储策略、本地目录、SM.MS Token（图片标签页）
//!   保存后写入 `config::AppConfig::save()`。

use config::{AppConfig, ImageHostingConfig, ImageStrategy};
use eframe::egui;

/// 设置对话框标签页。
#[derive(Debug, Clone, Copy, PartialEq)]
enum SettingsTab {
    Css,
    Image,
    Spell,
    Keybind,
}

/// 按键捕获状态。
#[derive(Debug, Clone)]
struct KeybindingCapture {
    /// 正在重新绑定的 action。
    action: config::Action,
}

/// 设置对话框状态。
#[derive(Debug, Clone)]
pub struct SettingsDialog {
    /// 对话框是否打开。
    pub open: bool,
    active_tab: SettingsTab,
    /// 用户正在编辑的 CSS 文本缓冲区。
    css_buffer: String,
    /// 图片设置缓冲区
    local_dir_buffer: String,
    smms_token_buffer: String,
    strategy_buffer: usize, // 0=Local, 1=Base64, 2=SmMs
    /// 拼写检查开关缓冲区。
    spell_check_buffer: bool,
    /// 快捷键映射的可变副本（编辑缓存）。
    keymap_buffer: config::Keymap,
    /// 当前按键捕获状态。
    key_capture: Option<KeybindingCapture>,
}

impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            active_tab: SettingsTab::Css,
            css_buffer: String::new(),
            local_dir_buffer: "images".to_string(),
            smms_token_buffer: String::new(),
            strategy_buffer: 0,
            spell_check_buffer: true,
            keymap_buffer: config::Keymap::default(),
            key_capture: None,
        }
    }
}

impl SettingsDialog {
    /// 打开对话框，将当前配置填充到编辑缓冲区。
    pub fn open_dialog(
        &mut self,
        current_css: Option<&str>,
        image_config: &ImageHostingConfig,
        spell_check_enabled: bool,
        keymap: &config::Keymap,
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
        self.spell_check_buffer = spell_check_enabled;
        self.keymap_buffer = keymap.clone();
        self.key_capture = None;
    }
}

/// 将 egui::Key 转为 key_name 字符串。
fn key_name_from_egui(key: egui::Key) -> Option<String> {
    use egui::Key;
    let name = match key {
        Key::A => "A",
        Key::B => "B",
        Key::C => "C",
        Key::D => "D",
        Key::E => "E",
        Key::F => "F",
        Key::G => "G",
        Key::H => "H",
        Key::I => "I",
        Key::J => "J",
        Key::K => "K",
        Key::L => "L",
        Key::M => "M",
        Key::N => "N",
        Key::O => "O",
        Key::P => "P",
        Key::Q => "Q",
        Key::R => "R",
        Key::S => "S",
        Key::T => "T",
        Key::U => "U",
        Key::V => "V",
        Key::W => "W",
        Key::X => "X",
        Key::Y => "Y",
        Key::Z => "Z",
        Key::Num0 => "Num0",
        Key::Num1 => "Num1",
        Key::Num2 => "Num2",
        Key::Num3 => "Num3",
        Key::Num4 => "Num4",
        Key::Num5 => "Num5",
        Key::Num6 => "Num6",
        Key::Num7 => "Num7",
        Key::Num8 => "Num8",
        Key::Num9 => "Num9",
        Key::Tab => "Tab",
        Key::Space => "Space",
        Key::Enter => "Enter",
        Key::Backspace => "Backspace",
        Key::Delete => "Delete",
        Key::Escape => "Escape",
        Key::ArrowUp => "ArrowUp",
        Key::ArrowDown => "ArrowDown",
        Key::ArrowLeft => "ArrowLeft",
        Key::ArrowRight => "ArrowRight",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::F1 => "F1",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",
        Key::Minus => "Minus",
        Key::Equals => "Equals",
        Key::Comma => "Comma",
        Key::Period => "Period",
        Key::Slash => "Slash",
        Key::Backslash => "Backslash",
        Key::OpenBracket => "OpenBracket",
        Key::CloseBracket => "CloseBracket",
        Key::Semicolon => "Semicolon",
        Key::Quote => "Quote",
        _ => return None,
    };
    Some(name.into())
}

/// 将 key_name 字符串转换回 egui::Key（key_name_from_egui 的逆向映射）。
pub(crate) fn key_from_name(name: &str) -> Option<egui::Key> {
    use egui::Key;
    Some(match name {
        "A" => Key::A,
        "B" => Key::B,
        "C" => Key::C,
        "D" => Key::D,
        "E" => Key::E,
        "F" => Key::F,
        "G" => Key::G,
        "H" => Key::H,
        "I" => Key::I,
        "J" => Key::J,
        "K" => Key::K,
        "L" => Key::L,
        "M" => Key::M,
        "N" => Key::N,
        "O" => Key::O,
        "P" => Key::P,
        "Q" => Key::Q,
        "R" => Key::R,
        "S" => Key::S,
        "T" => Key::T,
        "U" => Key::U,
        "V" => Key::V,
        "W" => Key::W,
        "X" => Key::X,
        "Y" => Key::Y,
        "Z" => Key::Z,
        "Num0" => Key::Num0,
        "Num1" => Key::Num1,
        "Num2" => Key::Num2,
        "Num3" => Key::Num3,
        "Num4" => Key::Num4,
        "Num5" => Key::Num5,
        "Num6" => Key::Num6,
        "Num7" => Key::Num7,
        "Num8" => Key::Num8,
        "Num9" => Key::Num9,
        "Tab" => Key::Tab,
        "Space" => Key::Space,
        "Enter" => Key::Enter,
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "Escape" => Key::Escape,
        "ArrowUp" => Key::ArrowUp,
        "ArrowDown" => Key::ArrowDown,
        "ArrowLeft" => Key::ArrowLeft,
        "ArrowRight" => Key::ArrowRight,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "F1" => Key::F1,
        "F2" => Key::F2,
        "F3" => Key::F3,
        "F4" => Key::F4,
        "F5" => Key::F5,
        "F6" => Key::F6,
        "F7" => Key::F7,
        "F8" => Key::F8,
        "F9" => Key::F9,
        "F10" => Key::F10,
        "F11" => Key::F11,
        "F12" => Key::F12,
        "Minus" => Key::Minus,
        "Equals" => Key::Equals,
        "Comma" => Key::Comma,
        "Period" => Key::Period,
        "Slash" => Key::Slash,
        "Backslash" => Key::Backslash,
        "OpenBracket" => Key::OpenBracket,
        "CloseBracket" => Key::CloseBracket,
        "Semicolon" => Key::Semicolon,
        "Quote" => Key::Quote,
        _ => return None,
    })
}

/// 处理按键捕获：在设置对话框快捷键标签页中消费按键事件。
fn handle_keybinding_capture(ctx: &egui::Context, dialog: &mut SettingsDialog) {
    let Some(capture) = dialog.key_capture.as_ref() else {
        return;
    };
    let capture_action = capture.action;
    let mods = ctx.input(|i| i.modifiers);

    // 至少需要一个修饰键（纯字母键不绑定）
    if !mods.ctrl && !mods.shift && !mods.alt {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            dialog.key_capture = None;
        }
        return;
    }

    // 扫描按键事件
    let events = ctx.input(|i| i.events.clone());
    for event in &events {
        if let egui::Event::Key {
            key, pressed: true, ..
        } = event
        {
            // 忽略 Escape 键（取消捕获）
            if *key == egui::Key::Escape {
                dialog.key_capture = None;
                continue;
            }

            if let Some(key_name) = key_name_from_egui(*key) {
                // 再次确认有修饰键
                if !mods.ctrl && !mods.shift && !mods.alt {
                    continue;
                }

                let new_binding = config::KeyBinding {
                    modifiers: config::Modifiers {
                        ctrl: mods.ctrl,
                        shift: mods.shift,
                        alt: mods.alt,
                    },
                    key_name,
                };

                let conflict = dialog
                    .keymap_buffer
                    .detect_conflict(&capture_action, &new_binding);

                dialog
                    .keymap_buffer
                    .set_override(capture_action, new_binding);
                // 清除捕获状态，让 UI 立即显示绑定结果
                // 冲突检测在 Grid 渲染中实时处理
                dialog.key_capture = None;
            }
            break;
        }
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

    handle_keybinding_capture(ctx, dialog);

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
                ui.selectable_value(&mut dialog.active_tab, SettingsTab::Image, "图片");
                ui.selectable_value(&mut dialog.active_tab, SettingsTab::Spell, "拼写");
                ui.selectable_value(&mut dialog.active_tab, SettingsTab::Keybind, "快捷键");
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
                                "/* 在此输入自定义 CSS，例如 */\nh1 { color: #2196F3; }\nbody { max-width: 900px; }",
                            ),
                    );
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
                SettingsTab::Spell => {
                    ui.label("英文拼写检查：");
                    ui.add_space(4.0);

                    ui.checkbox(&mut dialog.spell_check_buffer, "启用英文拼写检查");

                    ui.add_space(8.0);

                    ui.label(
                        egui::RichText::new("拼写检查在保存文件时自动执行。")
                            .weak()
                            .size(12.0),
                    );
                    ui.label(
                        egui::RichText::new("错误单词将以红色波浪下划线标记。")
                            .weak()
                            .size(12.0),
                    );

                    ui.add_space(8.0);

                    ui.label(
                        egui::RichText::new("词典：English (United States) \u{2014} en_US")
                            .weak()
                            .size(12.0),
                    );
                }
                SettingsTab::Keybind => {
                    ui.horizontal(|ui| {
                        ui.label("点击快捷键单元格后按下新组合键，Esc 取消");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("恢复全部默认").clicked() {
                                dialog.keymap_buffer.clear_all();
                                dialog.key_capture = None;
                            }
                        });
                    });
                    ui.add_space(4.0);

                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(ui, |ui| {
                            egui::Grid::new("keybind_grid")
                                .striped(true)
                                .min_col_width(120.0)
                                .show(ui, |ui| {
                                    // 表头
                                    ui.label(egui::RichText::new("操作").strong());
                                    ui.label(egui::RichText::new("快捷键").strong());
                                    ui.label("");
                                    ui.end_row();

                                    for action in config::Action::all() {
                                        let binding = dialog.keymap_buffer.resolve(action);
                                        let is_capturing = dialog
                                            .key_capture
                                            .as_ref()
                                            .is_some_and(|c| c.action == *action);

                                        // 冲突检测
                                        let has_conflict = dialog
                                            .keymap_buffer
                                            .detect_conflict(action, &binding)
                                            .is_some();

                                        ui.label(action.display_name());

                                        // 快捷键单元格
                                        let cell_text = if is_capturing {
                                            "\u{2318} 按下新快捷键..."
                                        } else {
                                            &binding.display()
                                        };

                                        let cell_rich = if is_capturing {
                                            egui::RichText::new(cell_text)
                                                .color(egui::Color32::from_rgb(100, 200, 255))
                                                .strong()
                                        } else if has_conflict {
                                            egui::RichText::new(format!("\u{26A0} {}", cell_text))
                                                .color(egui::Color32::RED)
                                        } else {
                                            egui::RichText::new(cell_text).monospace()
                                        };

                                        if ui
                                            .add(egui::Button::new(cell_rich).min_size(egui::vec2(160.0, 0.0)))
                                            .clicked()
                                        {
                                            dialog.key_capture = Some(KeybindingCapture {
                                                action: *action,
                                            });
                                        }

                                        // 恢复按钮
                                        let is_modified = dialog.keymap_buffer.overrides.contains_key(action);
                                        if is_modified {
                                            if ui.button("\u{21B6}").on_hover_text("恢复默认").clicked() {
                                                dialog.keymap_buffer.clear_override(action);
                                                if is_capturing {
                                                    dialog.key_capture = None;
                                                }
                                            }
                                        } else {
                                            ui.label("");
                                        }

                                        ui.end_row();
                                    }
                                });
                        });
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
                    // 图片设置
                    app_config.image_hosting.default_strategy = match dialog.strategy_buffer {
                        1 => ImageStrategy::Base64,
                        2 => ImageStrategy::SmMs,
                        _ => ImageStrategy::Local,
                    };
                    app_config.image_hosting.local_dir = dialog.local_dir_buffer.clone();
                    app_config.image_hosting.smms.api_token = dialog.smms_token_buffer.clone();

                    // 拼写检查设置
                    app_config.spell_check_enabled = dialog.spell_check_buffer;

                    // 快捷键设置
                    app_config.keymap = dialog.keymap_buffer.clone();

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
            true,
            &config::Keymap::default(),
        );
        assert!(dialog.open);
        assert_eq!(dialog.css_buffer, "h1{color:red}");
        assert_eq!(dialog.local_dir_buffer, "images");
    }

    #[test]
    fn open_with_none_sets_empty_buffer() {
        let mut dialog = SettingsDialog::default();
        dialog.open_dialog(None, &Default::default(), true, &config::Keymap::default());
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
    fn open_dialog_populates_spell_check_buffer() {
        let mut dialog = SettingsDialog::default();
        dialog.open_dialog(None, &Default::default(), false, &config::Keymap::default());
        assert!(dialog.open);
        assert!(!dialog.spell_check_buffer);
    }

    #[test]
    fn open_dialog_default_spell_check_enabled() {
        let mut dialog = SettingsDialog::default();
        dialog.open_dialog(None, &Default::default(), true, &config::Keymap::default());
        assert!(dialog.spell_check_buffer);
    }
}
