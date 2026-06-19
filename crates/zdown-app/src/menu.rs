//! 菜单栏 + 快捷键 + 未保存提示对话框。

use eframe::egui;

use crate::editor_state::EditorState;
use crate::view_mode::ViewMode;

/// 待确认的操作类型（用户选 New/Open/Quit 但有未保存修改时）。
#[derive(Debug, Clone, PartialEq)]
pub enum PendingAction {
    New,
    Open,
    Quit,
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
) {
    egui::TopBottomPanel::top("menu").show_inside(ui, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("文件", |ui| {
                if ui.button("新建 (Ctrl+N)").clicked() {
                    if state.is_dirty() {
                        confirm.pending = Some(PendingAction::New);
                    } else {
                        state.new_file();
                    }
                }
                if ui.button("打开... (Ctrl+O)").clicked() {
                    if state.is_dirty() {
                        confirm.pending = Some(PendingAction::Open);
                    } else {
                        trigger_open(state);
                    }
                }
                if ui.button("保存 (Ctrl+S)").clicked() {
                    if state.current_path.is_none() {
                        trigger_save_as(state);
                    } else {
                        let _ = state.save();
                    }
                }
                if ui.button("另存为... (Ctrl+Shift+S)").clicked() {
                    trigger_save_as(state);
                }
                if ui.button("导出 PDF...").clicked() {
                    trigger_export_pdf(state);
                }
                if ui.button("导出 HTML...").clicked() {
                    trigger_export_html(state);
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

                if ui.button("退出").clicked() {
                    if state.is_dirty() {
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
            PendingAction::New => "未保存修改 - 新建",
            PendingAction::Open => "未保存修改 - 打开",
            PendingAction::Quit => "未保存修改 - 退出",
        };
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
                    if state.current_path.is_some() {
                        let _ = state.save();
                    } else {
                        trigger_save_as(state);
                    }
                    execute_pending(state, &pending);
                }
                "discard" => {
                    execute_pending(state, &pending);
                }
                _ => {}
            }
            confirm.pending = None;
        }
    }
}

fn execute_pending(state: &mut EditorState, pending: &PendingAction) {
    match pending {
        PendingAction::New => state.new_file(),
        PendingAction::Open => trigger_open(state),
        PendingAction::Quit => state.quit(),
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
                } else {
                    tracing::info!("PDF 导出成功: {}", path.display());
                    state.recent.add(path);
                }
            }
            Err(e) => {
                tracing::error!("PDF 生成失败: {e}");
            }
        }
    }
}

fn trigger_export_html(state: &mut EditorState) {
    if let Some(mut path) = workspace::pick_save_file_html() {
        if path.extension().is_none_or(|e| e != "html" && e != "htm") {
            path.set_extension("html");
        }
        let config = export_engine::HtmlConfig {
            title: state
                .current_path
                .as_ref()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            ..Default::default()
        };
        let doc = state.current_doc();
        match export_engine::generate_html(&doc, &config) {
            Ok(html_str) => {
                if let Err(e) = std::fs::write(&path, &html_str) {
                    tracing::error!("HTML 写入失败: {e}");
                } else {
                    tracing::info!("HTML 导出成功: {}", path.display());
                    state.recent.add(path);
                }
            }
            Err(e) => {
                tracing::error!("HTML 生成失败: {e}");
            }
        }
    }
}

/// 处理快捷键。
pub fn handle_shortcuts(ctx: &egui::Context, state: &mut EditorState, confirm: &mut ConfirmDialog) {
    let mods = ctx.input(|i| i.modifiers);

    // Ctrl+S
    if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::S)) {
        if state.current_path.is_some() {
            let _ = state.save();
        } else {
            trigger_save_as(state);
        }
    }
    // Ctrl+Shift+S
    if mods.ctrl && mods.shift && ctx.input(|i| i.key_pressed(egui::Key::S)) {
        trigger_save_as(state);
    }
    // Ctrl+N
    if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::N)) {
        if state.is_dirty() {
            confirm.pending = Some(PendingAction::New);
        } else {
            state.new_file();
        }
    }
    // Ctrl+O
    if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::O)) {
        if state.is_dirty() {
            confirm.pending = Some(PendingAction::Open);
        } else {
            trigger_open(state);
        }
    }
    // Ctrl+Z
    if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::Z)) {
        let _ = state.undo();
    }
    // Ctrl+Y 或 Ctrl+Shift+Z
    if mods.ctrl
        && ((!mods.shift && ctx.input(|i| i.key_pressed(egui::Key::Y)))
            || (mods.shift && ctx.input(|i| i.key_pressed(egui::Key::Z))))
    {
        let _ = state.redo();
    }
}
