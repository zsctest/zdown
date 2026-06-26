//! 菜单栏 + 快捷键 + 未保存提示对话框。

use eframe::egui;
use fluent_bundle::FluentArgs;
use i18n::I18n;

use config::{AppConfig, ImageHostingConfig, ThemeMode};
use terminal_panel::TerminalPanel;

use crate::editor_state::EditorState;
use crate::settings_dialog::SettingsDialog;
use crate::settings_dialog::key_from_name;
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
#[allow(deprecated, clippy::too_many_arguments)]
pub fn show_menu(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
    view_mode: &mut ViewMode,
    settings_dialog: &mut SettingsDialog,
    app_config: &AppConfig,
    theme: &mut ThemeMode,
    _image_hosting: &ImageHostingConfig,
    i18n: &I18n,
    terminal: &mut TerminalPanel,
    file_tree: &mut crate::file_tree::FileTreeState,
) {
    egui::TopBottomPanel::top("menu").show_inside(ui, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button(i18n.t("menu-file"), |ui| {
                if ui.button(i18n.t("menu-file-new")).clicked() {
                    state.new_file();
                }
                if ui.button(i18n.t("menu-file-open")).clicked() {
                    trigger_open(state, i18n);
                }
                if ui.button(i18n.t("menu-file-open-folder")).clicked() {
                    trigger_open_folder(file_tree, i18n);
                }
                if ui.button(i18n.t("menu-file-save")).clicked() {
                    if state.current_path().is_none() {
                        trigger_save_as(state, i18n);
                    } else {
                        let _ = state.save();
                    }
                    state.run_spell_check(app_config.spell_check_enabled);
                }
                if ui.button(i18n.t("menu-file-save-as")).clicked() {
                    trigger_save_as(state, i18n);
                    state.run_spell_check(app_config.spell_check_enabled);
                }
                if ui.button(i18n.t("menu-file-save-all")).clicked() {
                    let (saved, skipped) = state.save_all();
                    if skipped > 0 {
                        let mut args = FluentArgs::new();
                        args.set("saved", saved as i64);
                        args.set("skipped", skipped as i64);
                        state.status_message = i18n.tr("status-save-skipped", Some(&args));
                    } else {
                        let mut args = FluentArgs::new();
                        args.set("saved", saved as i64);
                        state.status_message = i18n.tr("status-save-result", Some(&args));
                    }
                    state.run_spell_check(app_config.spell_check_enabled);
                }
                ui.separator();
                if ui.button(i18n.t("menu-file-export-pdf")).clicked() {
                    trigger_export_pdf(state, i18n);
                }
                if ui.button(i18n.t("menu-file-export-html")).clicked() {
                    trigger_export_html(state, app_config, i18n);
                }

                ui.separator();

                // 最近文件子菜单
                ui.menu_button(i18n.t("menu-file-recent"), |ui| {
                    if state.recent.list().is_empty() {
                        ui.label(i18n.t("menu-file-recent-empty"));
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

                if ui.button(i18n.t("menu-file-settings")).clicked() {
                    settings_dialog.open_dialog(
                        app_config.custom_css.as_deref(),
                        &app_config.image_hosting,
                        app_config.spell_check_enabled,
                        &app_config.keymap,
                    );
                    ui.close();
                }

                ui.separator();

                if ui.button(i18n.t("menu-file-quit")).clicked() {
                    if state.any_dirty() {
                        confirm.pending = Some(PendingAction::Quit);
                    } else {
                        state.quit();
                    }
                }
            });

            ui.menu_button(i18n.t("menu-edit"), |ui| {
                if ui.button(i18n.t("menu-edit-undo")).clicked() {
                    let _ = state.undo();
                }
                if ui.button(i18n.t("menu-edit-redo")).clicked() {
                    let _ = state.redo();
                }
                ui.separator();
                if ui.button(i18n.t("menu-edit-insert-image")).clicked() {
                    trigger_browse_image(state, &app_config.image_hosting, i18n);
                    ui.close();
                }
            });

            // 视图菜单
            ui.menu_button(i18n.t("menu-view"), |ui| {
                if ui.button(i18n.t("menu-view-source")).clicked() {
                    *view_mode = ViewMode::Source;
                }
                if ui.button(i18n.t("menu-view-preview")).clicked() {
                    *view_mode = ViewMode::Preview;
                }
                if ui.button(i18n.t("menu-view-hybrid")).clicked() {
                    *view_mode = ViewMode::Hybrid;
                }
                ui.separator();

                // 主题切换：显示可切换到的目标主题
                let toggle_label = match theme {
                    ThemeMode::Dark => i18n.t("menu-theme-light"),
                    ThemeMode::Light => i18n.t("menu-theme-dark"),
                };
                if ui.button(toggle_label).clicked() {
                    *theme = match theme {
                        ThemeMode::Dark => ThemeMode::Light,
                        ThemeMode::Light => ThemeMode::Dark,
                    };
                    ui.close();
                }

                ui.separator();

                if ui.button(i18n.t("menu-view-terminal")).clicked() {
                    terminal.toggle(&ui.ctx().clone());
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
    i18n: &I18n,
) {
    if let Some(pending) = confirm.pending.clone() {
        let title = match &pending {
            PendingAction::Quit => i18n.t("confirm-unsaved-title-quit"),
            PendingAction::CloseTab(_) => i18n.t("confirm-unsaved-title-close"),
        };
        let pending_clone = pending.clone();
        let mut action_taken = None;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(i18n.t("confirm-unsaved-body"));
                ui.horizontal(|ui| {
                    if ui.button(i18n.t("confirm-btn-save")).clicked() {
                        action_taken = Some("save");
                    }
                    if ui.button(i18n.t("confirm-btn-discard")).clicked() {
                        action_taken = Some("discard");
                    }
                    if ui.button(i18n.t("confirm-btn-cancel")).clicked() {
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
                        trigger_save_as(state, i18n);
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

fn trigger_open(state: &mut EditorState, i18n: &I18n) {
    let title = i18n.t("menu-file-open");
    if let Some(path) = workspace::pick_open_file(&title) {
        let _ = state.open(&path);
    }
}

fn trigger_open_folder(file_tree: &mut crate::file_tree::FileTreeState, i18n: &I18n) {
    let title = i18n.t("menu-file-open-folder");
    if let Some(path) = workspace::pick_folder(&title) {
        file_tree.open_folder(&path);
    }
}

fn trigger_save_as(state: &mut EditorState, i18n: &I18n) {
    let title = i18n.t("menu-file-save-as");
    if let Some(path) = workspace::pick_save_file(&title) {
        let _ = state.save_as(&path);
    }
}

fn trigger_export_pdf(state: &mut EditorState, i18n: &I18n) {
    let title = i18n.t("menu-file-export-pdf");
    if let Some(mut path) = workspace::pick_save_file_pdf(&title) {
        if path.extension().is_none_or(|e| e != "pdf") {
            path.set_extension("pdf");
        }
        let config = export_engine::PdfConfig {
            working_dir: state
                .current_path()
                .and_then(|p| p.parent().map(|d| d.to_path_buf())),
            ..Default::default()
        };
        let doc = state.current_doc();
        match export_engine::generate_pdf(&doc, &config) {
            Ok(pdf_bytes) => {
                if let Err(e) = std::fs::write(&path, &pdf_bytes) {
                    tracing::error!("PDF 写入失败: {e}");
                    let mut args = FluentArgs::new();
                    args.set("error", e.to_string());
                    state.status_message = i18n.tr("status-pdf-failed", Some(&args));
                } else {
                    tracing::info!("PDF 导出成功: {}", path.display());
                    let mut args = FluentArgs::new();
                    args.set("path", path.display().to_string());
                    state.status_message = i18n.tr("status-pdf-success", Some(&args));
                    state.recent.add(path);
                }
            }
            Err(e) => {
                tracing::error!("PDF 生成失败: {e}");
                let mut args = FluentArgs::new();
                args.set("error", e.to_string());
                state.status_message = i18n.tr("status-pdf-failed", Some(&args));
            }
        }
    }
}

fn trigger_export_html(state: &mut EditorState, app_config: &AppConfig, i18n: &I18n) {
    let title = i18n.t("menu-file-export-html");
    if let Some(mut path) = workspace::pick_save_file_html(&title) {
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
                    let mut args = FluentArgs::new();
                    args.set("error", e.to_string());
                    state.status_message = i18n.tr("status-html-failed", Some(&args));
                } else {
                    tracing::info!("HTML 导出成功: {}", path.display());
                    let mut args = FluentArgs::new();
                    args.set("path", path.display().to_string());
                    state.status_message = i18n.tr("status-html-success", Some(&args));
                    state.recent.add(path);
                }
            }
            Err(e) => {
                tracing::error!("HTML 生成失败: {e}");
                let mut args = FluentArgs::new();
                args.set("error", e.to_string());
                state.status_message = i18n.tr("status-html-failed", Some(&args));
            }
        }
    }
}

/// 浏览选择图片文件，按默认策略插入到编辑器。
pub(crate) fn trigger_browse_image(
    state: &mut EditorState,
    config: &ImageHostingConfig,
    i18n: &I18n,
) {
    let title = i18n.t("menu-edit-insert-image");
    let path = match workspace::pick_open_image(&title) {
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
            let mut args = FluentArgs::new();
            args.set("error", e.to_string());
            state.status_message = i18n.tr("status-image-read-failed", Some(&args));
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
                state.status_message = i18n.t("status-image-insert-failed");
            }
        }
        Err(e) => {
            let mut args = FluentArgs::new();
            args.set("error", e.to_string());
            state.status_message = i18n.tr("status-image-store-failed", Some(&args));
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
    actual.ctrl == expected.ctrl && actual.shift == expected.shift && actual.alt == expected.alt
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
                // No i18n available here — trigger_save_as needs it, but shortcuts
                // don't have i18n context yet. Title will default to action name.
                // This is acceptable for menu-file-save-as key since we use
                // the same translated action name approach via menu handler.
                // For now, pass an empty title (untitled.md as fallback).
                if let Some(path) = workspace::pick_save_file("Save Markdown File") {
                    let _ = state.save_as(&path);
                }
            }
            state.run_spell_check(app_config.spell_check_enabled);
        }
        config::Action::SaveAs => {
            if let Some(path) = workspace::pick_save_file("Save Markdown File") {
                let _ = state.save_as(&path);
            }
            state.run_spell_check(app_config.spell_check_enabled);
        }
        config::Action::NewFile => {
            state.new_file();
        }
        config::Action::Open => {
            if let Some(path) = workspace::pick_open_file("Open Markdown File") {
                let _ = state.open(&path);
            }
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
