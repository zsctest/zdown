//! 嵌入式终端面板。
//!
//! 基于 alacritty_terminal 的 VTE 解析 + portable-pty 的跨平台 PTY，
//! 通过 egui 0.34 的 Galley API 渲染终端网格。

pub mod backend;
pub mod bindings;
pub mod font;
pub mod shell;
pub mod theme;
pub mod view;

pub use view::TerminalView;

use crate::backend::TerminalBackend;
use crate::font::TerminalFont;
use crate::theme::TerminalTheme;

/// 终端面板状态机。
pub struct TerminalPanel {
    /// 后端（PTY + Term 状态）。None 表示未启动。
    backend: Option<TerminalBackend>,
    font: TerminalFont,
    theme: TerminalTheme,
    /// 终端可见性。
    pub visible: bool,
    /// 面板高度（像素）。
    pub height: f32,
    /// 错误信息（PTY 创建失败时设置）。
    pub error: Option<String>,
    /// 进程退出码。
    pub exit_code: Option<i32>,
    /// 是否需要在下一帧请求焦点。
    pub focus_requested: bool,
}

impl TerminalPanel {
    /// 创建未启动的终端面板。
    pub fn new() -> Self {
        Self {
            backend: None,
            font: TerminalFont::default(),
            theme: TerminalTheme::default(),
            visible: false,
            height: 200.0,
            error: None,
            exit_code: None,
            focus_requested: false,
        }
    }

    /// 切换终端显示/隐藏。
    pub fn toggle(&mut self, ctx: &egui::Context) {
        self.visible = !self.visible;
        if self.visible && self.backend.is_none() {
            let (shell, _) = shell::detect_shell();
            self.spawn(ctx, &shell, None);
        }
        if self.visible {
            self.focus_requested = true;
        }
    }

    /// 启动 PTY 进程。
    pub fn spawn(
        &mut self,
        ctx: &egui::Context,
        shell_program: &str,
        working_dir: Option<std::path::PathBuf>,
    ) {
        if self.backend.is_some() {
            return;
        }
        match TerminalBackend::spawn(ctx.clone(), shell_program, working_dir) {
            Ok(be) => {
                self.backend = Some(be);
                self.error = None;
                self.exit_code = None;
            }
            Err(e) => {
                self.error = Some(format!("终端启动失败: {e}"));
            }
        }
    }

    /// 面板是否存活（进程未退出）。
    pub fn is_alive(&self) -> bool {
        self.backend.as_ref().is_some_and(|be| be.is_alive())
    }

    /// 在 egui panel 中渲染终端。
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // 错误提示
        if let Some(ref e) = self.error.clone() {
            ui.vertical_centered(|ui| {
                ui.label(format!("❌ {e}"));
                if ui.button("重试").clicked() {
                    self.error = None;
                    self.backend = None;
                    let (shell, _) = shell::detect_shell();
                    self.spawn(ui.ctx(), &shell, None);
                }
            });
            return;
        }

        // 进程已退出
        if let Some(code) = self.exit_code {
            ui.vertical_centered(|ui| {
                ui.label(format!("进程已退出 (退出码: {code})"));
                if ui.button("重新启动 (Enter)").clicked() {
                    self.exit_code = None;
                    self.backend = None;
                    let (shell, _) = shell::detect_shell();
                    self.spawn(ui.ctx(), &shell, None);
                }
            });
            return;
        }

        // 渲染终端
        if let Some(ref mut be) = self.backend {
            let view = TerminalView::new(ui, be, self.font.clone(), self.theme.clone());
            let response = ui.add(view);

            // 焦点管理
            if self.focus_requested {
                response.request_focus();
                self.focus_requested = false;
            }
        }
    }
}

impl Default for TerminalPanel {
    fn default() -> Self {
        Self::new()
    }
}
