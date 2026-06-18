//! PDF 生成入口（阶段 3，后续任务实现）。

use crate::error::Error;
use crate::theme::PdfConfig;

/// 根据配置生成 PDF。
pub fn generate_pdf(_config: &PdfConfig) -> crate::Result<Vec<u8>> {
    Err(Error::Render("PDF 生成尚未实现".into()))
}
