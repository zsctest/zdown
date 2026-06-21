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

/// 终端面板状态机。
pub struct TerminalPanel {
    /// 后端（PTY + Term 状态）。None 表示未启动。
    backend: Option<backend::TerminalBackend>,
    /// 终端可见性。
    pub visible: bool,
    /// 面板高度（像素）。
    pub height: f32,
    /// 错误信息（PTY 创建失败时设置）。
    pub error: Option<String>,
    /// 进程退出码。
    pub exit_code: Option<i32>,
}

impl TerminalPanel {
    /// 创建未启动的终端面板。
    pub fn new() -> Self {
        Self {
            backend: None,
            visible: false,
            height: 200.0,
            error: None,
            exit_code: None,
        }
    }
}

impl Default for TerminalPanel {
    fn default() -> Self {
        Self::new()
    }
}
