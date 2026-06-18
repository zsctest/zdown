//! 视图模式：源码 / 预览 / hybrid。

/// 视图模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// 源码编辑模式。
    #[default]
    Source,
    /// 预览模式（只读渲染）。
    Preview,
    /// Hybrid 模式（光标处源码，其余渲染）。
    Hybrid,
}

impl ViewMode {
    /// 中文名（菜单显示）。
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "源码",
            Self::Preview => "预览",
            Self::Hybrid => "Hybrid",
        }
    }
}
