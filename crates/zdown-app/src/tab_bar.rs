//! 标签栏 UI：显示打开的文件标签页，支持切换、关闭、脏标记和右键菜单。

use eframe::egui;

use crate::editor_state::EditorState;
use crate::menu::{ConfirmDialog, PendingAction};

/// 渲染标签栏（位于菜单栏下方）。
#[allow(deprecated)]
pub fn show_tab_bar(ui: &mut egui::Ui, state: &mut EditorState, confirm: &mut ConfirmDialog) {
    egui::TopBottomPanel::top("tab_bar").show_inside(ui, |ui| {
        let active = state.active_tab_index();
        let tab_count = state.tab_count();

        egui::ScrollArea::horizontal()
            .id_salt("tab_scroll")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut close_request: Option<usize> = None;
                    let mut switch_to: Option<usize> = None;

                    for i in 0..tab_count {
                        let is_active = i == active;
                        let dirty = state.tab_is_dirty(i);
                        let title = state.tab_title(i);
                        let has_path = state.tabs()[i].path.is_some();

                        // 标签按钮
                        let tab_text = if dirty {
                            format!("{title} *")
                        } else {
                            title.clone()
                        };

                        let tab_btn = if is_active {
                            egui::SelectableLabel::new(
                                true,
                                egui::RichText::new(&tab_text).strong(),
                            )
                        } else {
                            egui::SelectableLabel::new(false, &tab_text)
                        };

                        let response = ui.add(tab_btn);

                        // 点击标签页 → 切换
                        if response.clicked() {
                            switch_to = Some(i);
                        }

                        // 右键菜单
                        let tab_idx = i;
                        let has_path_for_menu = has_path;
                        response.context_menu(|ui| {
                            if ui.button("关闭其他").clicked() {
                                state.close_other_tabs(tab_idx);
                                ui.close();
                            }
                            if state.tab_count() > tab_idx + 1 && ui.button("关闭右侧").clicked()
                            {
                                state.close_tabs_to_right(tab_idx);
                                ui.close();
                            }
                            if has_path_for_menu && ui.button("复制路径").clicked() {
                                if let Some(ref path) = state.tabs()[tab_idx].path {
                                    ui.ctx().copy_text(path.display().to_string());
                                }
                                ui.close();
                            }
                        });

                        // 关闭按钮
                        let close_response =
                            ui.add(egui::Label::new("×").sense(egui::Sense::click()));
                        if close_response.clicked() {
                            close_request = Some(i);
                        }
                    }

                    // 在循环外观望处理请求，避免借用冲突
                    if let Some(i) = switch_to {
                        state.switch_tab(i);
                    }
                    if let Some(i) = close_request {
                        if state.tab_is_dirty(i) {
                            confirm.pending = Some(PendingAction::CloseTab(i));
                        } else {
                            let removed = state.close_tab(i);
                            if !removed {
                                state.new_file();
                            }
                        }
                    }
                });
            });
    });
}
