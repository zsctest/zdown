//! ANSI 色彩主题。

use alacritty_terminal::vte::ansi::{self, NamedColor};
use egui::Color32;
use std::collections::HashMap;

/// ANSI 16 色调色板。
#[derive(Debug, Clone)]
pub struct ColorPalette {
    pub foreground: String,
    pub background: String,
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    pub bright_magenta: String,
    pub bright_cyan: String,
    pub bright_white: String,
    pub bright_foreground: Option<String>,
    pub dim_foreground: String,
    pub dim_black: String,
    pub dim_red: String,
    pub dim_green: String,
    pub dim_yellow: String,
    pub dim_blue: String,
    pub dim_magenta: String,
    pub dim_cyan: String,
    pub dim_white: String,
}

impl Default for ColorPalette {
    fn default() -> Self {
        // One Dark 风格（默认暗色主题）
        Self {
            foreground: String::from("#d8d8d8"),
            background: String::from("#181818"),
            black: String::from("#181818"),
            red: String::from("#ac4242"),
            green: String::from("#90a959"),
            yellow: String::from("#f4bf75"),
            blue: String::from("#6a9fb5"),
            magenta: String::from("#aa759f"),
            cyan: String::from("#75b5aa"),
            white: String::from("#d8d8d8"),
            bright_black: String::from("#6b6b6b"),
            bright_red: String::from("#c55555"),
            bright_green: String::from("#aac474"),
            bright_yellow: String::from("#feca88"),
            bright_blue: String::from("#82b8c8"),
            bright_magenta: String::from("#c28cb8"),
            bright_cyan: String::from("#93d3c3"),
            bright_white: String::from("#f8f8f8"),
            bright_foreground: None,
            dim_foreground: String::from("#828482"),
            dim_black: String::from("#0f0f0f"),
            dim_red: String::from("#712b2b"),
            dim_green: String::from("#5f6f3a"),
            dim_yellow: String::from("#a17e4d"),
            dim_blue: String::from("#456877"),
            dim_magenta: String::from("#704d68"),
            dim_cyan: String::from("#4d7770"),
            dim_white: String::from("#8e8e8e"),
        }
    }
}

/// 终端颜色主题。
#[derive(Debug, Clone)]
pub struct TerminalTheme {
    palette: ColorPalette,
    ansi256_colors: HashMap<u8, Color32>,
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self::new(ColorPalette::default())
    }
}

impl TerminalTheme {
    pub fn new(palette: ColorPalette) -> Self {
        Self {
            palette,
            ansi256_colors: Self::build_ansi256(),
        }
    }

    /// 预置 Monokai 主题。
    pub fn monokai() -> Self {
        Self::new(ColorPalette {
            foreground: String::from("#f8f8f2"),
            background: String::from("#272822"),
            black: String::from("#272822"),
            red: String::from("#f92672"),
            green: String::from("#a6e22e"),
            yellow: String::from("#f4bf75"),
            blue: String::from("#66d9ef"),
            magenta: String::from("#ae81ff"),
            cyan: String::from("#a1efe4"),
            white: String::from("#f8f8f2"),
            bright_black: String::from("#75715e"),
            bright_red: String::from("#f92672"),
            bright_green: String::from("#a6e22e"),
            bright_yellow: String::from("#f4bf75"),
            bright_blue: String::from("#66d9ef"),
            bright_magenta: String::from("#ae81ff"),
            bright_cyan: String::from("#a1efe4"),
            bright_white: String::from("#f9f8f5"),
            bright_foreground: None,
            dim_foreground: String::from("#75715e"),
            dim_black: String::from("#1b1b18"),
            dim_red: String::from("#a5204d"),
            dim_green: String::from("#6e971f"),
            dim_yellow: String::from("#a17e4d"),
            dim_blue: String::from("#448fa3"),
            dim_magenta: String::from("#7356aa"),
            dim_cyan: String::from("#6b9f98"),
            dim_white: String::from("#a5a5a1"),
        })
    }

    fn build_ansi256() -> HashMap<u8, Color32> {
        let mut colors = HashMap::new();
        // 6x6x6 色彩立方 (index 16-231)
        for r in 0..6u8 {
            for g in 0..6u8 {
                for b in 0..6u8 {
                    let index = 16 + r * 36 + g * 6 + b;
                    let rv = if r == 0 { 0 } else { r * 40 + 55 };
                    let gv = if g == 0 { 0 } else { g * 40 + 55 };
                    let bv = if b == 0 { 0 } else { b * 40 + 55 };
                    colors.insert(index, Color32::from_rgb(rv, gv, bv));
                }
            }
        }
        // 灰度 (index 232-255)
        for i in 0..24u8 {
            let v = i * 10 + 8;
            colors.insert(232 + i, Color32::from_rgb(v, v, v));
        }
        colors
    }

    /// 将 alacritty Color 转换为 egui Color32。
    pub fn get_color(&self, c: ansi::Color) -> Color32 {
        match c {
            ansi::Color::Spec(rgb) => Color32::from_rgb(rgb.r, rgb.g, rgb.b),
            ansi::Color::Indexed(index) => {
                if index <= 15 {
                    self.indexed_16_color(index)
                } else {
                    self.ansi256_colors
                        .get(&index)
                        .copied()
                        .unwrap_or(Color32::BLACK)
                }
            }
            ansi::Color::Named(named) => self.named_color(named),
        }
    }

    fn indexed_16_color(&self, index: u8) -> Color32 {
        let hex = match index {
            0 => &self.palette.black,
            1 => &self.palette.red,
            2 => &self.palette.green,
            3 => &self.palette.yellow,
            4 => &self.palette.blue,
            5 => &self.palette.magenta,
            6 => &self.palette.cyan,
            7 => &self.palette.white,
            8 => &self.palette.bright_black,
            9 => &self.palette.bright_red,
            10 => &self.palette.bright_green,
            11 => &self.palette.bright_yellow,
            12 => &self.palette.bright_blue,
            13 => &self.palette.bright_magenta,
            14 => &self.palette.bright_cyan,
            15 => &self.palette.bright_white,
            _ => &self.palette.background,
        };
        hex_to_color(hex).unwrap_or(Color32::BLACK)
    }

    fn named_color(&self, named: NamedColor) -> Color32 {
        let hex = match named {
            NamedColor::Foreground => &self.palette.foreground,
            NamedColor::Background => &self.palette.background,
            NamedColor::Black => &self.palette.black,
            NamedColor::Red => &self.palette.red,
            NamedColor::Green => &self.palette.green,
            NamedColor::Yellow => &self.palette.yellow,
            NamedColor::Blue => &self.palette.blue,
            NamedColor::Magenta => &self.palette.magenta,
            NamedColor::Cyan => &self.palette.cyan,
            NamedColor::White => &self.palette.white,
            NamedColor::BrightBlack => &self.palette.bright_black,
            NamedColor::BrightRed => &self.palette.bright_red,
            NamedColor::BrightGreen => &self.palette.bright_green,
            NamedColor::BrightYellow => &self.palette.bright_yellow,
            NamedColor::BrightBlue => &self.palette.bright_blue,
            NamedColor::BrightMagenta => &self.palette.bright_magenta,
            NamedColor::BrightCyan => &self.palette.bright_cyan,
            NamedColor::BrightWhite => &self.palette.bright_white,
            NamedColor::BrightForeground => self
                .palette
                .bright_foreground
                .as_deref()
                .unwrap_or(&self.palette.foreground),
            NamedColor::DimForeground => &self.palette.dim_foreground,
            NamedColor::DimBlack => &self.palette.dim_black,
            NamedColor::DimRed => &self.palette.dim_red,
            NamedColor::DimGreen => &self.palette.dim_green,
            NamedColor::DimYellow => &self.palette.dim_yellow,
            NamedColor::DimBlue => &self.palette.dim_blue,
            NamedColor::DimMagenta => &self.palette.dim_magenta,
            NamedColor::DimCyan => &self.palette.dim_cyan,
            NamedColor::DimWhite => &self.palette.dim_white,
            _ => &self.palette.background,
        };
        hex_to_color(hex).unwrap_or(Color32::BLACK)
    }
}

/// 将 "#rrggbb" 字符串转换为 Color32。
pub fn hex_to_color(hex: &str) -> Result<Color32, String> {
    if hex.len() != 7 || !hex.starts_with('#') {
        return Err(format!("无效的颜色格式: {hex}"));
    }
    let r = u8::from_str_radix(&hex[1..3], 16).map_err(|e| format!("{e}"))?;
    let g = u8::from_str_radix(&hex[3..5], 16).map_err(|e| format!("{e}"))?;
    let b = u8::from_str_radix(&hex[5..7], 16).map_err(|e| format!("{e}"))?;
    Ok(Color32::from_rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::vte::ansi::{Color, NamedColor};
    use alacritty_terminal::vte::ansi;

    #[test]
    fn default_palette_has_16_colors() {
        let palette = ColorPalette::default();
        let bg = hex_to_color(&palette.background).unwrap();
        assert_eq!(bg, egui::Color32::from_rgb(0x18, 0x18, 0x18));
    }

    #[test]
    fn theme_get_color_named() {
        let theme = TerminalTheme::default();
        let fg = theme.get_color(Color::Named(NamedColor::Foreground));
        assert_eq!(fg, egui::Color32::from_rgb(0xd8, 0xd8, 0xd8));
    }

    #[test]
    fn theme_get_color_indexed() {
        let theme = TerminalTheme::default();
        let red = theme.get_color(Color::Indexed(1));
        assert_eq!(red, egui::Color32::from_rgb(0xac, 0x42, 0x42));
    }

    #[test]
    fn theme_get_color_rgb() {
        let theme = TerminalTheme::default();
        let color = theme.get_color(Color::Spec(ansi::Rgb {
            r: 255,
            g: 128,
            b: 64,
        }));
        assert_eq!(color, egui::Color32::from_rgb(255, 128, 64));
    }

    #[test]
    fn hex_to_color_valid() {
        assert_eq!(
            hex_to_color("#ff0080").unwrap(),
            egui::Color32::from_rgb(0xff, 0x00, 0x80)
        );
    }

    #[test]
    fn hex_to_color_invalid() {
        assert!(hex_to_color("#123").is_err());
        assert!(hex_to_color("invalid").is_err());
    }

    #[test]
    fn ansi256_index_16_is_color() {
        let theme = TerminalTheme::default();
        let color = theme.get_color(Color::Indexed(16));
        // Index 16 = r=0,g=0,b=0 → r=0 (special-cased), so rgb(0,0,0)
        assert_eq!(color, egui::Color32::from_rgb(0, 0, 0));
    }

    #[test]
    fn theme_monokai_predefined() {
        let monokai = TerminalTheme::monokai();
        let bg = monokai.get_color(Color::Named(NamedColor::Background));
        assert_eq!(bg, egui::Color32::from_rgb(0x27, 0x28, 0x22));
    }
}
