//! 自定义 PDF 页装饰器：页眉 + 页脚 + 页码。
//!
//! 实现 genpdf::PageDecorator trait，替代 SimplePageDecorator。

use crate::theme::HeaderFooter;

use genpdf::elements::Paragraph;
use genpdf::style::{Color, Style};
use genpdf::{Alignment, Context, Element, Mm, PageDecorator, Position};

pub struct ZdownPageDecorator {
    page: usize,
    config: HeaderFooter,
    margins: genpdf::Margins,
    file_name: String,
    date_str: String,
    font_size: u8,
}

impl ZdownPageDecorator {
    pub fn new(
        config: HeaderFooter,
        margins: genpdf::Margins,
        file_name: String,
        date_str: String,
        font_size: f32,
    ) -> Self {
        Self {
            page: 0,
            config,
            margins,
            file_name,
            date_str,
            font_size: font_size as u8,
        }
    }

    fn fill_template(&self, template: &str) -> String {
        if template.is_empty() {
            return String::new();
        }
        template
            .replace("{page}", &self.page.to_string())
            .replace("{total}", "?")
            .replace("{file}", &self.file_name)
            .replace("{date}", &self.date_str)
    }

    /// 拼接左/中/右为一个字符串，用空格分隔。
    fn build_line(&self) -> String {
        let parts: [&str; 3] = [&self.config.left, &self.config.center, &self.config.right];
        let filled: Vec<String> = parts
            .iter()
            .map(|t| self.fill_template(t))
            .filter(|s| !s.is_empty())
            .collect();
        filled.join("    ")
    }

    fn hf_style(&self) -> Style {
        Style::new()
            .with_font_size(self.font_size)
            .with_color(Color::Rgb(128, 128, 128))
    }
}

impl PageDecorator for ZdownPageDecorator {
    fn decorate_page<'a>(
        &mut self,
        context: &Context,
        mut area: genpdf::render::Area<'a>,
        style: Style,
    ) -> Result<genpdf::render::Area<'a>, genpdf::error::Error> {
        self.page += 1;

        // 1. 页边距（来自配置）
        area.add_margins(self.margins);

        // 2. 页眉
        let header_text = self.build_line();
        if !header_text.is_empty() {
            let mut p = Paragraph::new(header_text)
                .aligned(Alignment::Center)
                .styled(self.hf_style());
            let result = p.render(context, area.clone(), style)?;
            area.add_offset(Position::new(
                Mm::from(0.0_f32),
                result.size.height + Mm::from(2.0_f32),
            ));
        }

        // 3. 页脚
        let footer_text = self.build_line();
        if !footer_text.is_empty() {
            let footer_h = Mm::from(self.font_size as f64 * 0.3528 + 4.0);
            area.set_height(area.size().height - footer_h);

            let mut footer_area = area.clone();
            footer_area.add_offset(Position::new(
                Mm::from(0.0_f32),
                area.size().height + Mm::from(2.0_f32),
            ));
            footer_area.set_height(footer_h);

            let mut p = Paragraph::new(footer_text)
                .aligned(Alignment::Center)
                .styled(self.hf_style());
            p.render(context, footer_area, style)?;
        }

        Ok(area)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_margins() -> genpdf::Margins {
        genpdf::Margins::trbl(
            Mm::from(25.4_f32),
            Mm::from(25.4_f32),
            Mm::from(25.4_f32),
            Mm::from(25.4_f32),
        )
    }

    fn make_decorator(config: HeaderFooter, file_name: &str, date_str: &str) -> ZdownPageDecorator {
        ZdownPageDecorator::new(
            config,
            test_margins(),
            file_name.into(),
            date_str.into(),
            9.0,
        )
    }

    #[test]
    fn fill_template_no_placeholders_is_identity() {
        let config = HeaderFooter {
            left: String::new(),
            center: String::new(),
            right: String::new(),
        };
        let d = make_decorator(config, "test.md", "2026-06-19");
        assert_eq!(d.fill_template("hello"), "hello");
        assert_eq!(d.fill_template(""), "");
    }

    #[test]
    fn total_placeholder_renders_question_mark() {
        let config = HeaderFooter {
            left: String::new(),
            center: String::new(),
            right: String::new(),
        };
        let d = make_decorator(config, "test.md", "2026-06-19");
        assert_eq!(d.fill_template("{total}"), "?");
        assert_eq!(d.fill_template("{page}/{total}"), "0/?");
    }

    #[test]
    fn fill_template_replaces_page_file_date() {
        let config = HeaderFooter {
            left: "{file}".into(),
            center: "{date}".into(),
            right: "{page}/{total}".into(),
        };
        let d = make_decorator(config, "mydoc.md", "2026-06-19");
        assert_eq!(d.fill_template("{file}"), "mydoc.md");
        assert_eq!(d.fill_template("{date}"), "2026-06-19");
        assert_eq!(d.fill_template("{page}"), "0");
        assert_eq!(d.fill_template("{total}"), "?");
    }
}
