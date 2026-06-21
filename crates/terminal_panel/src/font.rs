//! 终端等宽字体度量。

use egui::{Context, FontId};

/// 终端字体（等宽）。
#[derive(Debug, Clone)]
pub struct TerminalFont {
    font_type: FontId,
}

impl TerminalFont {
    /// 以指定字号创建。
    pub fn new(size: f32) -> Self {
        Self {
            font_type: FontId::monospace(size),
        }
    }

    /// 获取 egui FontId。
    pub fn font_type(&self) -> FontId {
        self.font_type.clone()
    }

    /// 使用 'm' 字符测量单元格宽高。
    pub fn cell_size(&self, ctx: &Context) -> (f32, f32) {
        ctx.fonts_mut(|f| {
            let width = f.glyph_width(&self.font_type, 'm');
            let height = f.row_height(&self.font_type);
            (width, height)
        })
    }
}

impl Default for TerminalFont {
    fn default() -> Self {
        Self::new(14.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_font_default_is_monospace_14() {
        let font = TerminalFont::default();
        let fid = font.font_type();
        assert_eq!(fid.size, 14.0);
    }

    #[test]
    fn terminal_font_new_respects_size() {
        let font = TerminalFont::new(16.0);
        let fid = font.font_type();
        assert_eq!(fid.size, 16.0);
    }
}
