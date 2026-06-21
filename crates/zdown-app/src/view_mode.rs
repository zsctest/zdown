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
    /// 返回 FTL 翻译 key（由调用方通过 i18n 翻译）。
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "view-source",
            Self::Preview => "view-preview",
            Self::Hybrid => "view-hybrid",
        }
    }
}
