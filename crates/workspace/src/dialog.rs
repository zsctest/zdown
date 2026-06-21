//! rfd 文件对话框封装（同步 API）。
//!
//! 在 headless 环境（CI 无 DISPLAY）返回 `None`，不 panic。
//!
//! 所有 pick_* 函数接受翻译后的标题字符串，由调用方通过 i18n 提供。

use std::path::PathBuf;

/// 弹出打开文件对话框。用户取消或环境不支持时返回 `None`。
pub fn pick_open_file(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Markdown", &["md", "markdown"])
        .set_title(title)
        .pick_file()
}

/// 弹出保存文件对话框。用户取消或环境不支持时返回 `None`。
pub fn pick_save_file(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Markdown", &["md", "markdown"])
        .set_title(title)
        .set_file_name("untitled.md")
        .save_file()
}

/// 弹出 PDF 导出保存对话框。用户取消或环境不支持时返回 `None`。
pub fn pick_save_file_pdf(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("PDF", &["pdf"])
        .set_title(title)
        .set_file_name("untitled.pdf")
        .save_file()
}

/// 弹出 HTML 导出保存对话框。用户取消或环境不支持时返回 `None`。
pub fn pick_save_file_html(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("HTML", &["html", "htm"])
        .set_title(title)
        .set_file_name("untitled.html")
        .save_file()
}

/// 弹出打开图片文件对话框。用户取消或环境不支持时返回 `None`。
pub fn pick_open_image(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Image", &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"])
        .set_title(title)
        .pick_file()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    /// CI 无显示环境，pick_* 应返回 None 而非 panic。
    /// 本地手动运行时（有 DISPLAY）可能弹窗，测试会阻塞——
    /// 因此本测试标记 ignored，仅手动 `cargo test -- --ignored` 验证。
    #[test]
    #[ignore = "需要手动在桌面环境验证对话框弹窗"]
    fn pick_open_file_does_not_panic() {
        let _ = pick_open_file("Open Markdown File");
    }

    #[test]
    #[ignore = "需要手动在桌面环境验证对话框弹窗"]
    fn pick_save_file_does_not_panic() {
        let _ = pick_save_file("Save Markdown File");
    }
}
