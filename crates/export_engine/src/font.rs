//! 字体加载：内嵌 TTF 后备 + 系统查找。
//!
//! 策略：
//! 1. ttf_data 有值 → 从内存加载
//! 2. 无值 → font-kit 系统查找匹配 name
//! 3. 查找失败 → 编译期内嵌 Noto Sans CJK SC 子集（fonts/ 下 .ttf）
//! 4. 全失败 → 返回 Error::FontLoad

use crate::Result;
use crate::error::Error;
use crate::theme::{FontConfig, PdfConfig};
use genpdf::fonts;

/// 一次加载的字体集合，renderer 各处复用。
pub struct FontSet {
    pub body: fonts::FontData,
    pub mono: fonts::FontData,
    pub heading: fonts::FontData,
    pub header_footer: fonts::FontData,
}

impl FontSet {
    /// 根据 PdfConfig 加载所有字体。
    pub fn load(config: &PdfConfig) -> Result<Self> {
        let body = load_font(&config.theme.body_font)?;
        let mono = load_font(&config.theme.mono_font)?;
        let heading = load_font(&config.theme.heading_font)?;
        // 页眉页脚复用正文字体
        let header_footer = body.clone();
        Ok(Self {
            body,
            mono,
            heading,
            header_footer,
        })
    }
}

fn load_font(config: &FontConfig) -> Result<fonts::FontData> {
    // 1. 内嵌 TTF
    if let Some(ref data) = config.ttf_data {
        return fonts::FontData::new(data.clone(), None)
            .map_err(|e| Error::FontLoad(format!("内嵌字体加载失败: {e}")));
    }
    // 2. 系统字体查找
    if let Some(data) = find_system_font(&config.name) {
        return fonts::FontData::new(data, None)
            .map_err(|e| Error::FontLoad(format!("系统字体加载失败: {e}")));
    }
    // 3. 编译期内嵌后备
    let fallback = get_fallback_ttf(&config.name);
    if !fallback.is_empty() {
        return fonts::FontData::new(fallback, None)
            .map_err(|e| Error::FontLoad(format!("后备字体加载失败: {e}")));
    }
    // 4. 全部失败
    Err(Error::FontLoad(format!(
        "无法加载字体 '{}'：无内嵌数据、系统未找到、无后备字体",
        config.name
    )))
}

/// 用 font-kit 在系统字体目录查找匹配 name 的字体文件。
fn find_system_font(name: &str) -> Option<Vec<u8>> {
    let source = font_kit::source::SystemSource::new();
    let handle = source
        .select_best_match(
            &[font_kit::family_name::FamilyName::Title(name.into())],
            font_kit::properties::Properties::new().style(font_kit::properties::Style::Normal),
        )
        .ok()?;
    match handle {
        font_kit::handle::Handle::Path { path, .. } => std::fs::read(path).ok(),
        font_kit::handle::Handle::Memory { bytes, .. } => Some((*bytes).clone()),
    }
}

/// 编译期内嵌的后备字体数据。
/// 当 `embed-fonts` feature 启用且字体文件存在时，
/// 返回对应 TTF 字节；否则返回空 Vec。
fn get_fallback_ttf(name: &str) -> Vec<u8> {
    let _ = name;
    #[cfg(feature = "embed-fonts")]
    {
        let name_lower = name.to_lowercase();
        // 正文/标题/常规 CJK 字体
        if name_lower.contains("cjk") || name_lower.contains("sans") {
            return include_bytes!("../fonts/NotoSansCJKsc-Regular-subset.ttf").to_vec();
        }
        // mono/等宽字体回退到同一个 CJK 字体（Noto Sans CJK SC 包含等宽英文/数字字形）
        if name_lower.contains("mono") {
            return include_bytes!("../fonts/NotoSansCJKsc-Regular-subset.ttf").to_vec();
        }
    }
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::PdfConfig;

    #[test]
    fn load_default_config_fonts() {
        let config = PdfConfig::default();
        let fonts = FontSet::load(&config);
        // 系统可能没有 Noto Sans CJK SC，但加载函数不应 panic
        match fonts {
            Ok(_) => {}
            Err(e) => {
                // 如果系统没有字体，应在 font-kit 查找失败时返回错误
                assert!(e.to_string().contains("无法加载字体"));
            }
        }
    }

    #[test]
    fn load_with_embedded_ttf() {
        let mut config = PdfConfig::default();
        let dummy_ttf = b"not a valid ttf font file".to_vec();
        config.theme.body_font.ttf_data = Some(dummy_ttf.clone());
        config.theme.mono_font.ttf_data = Some(dummy_ttf.clone());
        config.theme.heading_font.ttf_data = Some(dummy_ttf);
        let result = FontSet::load(&config);
        // 内嵌数据可能不是有效 TTF，genpdf 应给出 parse 错误
        assert!(result.is_err());
    }
}
