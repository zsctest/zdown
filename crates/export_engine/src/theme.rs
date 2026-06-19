//! PDF 导出主题配置。
//!
//! PdfConfig 提供 3 个 preset：default（内嵌字体/A4/浅色）、dark（暗色背景）、minimal（系统字体/省墨）。

/// PDF 导出总配置。
#[derive(Debug, Clone)]
pub struct PdfConfig {
    pub paper: Paper,
    pub margins: Margins,
    pub header_footer: HeaderFooter,
    pub theme: PdfTheme,
}

#[derive(Debug, Clone, Copy)]
pub enum Paper {
    A4,
    Letter,
    Custom { width_mm: f32, height_mm: f32 },
}

#[derive(Debug, Clone, Copy)]
pub struct Margins {
    /// 上边距（毫米）
    pub top: f32,
    /// 下边距（毫米）
    pub bottom: f32,
    /// 左边距（毫米）
    pub left: f32,
    /// 右边距（毫米）
    pub right: f32,
}

#[derive(Debug, Clone)]
pub struct HeaderFooter {
    /// 左模板，{file} {date} {page} {total} 占位
    pub left: String,
    /// 中模板
    pub center: String,
    /// 右模板
    pub right: String,
}

#[derive(Debug, Clone)]
pub struct PdfTheme {
    pub body_font: FontConfig,
    pub mono_font: FontConfig,
    pub heading_font: FontConfig,
    pub font_size: FontSizes,
    pub colors: ThemeColors,
    pub spacing: ThemeSpacing,
    /// syntect 高亮主题名，默认 "InspiredGitHub"（亮色，适合白底 PDF）
    pub syntax_theme: String,
}

#[derive(Debug, Clone)]
pub struct FontConfig {
    pub name: String,
    pub ttf_data: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy)]
pub struct FontSizes {
    pub body: f32,
    pub h1: f32,
    pub h2: f32,
    pub h3: f32,
    pub h4: f32,
    pub h5: f32,
    pub h6: f32,
    pub code: f32,
    pub header_footer: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    pub text: (u8, u8, u8),
    pub heading: (u8, u8, u8),
    pub code_bg: (u8, u8, u8),
    pub table_border: (u8, u8, u8),
    pub blockquote_border: (u8, u8, u8),
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeSpacing {
    pub line_height: f32,
    pub paragraph_gap: f32,
    pub list_indent: f32,
    pub cell_padding: f32,
}

impl PdfConfig {
    /// 暗色主题 preset：深色背景 + 浅色文字。
    pub fn dark() -> Self {
        let mut c = Self::default();
        c.theme.colors = ThemeColors {
            text: (220, 220, 220),
            heading: (255, 255, 255),
            code_bg: (60, 60, 60),
            table_border: (100, 100, 100),
            blockquote_border: (120, 120, 255),
        };
        c.theme.syntax_theme = "base16-ocean.dark".into();
        c
    }

    /// 极简 preset：系统字体，省墨。
    pub fn minimal() -> Self {
        let mut c = Self::default();
        c.theme.body_font = FontConfig {
            name: "sans-serif".into(),
            ttf_data: None,
        };
        c.theme.mono_font = FontConfig {
            name: "monospace".into(),
            ttf_data: None,
        };
        c.theme.heading_font = FontConfig {
            name: "sans-serif".into(),
            ttf_data: None,
        };
        c.header_footer = HeaderFooter {
            left: String::new(),
            center: String::new(),
            right: String::new(),
        };
        c
    }
}

impl Default for PdfConfig {
    /// 默认 preset：内嵌 Noto Sans CJK SC，A4，浅色主题。
    fn default() -> Self {
        Self {
            paper: Paper::A4,
            margins: Margins {
                top: 25.4,
                bottom: 25.4,
                left: 25.4,
                right: 25.4,
            },
            header_footer: HeaderFooter {
                left: String::new(),
                center: String::new(),
                right: String::from("{page}/{total}"),
            },
            theme: PdfTheme {
                body_font: FontConfig {
                    name: "Noto Sans CJK SC".into(),
                    ttf_data: None,
                },
                mono_font: FontConfig {
                    name: "Noto Sans Mono CJK SC".into(),
                    ttf_data: None,
                },
                heading_font: FontConfig {
                    name: "Noto Sans CJK SC".into(),
                    ttf_data: None,
                },
                font_size: FontSizes {
                    body: 11.0,
                    h1: 20.0,
                    h2: 18.0,
                    h3: 16.0,
                    h4: 14.0,
                    h5: 12.0,
                    h6: 11.0,
                    code: 9.0,
                    header_footer: 9.0,
                },
                colors: ThemeColors {
                    text: (0, 0, 0),
                    heading: (0, 0, 0),
                    code_bg: (240, 240, 240),
                    table_border: (180, 180, 180),
                    blockquote_border: (100, 100, 255),
                },
                spacing: ThemeSpacing {
                    line_height: 1.4,
                    paragraph_gap: 6.0,
                    list_indent: 20.0,
                    cell_padding: 4.0,
                },
                syntax_theme: "InspiredGitHub".into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_creates_a4_light_theme() {
        let c = PdfConfig::default();
        assert!(matches!(c.paper, Paper::A4));
        assert_eq!(c.theme.colors.text, (0, 0, 0));
        assert_eq!(c.theme.syntax_theme, "InspiredGitHub");
    }

    #[test]
    fn dark_theme_has_light_text() {
        let c = PdfConfig::dark();
        assert_eq!(c.theme.colors.text, (220, 220, 220));
        assert_eq!(c.theme.colors.code_bg, (60, 60, 60));
        assert_eq!(c.theme.syntax_theme, "base16-ocean.dark");
    }

    #[test]
    fn minimal_has_no_header_footer() {
        let c = PdfConfig::minimal();
        assert!(c.header_footer.left.is_empty());
        assert!(c.header_footer.center.is_empty());
        assert!(c.header_footer.right.is_empty());
    }

    #[test]
    fn minimal_inherits_default_syntax_theme() {
        let c = PdfConfig::minimal();
        assert_eq!(c.theme.syntax_theme, "InspiredGitHub");
    }
}
