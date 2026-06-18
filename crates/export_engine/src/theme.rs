//! PDF 主题与样式配置（阶段 3，后续任务实现）。

/// PDF 导出配置。
#[derive(Debug, Clone)]
pub struct PdfConfig {
    /// 页面宽度（mm）。
    pub page_width: f64,
    /// 页面高度（mm）。
    pub page_height: f64,
    /// 上边距（mm）。
    pub margin_top: f64,
    /// 下边距（mm）。
    pub margin_bottom: f64,
    /// 左边距（mm）。
    pub margin_left: f64,
    /// 右边距（mm）。
    pub margin_right: f64,
}

impl Default for PdfConfig {
    fn default() -> Self {
        Self {
            // A4
            page_width: 210.0,
            page_height: 297.0,
            margin_top: 20.0,
            margin_bottom: 20.0,
            margin_left: 20.0,
            margin_right: 20.0,
        }
    }
}
