//! 菜单栏 + 快捷键 + 未保存提示对话框。

use eframe::egui;

use config::{AppConfig, ImageHostingConfig, ThemeMode};

use crate::editor_state::EditorState;
use crate::settings_dialog::key_from_name;
use crate::settings_dialog::SettingsDialog;
use crate::view_mode::ViewMode;

/// 待确认的操作类型（用户选 New/Open/Quit 但有未保存修改时）。
#[derive(Debug, Clone, PartialEq)]
pub enum PendingAction {
    Quit,
    /// 关闭标签页（带未保存提示）。
    CloseTab(usize),
}

/// UI 状态：是否显示未保存确认对话框 + 待确认操作。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConfirmDialog {
    pub pending: Option<PendingAction>,
}

impl ConfirmDialog {
    #[allow(dead_code)]
    pub fn is_open(&self) -> bool {
        self.pending.is_some()
    }
}

/// 渲染菜单栏。
#[allow(deprecated)]
pub fn show_menu(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
    view_mode: &mut ViewMode,
    settings_dialog: &mut SettingsDialog,
    app_config: &AppConfig,
    theme: &mut ThemeMode,
    _image_hosting: &ImageHostingConfig,
) {
    egui::TopBottomPanel::top("menu").show_inside(ui, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("文件", |ui| {
                if ui.button("新建 (Ctrl+N)").clicked() {
                    state.new_file();
                }
                if ui.button("打开... (Ctrl+O)").clicked() {
                    trigger_open(state);
                }
                if ui.button("保存 (Ctrl+S)").clicked() {
                    if state.current_path().is_none() {
                        trigger_save_as(state);
                    } else {
                        let _ = state.save();
                    }
                    state.run_spell_check(app_config.spell_check_enabled);
                }
                if ui.button("另存为... (Ctrl+Shift+S)").clicked() {
                    trigger_save_as(state);
                    state.run_spell_check(app_config.spell_check_enabled);
                }
                if ui.button("保存所有").clicked() {
                    let (saved, skipped) = state.save_all();
                    let mut msg = format!("保存完成：{saved} 个文件");
                    if skipped > 0 {
                        msg.push_str(&format!("，{skipped} 个未命名文件已跳过"));
                    }
                    state.status_message = msg;
                    state.run_spell_check(app_config.spell_check_enabled);
                }
                ui.separator();
                if ui.button("导出 PDF...").clicked() {
                    trigger_export_pdf(state);
                }
                if ui.button("导出 HTML...").clicked() {
                    trigger_export_html(state, app_config);
                }

                ui.separator();

                // 最近文件子菜单
                ui.menu_button("最近文件", |ui| {
                    if state.recent.list().is_empty() {
                        ui.label("(无)");
                    } else {
                        for path in state.recent.list().to_vec() {
                            if ui.button(path.display().to_string()).clicked() {
                                let _ = state.open(&path);
                                ui.close();
                            }
                        }
                    }
                });

                ui.separator();

                if ui.button("设置...").clicked() {
                    settings_dialog.open_dialog(
                        app_config.custom_css.as_deref(),
                        &app_config.image_hosting,
                        app_config.spell_check_enabled,
                        &app_config.keymap,
                    );
                    ui.close();
                }

                ui.separator();

                if ui.button("退出").clicked() {
                    if state.any_dirty() {
                        confirm.pending = Some(PendingAction::Quit);
                    } else {
                        state.quit();
                    }
                }
            });

            ui.menu_button("编辑", |ui| {
                if ui.button("撤销 (Ctrl+Z)").clicked() {
                    let _ = state.undo();
                }
                if ui.button("重做 (Ctrl+Y)").clicked() {
                    let _ = state.redo();
                }
                ui.separator();
                if ui.button("插入图片... (Ctrl+I)").clicked() {
                    trigger_browse_image(state, &app_config.image_hosting);
                    ui.close();
                }
            });

            // 视图菜单
            ui.menu_button("视图", |ui| {
                if ui.button("源码 (Ctrl+1)").clicked() {
                    *view_mode = ViewMode::Source;
                }
                if ui.button("预览 (Ctrl+2)").clicked() {
                    *view_mode = ViewMode::Preview;
                }
                if ui.button("Hybrid (Ctrl+3)").clicked() {
                    *view_mode = ViewMode::Hybrid;
                }
                ui.separator();

                // 主题切换：显示可切换到的目标主题
                let toggle_label = match theme {
                    ThemeMode::Dark => "\u{2600}\u{FE0F} 亮色主题",
                    ThemeMode::Light => "\u{1F319} 暗色主题",
                };
                if ui.button(toggle_label).clicked() {
                    *theme = match theme {
                        ThemeMode::Dark => ThemeMode::Light,
                        ThemeMode::Light => ThemeMode::Dark,
                    };
                    ui.close();
                }
            });
        });
    });
}

/// 渲染未保存确认对话框。
pub fn show_confirm_dialog(
    ctx: &egui::Context,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
) {
    if let Some(pending) = confirm.pending.clone() {
        let title = match &pending {
            PendingAction::Quit => "未保存修改 - 退出",
            PendingAction::CloseTab(_) => "未保存修改 - 关闭标签页",
        };
        let pending_clone = pending.clone();
        let mut action_taken = None;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("当前文档有未保存修改。是否保存?");
                ui.horizontal(|ui| {
                    if ui.button("保存").clicked() {
                        action_taken = Some("save");
                    }
                    if ui.button("不保存").clicked() {
                        action_taken = Some("discard");
                    }
                    if ui.button("取消").clicked() {
                        action_taken = Some("cancel");
                    }
                });
            });

        if let Some(action) = action_taken {
            match action {
                "save" => {
                    if state.current_path().is_some() {
                        let _ = state.save();
                    } else {
                        trigger_save_as(state);
                    }
                    execute_pending(state, &pending_clone);
                }
                "discard" => {
                    execute_pending(state, &pending_clone);
                }
                _ => {}
            }
            confirm.pending = None;
        }
    }
}

fn execute_pending(state: &mut EditorState, pending: &PendingAction) {
    match pending {
        PendingAction::Quit => state.quit(),
        PendingAction::CloseTab(i) => {
            let removed = state.close_tab(*i);
            if !removed {
                state.new_file();
            }
        }
    }
}

fn trigger_open(state: &mut EditorState) {
    if let Some(path) = workspace::pick_open_file() {
        let _ = state.open(&path);
    }
}

fn trigger_save_as(state: &mut EditorState) {
    if let Some(path) = workspace::pick_save_file() {
        let _ = state.save_as(&path);
    }
}

fn trigger_export_pdf(state: &mut EditorState) {
    if let Some(mut path) = workspace::pick_save_file_pdf() {
        if path.extension().is_none_or(|e| e != "pdf") {
            path.set_extension("pdf");
        }
        let config = export_engine::PdfConfig::default();
        let doc = state.current_doc();
        match export_engine::generate_pdf(&doc, &config) {
            Ok(pdf_bytes) => {
                if let Err(e) = std::fs::write(&path, &pdf_bytes) {
                    tracing::error!("PDF 写入失败: {e}");
                    state.status_message = format!("PDF 导出失败: {e}");
                } else {
                    tracing::info!("PDF 导出成功: {}", path.display());
                    state.status_message = format!("PDF 已导出: {}", path.display());
                    state.recent.add(path);
                }
            }
            Err(e) => {
                tracing::error!("PDF 生成失败: {e}");
            }
        }
    }
}

fn trigger_export_html(state: &mut EditorState, app_config: &AppConfig) {
    if let Some(mut path) = workspace::pick_save_file_html() {
        if path.extension().is_none_or(|e| e != "html" && e != "htm") {
            path.set_extension("html");
        }
        let config = export_engine::HtmlConfig {
            title: state
                .current_path()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            css: app_config.custom_css.clone(),
            ..Default::default()
        };
        let doc = state.current_doc();
        match export_engine::generate_html(&doc, &config) {
            Ok(html_str) => {
                if let Err(e) = std::fs::write(&path, &html_str) {
                    tracing::error!("HTML 写入失败: {e}");
                    state.status_message = format!("HTML 导出失败: {e}");
                } else {
                    tracing::info!("HTML 导出成功: {}", path.display());
                    state.status_message = format!("HTML 已导出: {}", path.display());
                    state.recent.add(path);
                }
            }
            Err(e) => {
                tracing::error!("HTML 生成失败: {e}");
            }
        }
    }
}

/// 浏览选择图片文件，按默认策略插入到编辑器。
pub(crate) fn trigger_browse_image(state: &mut EditorState, config: &ImageHostingConfig) {
    let path = match workspace::pick_open_image() {
        Some(p) => p,
        None => return,
    };

    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image.png".to_string());

    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) => {
            state.status_message = format!("图片读取失败: {e}");
            return;
        }
    };

    let format = crate::image_hosting::ImageFormat::from_filename(&filename);
    let working_dir = state
        .current_path()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let storage = crate::image_hosting::create_storage(config, working_dir);

    match storage.store(&data, &filename, format) {
        Ok(url) => {
            let md_text = format!("![{filename}]({url})");
            let cursor = state.editor().cursor;
            if state
                .apply(editor_engine::Command::Insert {
                    pos: cursor,
                    text: md_text,
                })
                .is_err()
            {
                state.status_message = "图片插入失败".to_string();
            }
        }
        Err(e) => {
            state.status_message = format!("图片存储失败: {e}");
        }
    }
}

/// 处理快捷键（查表驱动：从 AppConfig.keymap 读取绑定）。
pub fn handle_shortcuts(
    ctx: &egui::Context,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
    view_mode: &mut ViewMode,
    theme: &mut ThemeMode,
    app_config: &AppConfig,
) {
    let mods = ctx.input(|i| i.modifiers);

    for action in config::Action::all() {
        let binding = app_config.keymap.resolve(action);
        if !mods_match(&mods, &binding.modifiers) {
            continue;
        }
        if let Some(key) = key_from_name(&binding.key_name) {
            if ctx.input(|i| i.key_pressed(key)) {
                execute_action(action, state, confirm, view_mode, theme, app_config);
            }
        }
    }
}

/// 判断 egui 修饰键是否匹配绑定要求的修饰键。
fn mods_match(actual: &egui::Modifiers, expected: &config::Modifiers) -> bool {
    actual.ctrl == expected.ctrl
        && actual.shift == expected.shift
        && actual.alt == expected.alt
}

/// 根据 action 分发执行具体操作。
fn execute_action(
    action: &config::Action,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
    view_mode: &mut ViewMode,
    theme: &mut ThemeMode,
    app_config: &AppConfig,
) {
    match action {
        config::Action::Save => {
            if state.current_path().is_some() {
                let _ = state.save();
            } else {
                trigger_save_as(state);
            }
            state.run_spell_check(app_config.spell_check_enabled);
        }
        config::Action::SaveAs => {
            trigger_save_as(state);
            state.run_spell_check(app_config.spell_check_enabled);
        }
        config::Action::NewFile => {
            state.new_file();
        }
        config::Action::Open => {
            trigger_open(state);
        }
        config::Action::CloseTab => {
            if state.tab_count() > 1 {
                let idx = state.active_tab_index();
                if state.tab_is_dirty(idx) {
                    confirm.pending = Some(PendingAction::CloseTab(idx));
                } else {
                    let removed = state.close_tab(idx);
                    if !removed {
                        state.new_file();
                    }
                }
            }
        }
        config::Action::NextTab => {
            state.next_tab();
        }
        config::Action::PrevTab => {
            state.prev_tab();
        }
        config::Action::MoveTabLeft => {
            let idx = state.active_tab_index();
            if idx > 0 {
                state.move_tab(idx, idx - 1);
            }
        }
        config::Action::MoveTabRight => {
            let idx = state.active_tab_index();
            state.move_tab(idx, idx + 1);
        }
        config::Action::Undo => {
            let _ = state.undo();
        }
        config::Action::Redo => {
            let _ = state.redo();
        }
        config::Action::ViewSource => {
            *view_mode = ViewMode::Source;
        }
        config::Action::ViewPreview => {
            *view_mode = ViewMode::Preview;
        }
        config::Action::ViewHybrid => {
            *view_mode = ViewMode::Hybrid;
        }
        config::Action::ToggleTheme => {
            *theme = match theme {
                ThemeMode::Dark => ThemeMode::Light,
                ThemeMode::Light => ThemeMode::Dark,
            };
        }
    }
}
