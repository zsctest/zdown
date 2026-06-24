//! 内联 style 属性解析。
//!
//! 支持 9 个 CSS 属性到 egui 的映射。

use egui::Color32;

/// 解析后的 CSS 样式属性集合。
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CssStyle {
    pub color: Option<Color32>,
    pub background_color: Option<Color32>,
    pub font_size: Option<f32>,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub text_decoration: Option<TextDecoration>,
    pub text_align: Option<TextAlign>,
    pub margin: Option<Spacing>,
    pub padding: Option<Spacing>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FontWeight {
    Bold,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FontStyle {
    Italic,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TextDecoration {
    Underline,
    LineThrough,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TextAlign {
    Left,
    Center,
    Right,
}

/// 四边间距。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Spacing {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Spacing {
    pub fn uniform(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }
}

/// 解析颜色字符串：支持十六进制 `#rgb` / `#rrggbb`、`rgb(r,g,b)` 和 16 个命名颜色。
fn parse_color(value: &str) -> Option<Color32> {
    let value = value.trim();
    if value.starts_with('#') {
        parse_hex_color(value)
    } else if value.starts_with("rgb(") {
        parse_rgb_color(value)
    } else {
        parse_named_color(value)
    }
}

fn parse_hex_color(value: &str) -> Option<Color32> {
    let hex = value.trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some(Color32::from_rgb(r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color32::from_rgb(r, g, b))
        }
        _ => None,
    }
}

fn parse_rgb_color(value: &str) -> Option<Color32> {
    let inner = value.trim_start_matches("rgb(").trim_end_matches(')');
    let mut parts = inner.split(',');
    let r: u8 = parts.next()?.trim().parse().ok()?;
    let g: u8 = parts.next()?.trim().parse().ok()?;
    let b: u8 = parts.next()?.trim().parse().ok()?;
    Some(Color32::from_rgb(r, g, b))
}

fn parse_named_color(value: &str) -> Option<Color32> {
    match value.to_lowercase().as_str() {
        "red" => Some(Color32::RED),
        "blue" => Some(Color32::BLUE),
        "green" => Some(Color32::GREEN),
        "yellow" => Some(Color32::YELLOW),
        "white" => Some(Color32::WHITE),
        "black" => Some(Color32::BLACK),
        "gray" | "grey" => Some(Color32::GRAY),
        "orange" => Some(Color32::from_rgb(255, 165, 0)),
        "purple" => Some(Color32::from_rgb(128, 0, 128)),
        "pink" => Some(Color32::from_rgb(255, 192, 203)),
        "cyan" => Some(Color32::from_rgb(0, 255, 255)),
        "magenta" => Some(Color32::from_rgb(255, 0, 255)),
        "lime" => Some(Color32::from_rgb(0, 255, 0)),
        "navy" => Some(Color32::from_rgb(0, 0, 128)),
        "teal" => Some(Color32::from_rgb(0, 128, 128)),
        "maroon" => Some(Color32::from_rgb(128, 0, 0)),
        _ => None,
    }
}

/// 解析 `style="..."` 属性字符串为 `CssStyle`。
pub(crate) fn parse_style(style_str: &str) -> CssStyle {
    let mut style = CssStyle::default();
    for decl in style_str.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let mut parts = decl.splitn(2, ':');
        let key = parts.next().map(|s| s.trim().to_lowercase());
        let val = parts.next().map(|s| s.trim());
        match (key.as_deref(), val) {
            (Some("color"), Some(v)) => style.color = parse_color(v),
            (Some("background-color"), Some(v)) => style.background_color = parse_color(v),
            (Some("font-size"), Some(v)) => style.font_size = parse_px(v),
            (Some("font-weight"), Some(v)) => style.font_weight = parse_font_weight(v),
            (Some("font-style"), Some(v)) => style.font_style = parse_font_style(v),
            (Some("text-decoration"), Some(v)) => style.text_decoration = parse_text_decoration(v),
            (Some("text-align"), Some(v)) => style.text_align = parse_text_align(v),
            (Some("margin"), Some(v)) => style.margin = parse_spacing(v),
            (Some("padding"), Some(v)) => style.padding = parse_spacing(v),
            _ => {}
        }
    }
    style
}

fn parse_px(value: &str) -> Option<f32> {
    let v = value.trim_end_matches("px").trim();
    v.parse::<f32>().ok()
}

fn parse_font_weight(value: &str) -> Option<FontWeight> {
    match value.trim().to_lowercase().as_str() {
        "bold" | "700" | "800" | "900" | "bolder" => Some(FontWeight::Bold),
        "normal" | "400" | "lighter" => Some(FontWeight::Normal),
        _ => None,
    }
}

fn parse_font_style(value: &str) -> Option<FontStyle> {
    match value.trim().to_lowercase().as_str() {
        "italic" | "oblique" => Some(FontStyle::Italic),
        "normal" => Some(FontStyle::Normal),
        _ => None,
    }
}

fn parse_text_decoration(value: &str) -> Option<TextDecoration> {
    match value.trim().to_lowercase().as_str() {
        "underline" => Some(TextDecoration::Underline),
        "line-through" => Some(TextDecoration::LineThrough),
        _ => None,
    }
}

fn parse_text_align(value: &str) -> Option<TextAlign> {
    match value.trim().to_lowercase().as_str() {
        "left" | "start" => Some(TextAlign::Left),
        "center" => Some(TextAlign::Center),
        "right" | "end" => Some(TextAlign::Right),
        _ => None,
    }
}

fn parse_spacing(value: &str) -> Option<Spacing> {
    let parts: Vec<f32> = value
        .split_whitespace()
        .filter_map(|s| s.trim_end_matches("px").parse::<f32>().ok())
        .collect();
    match parts.len() {
        1 => Some(Spacing::uniform(parts[0])),
        2 => Some(Spacing {
            top: parts[0],
            right: parts[1],
            bottom: parts[0],
            left: parts[1],
        }),
        3 => Some(Spacing {
            top: parts[0],
            right: parts[1],
            bottom: parts[2],
            left: parts[1],
        }),
        4 => Some(Spacing {
            top: parts[0],
            right: parts[1],
            bottom: parts[2],
            left: parts[3],
        }),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ---- color parsing ----

    #[test]
    fn parse_color_hex_3() {
        assert_eq!(parse_color("#f00"), Some(Color32::RED));
        assert_eq!(parse_color("#0f0"), Some(Color32::GREEN));
        assert_eq!(parse_color("#00f"), Some(Color32::BLUE));
    }

    #[test]
    fn parse_color_hex_6() {
        assert_eq!(parse_color("#ff0000"), Some(Color32::RED));
        assert_eq!(parse_color("#00ff00"), Some(Color32::GREEN));
    }

    #[test]
    fn parse_color_rgb() {
        assert_eq!(parse_color("rgb(255, 0, 0)"), Some(Color32::RED));
        assert_eq!(parse_color("rgb(0,0,255)"), Some(Color32::BLUE));
    }

    #[test]
    fn parse_color_named() {
        assert_eq!(parse_color("red"), Some(Color32::RED));
        assert_eq!(parse_color("blue"), Some(Color32::BLUE));
        assert_eq!(parse_color("orange"), Some(Color32::from_rgb(255, 165, 0)));
    }

    #[test]
    fn parse_color_invalid() {
        assert_eq!(parse_color("notacolor"), None);
        assert_eq!(parse_color(""), None);
    }

    // ---- CSS style parsing ----

    #[test]
    fn parse_style_color() {
        let s = parse_style("color: red");
        assert_eq!(s.color, Some(Color32::RED));
    }

    #[test]
    fn parse_style_bg_color() {
        let s = parse_style("background-color: #ff0");
        assert_eq!(s.background_color, Some(Color32::YELLOW));
    }

    #[test]
    fn parse_style_font_size() {
        let s = parse_style("font-size: 16px");
        assert_eq!(s.font_size, Some(16.0));
    }

    #[test]
    fn parse_style_font_weight() {
        let s = parse_style("font-weight: bold");
        assert_eq!(s.font_weight, Some(FontWeight::Bold));
    }

    #[test]
    fn parse_style_font_style() {
        let s = parse_style("font-style: italic");
        assert_eq!(s.font_style, Some(FontStyle::Italic));
    }

    #[test]
    fn parse_style_text_decoration() {
        let s = parse_style("text-decoration: underline");
        assert_eq!(s.text_decoration, Some(TextDecoration::Underline));

        let s = parse_style("text-decoration: line-through");
        assert_eq!(s.text_decoration, Some(TextDecoration::LineThrough));
    }

    #[test]
    fn parse_style_text_align() {
        let s = parse_style("text-align: center");
        assert_eq!(s.text_align, Some(TextAlign::Center));
    }

    #[test]
    fn parse_style_margin_uniform() {
        let s = parse_style("margin: 8px");
        assert_eq!(s.margin, Some(Spacing::uniform(8.0)));
    }

    #[test]
    fn parse_style_margin_two_values() {
        let s = parse_style("margin: 4px 8px");
        assert_eq!(
            s.margin,
            Some(Spacing {
                top: 4.0,
                right: 8.0,
                bottom: 4.0,
                left: 8.0,
            })
        );
    }

    #[test]
    fn parse_style_padding() {
        let s = parse_style("padding: 4px");
        assert_eq!(s.padding, Some(Spacing::uniform(4.0)));
    }

    #[test]
    fn parse_style_multiple_properties() {
        let s = parse_style("color: red; font-weight: bold; padding: 8px");
        assert_eq!(s.color, Some(Color32::RED));
        assert_eq!(s.font_weight, Some(FontWeight::Bold));
        assert_eq!(s.padding, Some(Spacing::uniform(8.0)));
    }

    #[test]
    fn parse_style_empty() {
        let s = parse_style("");
        assert_eq!(s, CssStyle::default());
    }

    #[test]
    fn parse_style_unknown_property_ignored() {
        let s = parse_style("display: flex; color: red");
        assert_eq!(s.color, Some(Color32::RED));
    }
}
