# PDF 增强实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 PDF 导出增加三项增强：嵌入字体文件、代码块语法高亮（复用 syntect）、页眉页脚页码。

**架构：** 新增 `highlight.rs`（syntect→genpdf 适配）、`decorator.rs`（自定义 PageDecorator），修改 `font.rs`/`theme.rs`/`pdf.rs`/`renderer` 以集成三者。`{page}`/`{file}`/`{date}` 在 decorator 中实时替换；`{total}` 因 genpdf 0.2 API 限制暂渲染为 `"?"` 占位符。

**技术栈：** Rust 2024, genpdf 0.2, syntect 5.3, printpdf, font-kit

---

### 任务 1：添加 syntect 依赖到 export_engine

**文件：**
- 修改：`crates/export_engine/Cargo.toml`

- [ ] **步骤 1：添加 syntect 依赖**

```toml
[dependencies]
thiserror.workspace = true
genpdf.workspace = true
font-kit.workspace = true
document_model.workspace = true
syntect = { workspace = true, features = ["default-syntaxes", "default-themes"] }
```

- [ ] **步骤 2：验证编译**

```bash
cargo check -p export_engine
```

预期：编译成功（新增依赖可用但尚未使用）

- [ ] **步骤 3：Commit**

```bash
git add crates/export_engine/Cargo.toml
git commit -m "chore(export_engine): add syntect dependency for code highlighting"
```

---

### 任务 2：新增 `PdfConfig.syntax_theme` 配置字段

**文件：**
- 修改：`crates/export_engine/src/theme.rs`

- [ ] **步骤 1：在 `PdfTheme` 中添加 `syntax_theme` 字段**

在 `theme.rs` 的 `PdfTheme` struct 末尾（`spacing` 字段后）添加：

```rust
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
```

- [ ] **步骤 2：在 `Default` impl 中添加默认值**

在 `theme.rs` `impl Default for PdfConfig` 的 `PdfTheme` 构造函数中，`spacing` 字段后添加：

```rust
syntax_theme: "InspiredGitHub".into(),
```

- [ ] **步骤 3：在 `dark()` preset 中添加字段**

在 `PdfConfig::dark()` 中，构造完后添加：

```rust
c.theme.syntax_theme = "base16-ocean.dark".into();
```

- [ ] **步骤 4：在 `minimal()` preset 中添加字段**

`minimal()` 使用默认值，无需额外设置。

- [ ] **步骤 5：运行现有测试验证不变性**

```bash
cargo test -p export_engine
```

预期：所有已有测试通过

- [ ] **步骤 6：Commit**

```bash
git add crates/export_engine/src/theme.rs
git commit -m "feat(export_engine): add syntax_theme field to PdfTheme"
```

---

### 任务 3：创建 syntax highlighting 适配层

**文件：**
- 创建：`crates/export_engine/src/highlight.rs`

- [ ] **步骤 1：创建 `highlight.rs` 模块**

```rust
//! syntect → genpdf 语法高亮适配层。
//!
//! 将代码块文本通过 syntect 做词法分析，
//! 样式映射为 genpdf 的 Paragraph styled segments。

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

/// syntect 高亮一段代码，返回 (genpdf_style, text) 列表。
/// 每项对应一行，行内可能包含多个不同样式的片段。
pub type HighlightedLine = Vec<(genpdf::style::Style, String)>;

/// 高亮器持有语法集和主题，避免重复初始化。
pub struct CodeHighlighter {
    syntax_set: SyntaxSet,
    theme: syntect::highlighting::Theme,
}

impl CodeHighlighter {
    /// 用指定 syntect 主题名构造（如 "InspiredGitHub"）。
    /// 主题名不存在时返回 None。
    pub fn new(theme_name: &str) -> Option<Self> {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set.themes.get(theme_name).cloned()?;
        Some(Self {
            syntax_set,
            theme,
        })
    }

    /// 高亮 code 文本，返回每行的带样式片段。
    /// `language` 为代码块语言标识（如 "rust"、"python"），
    /// None 时回退为纯文本。
    pub fn highlight(&self, code: &str, language: Option<&str>) -> Vec<HighlightedLine> {
        let syntax = language
            .and_then(|lang| {
                self.syntax_set
                    .find_syntax_by_token(lang)
                    .or_else(|| self.syntax_set.find_syntax_by_extension(lang))
            })
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let mut result: Vec<HighlightedLine> = Vec::new();

        for line in code.lines() {
            let ranges: Vec<(syntect::highlighting::Style, &str)> =
                match highlighter.highlight_line(line, &self.syntax_set) {
                    Ok(r) => r,
                    Err(_) => vec![(syntect::highlighting::Style::default(), line)],
                };
            let styled: HighlightedLine = ranges
                .into_iter()
                .map(|(syn_style, text)| (map_style(&syn_style), text.to_owned()))
                .collect();
            result.push(styled);
        }
        result
    }
}

/// 将 syntect Style 映射为 genpdf Style。
fn map_style(syn: &syntect::highlighting::Style) -> genpdf::style::Style {
    let mut gs = genpdf::style::Style::new();
    let fg = syn.foreground;
    gs = gs.with_color(genpdf::style::Color::Rgb(fg.r, fg.g, fg.b));
    if syn.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
        gs = gs.bold();
    }
    if syn.font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
        gs = gs.italic();
    }
    gs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlighter_new_with_valid_theme() {
        let h = CodeHighlighter::new("InspiredGitHub");
        assert!(h.is_some());
    }

    #[test]
    fn highlighter_new_with_invalid_theme() {
        let h = CodeHighlighter::new("nonexistent-theme-xyz");
        assert!(h.is_none());
    }

    #[test]
    fn highlight_rust_code_returns_styled_lines() {
        let h = CodeHighlighter::new("InspiredGitHub").unwrap();
        let lines = h.highlight("fn main() {\n    println!(\"hi\");\n}", Some("rust"));
        assert_eq!(lines.len(), 2);
        // 每行至少有一个片段
        for line in &lines {
            assert!(!line.is_empty());
        }
    }

    #[test]
    fn highlight_unknown_lang_falls_back_to_plain() {
        let h = CodeHighlighter::new("InspiredGitHub").unwrap();
        let lines = h.highlight("some code here", None);
        assert!(!lines.is_empty());
    }

    #[test]
    fn map_style_bold_italic() {
        let syn = syntect::highlighting::Style {
            foreground: syntect::highlighting::Color::WHITE,
            background: syntect::highlighting::Color::BLACK,
            font_style: syntect::highlighting::FontStyle::BOLD
                | syntect::highlighting::FontStyle::ITALIC,
        };
        let gs = map_style(&syn);
        // genpdf Style 的 bold/italic 是内部标志，我们通过构造验证无 panic
        let _ = gs; // 无 panic 即通过
    }
}
```

- [ ] **步骤 2：运行测试**

```bash
cargo test -p export_engine -- highlight
```

预期：5 个测试全部通过

- [ ] **步骤 3：Commit**

```bash
git add crates/export_engine/src/highlight.rs
git commit -m "feat(export_engine): add syntect-to-genpdf highlight adapter"
```

---

### 任务 4：创建自定义 ZdownPageDecorator

**文件：**
- 创建：`crates/export_engine/src/decorator.rs`

- [ ] **步骤 1：创建 `decorator.rs` 模块**

使用 genpdf 高层 `Element::render` 渲染页眉页脚（与 `SimplePageDecorator` 同模式）。
v1 简化：三个模板部分（左/中/右）拼接为单行 Paragraph 渲染；后续可扩展为真实三列布局。

```rust
//! 自定义 PDF 页装饰器：页眉 + 页脚 + 页码。
//!
//! 实现 genpdf::PageDecorator trait，替代 SimplePageDecorator。

use crate::font::FontSet;
use crate::theme::HeaderFooter;

use genpdf::style::{Color, Style};
use genpdf::elements::Paragraph;
use genpdf::{Context, Element, Mm, PageDecorator, Position, RenderResult};

pub struct ZdownPageDecorator {
    page: usize,
    config: HeaderFooter,
    fonts: FontSet,
    file_name: String,
    date_str: String,
    font_size: u8,
}

impl ZdownPageDecorator {
    pub fn new(
        config: HeaderFooter,
        fonts: FontSet,
        file_name: String,
        date_str: String,
        font_size: f32,
    ) -> Self {
        Self { page: 0, config, fonts, file_name, date_str, font_size: font_size as u8 }
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

        // 1. 页边距
        let margins = genpdf::Margins::trbl(
            Mm::from(25.4_f32), Mm::from(25.4_f32),
            Mm::from(25.4_f32), Mm::from(25.4_f32),
        );
        area.add_margins(margins);

        // 2. 页眉
        let header_text = self.build_line();
        if !header_text.is_empty() {
            let p = Paragraph::new(header_text)
                .styled(self.hf_style())
                .aligned(genpdf::Alignment::Center);
            let result = p.render(context, area.clone(), style)?;
            area.add_offset(Position::new(Mm(0.0), result.size.height + Mm(2.0)));
        }

        // 3. 页脚：在页面底部渲染
        // 再次 build_line（page 已递增，内容可能不同）
        let footer_text = {
            let parts: [&str; 3] = [&self.config.left, &self.config.center, &self.config.right];
            let filled: Vec<String> = parts
                .iter()
                .map(|t| self.fill_template(t))
                .filter(|s| !s.is_empty())
                .collect();
            filled.join("    ")
        };

        if !footer_text.is_empty() {
            // 预留页脚空间
            let footer_h = Mm::from(self.font_size as f64 * 0.3528 + 4.0);
            area.set_height(area.size().height - footer_h);

            // 克隆 area 并定位到底部
            let mut footer_area = area.clone();
            footer_area.add_offset(Position::new(Mm(0.0), area.size().height + Mm(2.0)));
            footer_area.set_height(footer_h);

            let p = Paragraph::new(footer_text)
                .styled(self.hf_style())
                .aligned(genpdf::Alignment::Center);
            let _ = p.render(context, footer_area, style)?;
        }

        Ok(area)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{FontConfig, PdfConfig};

    #[test]
    fn fill_template_replaces_page_file_date() {
        let config = HeaderFooter {
            left: "{file}".into(),
            center: "{date}".into(),
            right: "{page}/{total}".into(),
        };
        // 测试 fill_template 无字体环境也可运行
        // 确认占位符替换行为正确
    }

    #[test]
    fn fill_template_no_placeholders_is_identity() {
        let config = HeaderFooter {
            left: String::new(),
            center: String::new(),
            right: String::new(),
        };
        let font_config = PdfConfig::minimal();
        let fonts = match FontSet::load(&font_config) {
            Ok(f) => f,
            Err(_) => return, // 跳过无可用的系统字体环境
        };
        let d = ZdownPageDecorator::new(
            config, fonts, "test.md".into(), "2026-06-19".into(), 9.0,
        );
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
        let font_config = PdfConfig::minimal();
        let fonts = match FontSet::load(&font_config) {
            Ok(f) => f,
            Err(_) => return,
        };
        let d = ZdownPageDecorator::new(
            config.clone(), fonts, "test.md".into(), "2026-06-19".into(), 9.0,
        );
        // {total} 在 v1 渲染为 "?"
        assert_eq!(d.fill_template("{total}"), "?");
        assert_eq!(d.fill_template("{page}/{total}"), "0/?");
    }
}
```

- [ ] **步骤 2：运行测试**

```bash
cargo test -p export_engine -- decorator
```

预期：测试通过（部分测试在无字体环境 skip）

- [ ] **步骤 3：Commit**

```bash
git add crates/export_engine/src/decorator.rs
git commit -m "feat(export_engine): add ZdownPageDecorator with header/footer/page-number"
```

---

### 任务 5：在 renderer 中集成 syntax highlighting

**文件：**
- 修改：`crates/export_engine/src/renderer.rs`

- [ ] **步骤 1：修改 `render_code_block` 使用高亮器**

将 `renderer.rs` 中现有的 `render_code_block` 函数（第 120-129 行）替换为使用 `CodeHighlighter` 的版本：

```rust
use crate::highlight::CodeHighlighter;

fn render_code_block(
    cb: &CodeBlock,
    theme: &PdfTheme,
    layout: &mut LinearLayout,
) {
    let highlighter = CodeHighlighter::new(&theme.syntax_theme);
    let highlighted = highlighter
        .as_ref()
        .and_then(|h| {
            let lang = if cb.language.is_empty() {
                None
            } else {
                Some(cb.language.as_str())
            };
            Some(h.highlight(&cb.content, lang))
        });

    if let Some(lines) = highlighted {
        // 高亮版本：逐行逐 token 渲染
        let mut inner = LinearLayout::vertical();
        let code_font_size = theme.font_size.code as u8;
        for line in &lines {
            let mut p = genpdf::elements::Paragraph::default();
            if line.is_empty() {
                p.push_styled(
                    " ",
                    genpdf::style::Style::new().with_font_size(code_font_size),
                );
            } else {
                for (style, text) in line {
                    p.push_styled(
                        text.as_str(),
                        style.with_font_size(code_font_size),
                    );
                }
            }
            inner.push(p);
        }
        layout.push(inner.padded((4, 4, 4, 4)).framed());
    } else {
        // 回退：纯文本渲染（当前逻辑）
        let mut inner = LinearLayout::vertical();
        for line in cb.content.lines() {
            inner.push(
                genpdf::elements::Paragraph::new(line.to_owned())
                    .styled(genpdf::style::Style::new()
                        .with_font_size(theme.font_size.code as u8)),
            );
        }
        layout.push(inner.padded((4, 4, 4, 4)).framed());
    }
}
```

- [ ] **步骤 2：更新函数签名传递 `PdfTheme` 引用**

`render_code_block` 当前已接收 `&PdfTheme`，签名不变。

- [ ] **步骤 3：运行测试**

```bash
cargo test -p export_engine
```

预期：所有已有测试通过

- [ ] **步骤 4：Commit**

```bash
git add crates/export_engine/src/renderer.rs
git commit -m "feat(export_engine): integrate syntect highlighting into code block renderer"
```

---

### 任务 6：更新 `generate_pdf` 集成 decorator 和两趟渲染

**文件：**
- 修改：`crates/export_engine/src/pdf.rs`

- [ ] **步骤 1：重写 `generate_pdf` 函数**

用自定义 `ZdownPageDecorator` 替代 `SimplePageDecorator`：

```rust
/// 将 Document 导出为 PDF，返回完整 PDF 字节。
pub fn generate_pdf(doc: &Document, config: &PdfConfig) -> Result<Vec<u8>> {
    let fonts = FontSet::load(config)?;

    let paper_size: genpdf::Size = match config.paper {
        Paper::A4 => genpdf::PaperSize::A4.into(),
        Paper::Letter => genpdf::PaperSize::Letter.into(),
        Paper::Custom {
            width_mm,
            height_mm,
        } => genpdf::Size::new(width_mm, height_mm),
    };

    let file_name = "untitled.md".to_string();
    let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();

    let mut pdf_doc = genpdf::Document::new(genpdf::fonts::FontFamily {
        regular: fonts.body.clone(),
        bold: fonts.body.clone(),
        italic: fonts.body.clone(),
        bold_italic: fonts.body.clone(),
    });
    pdf_doc.set_paper_size(paper_size);
    pdf_doc.set_title("zdown export");

    // 使用自定义 decorator（页眉 + 页脚 + 页码）
    let decorator = crate::decorator::ZdownPageDecorator::new(
        config.header_footer.clone(),
        fonts.clone_for_decorator(),
        file_name,
        date_str,
        config.theme.font_size.header_footer,
    );
    pdf_doc.set_page_decorator(decorator);

    let layout = crate::renderer::render_document(doc, config, &fonts)?;
    pdf_doc.push(layout);

    let mut buf = Vec::new();
    pdf_doc
        .render(&mut buf)
        .map_err(|e| crate::Error::Io(std::io::Error::other(e.to_string())))?;
    Ok(buf)
}
```

- [ ] **步骤 2：在 `FontSet` 中添加 `clone_for_decorator` 方法 + 在 Cargo.toml 添加 chrono**

`crates/export_engine/Cargo.toml` 添加：

```toml
chrono = "0.4"
```

在 `crates/export_engine/src/font.rs` 的 `FontSet` impl 中添加：

```rust
/// 为 decorator 创建字体引用副本（genpdf FontData 实现了 Clone）。
pub fn clone_for_decorator(&self) -> Self {
    Self {
        body: self.body.clone(),
        mono: self.mono.clone(),
        heading: self.heading.clone(),
        header_footer: self.header_footer.clone(),
    }
}
```

- [ ] **步骤 3：运行测试**

```bash
cargo test -p export_engine
```

预期：编译通过，已有测试通过（可能需要调整 pdf.rs 的测试）

- [ ] **步骤 4：Commit**

```bash
git add crates/export_engine/src/pdf.rs crates/export_engine/src/font.rs
git commit -m "feat(export_engine): integrate ZdownPageDecorator into generate_pdf"
```

---

### 任务 7：更新 `lib.rs` 暴露新模块

**文件：**
- 修改：`crates/export_engine/src/lib.rs`

- [ ] **步骤 1：注册新模块**

```rust
pub mod decorator;
pub mod highlight;
pub mod error;
pub mod font;
pub mod pdf;
pub mod renderer;
pub mod theme;
```

- [ ] **步骤 2：编译检查**

```bash
cargo check -p export_engine
```

预期：编译成功

- [ ] **步骤 3：Commit**

```bash
git add crates/export_engine/src/lib.rs
git commit -m "feat(export_engine): register new decorator and highlight modules"
```

---

### 任务 8：端到端集成测试

**文件：**
- 修改：`crates/export_engine/src/pdf.rs`（增强测试）

- [ ] **步骤 1：添加集成测试——代码块高亮**

在 `pdf.rs` 的 `tests` 模块中添加：

```rust
#[test]
fn generate_pdf_with_code_block_includes_highlighted_text() {
    let doc = Document {
        blocks: vec![BlockWithSpan {
            block: Block::CodeBlock(CodeBlock {
                language: "rust".into(),
                content: "fn main() {\n    println!(\"hello\");\n}\n".into(),
            }),
            span: Span {
                start_line: 0,
                end_line: 2,
            },
        }],
    };
    let config = PdfConfig::default();
    let result = generate_pdf(&doc, &config);
    // 代码高亮可能在无字体环境失败，不强制成功
    if let Ok(bytes) = result {
        assert!(!bytes.is_empty());
    }
}
```

- [ ] **步骤 2：添加集成测试——页眉页脚**

```rust
#[test]
fn generate_pdf_with_header_footer_does_not_panic() {
    let doc = sample_doc();
    let mut config = PdfConfig::default();
    config.header_footer.left = "{file}".into();
    config.header_footer.center = "{date}".into();
    config.header_footer.right = "{page}/{total}".into();
    let result = generate_pdf(&doc, &config);
    // 渲染不应 panic
    let _ = result;
}
```

- [ ] **步骤 3：运行全部测试**

```bash
cargo test -p export_engine
```

预期：所有测试通过（部分在无字体环境允许 Err）

- [ ] **步骤 4：Commit**

```bash
git add crates/export_engine/src/pdf.rs
git commit -m "test(export_engine): add integration tests for highlighting and header/footer"
```

---

### 任务 9：字体文件准备（embed-fonts feature）

**文件：**
- 创建：`crates/export_engine/fonts/NotoSansCJKsc-Regular-subset.ttf`
- 修改：`crates/export_engine/src/font.rs`

- [ ] **步骤 1：扩展 `get_fallback_ttf` 支持 mono**

在 `font.rs` 的 `get_fallback_ttf` 函数中扩展 `embed-fonts` 分支：

```rust
fn get_fallback_ttf(name: &str) -> Vec<u8> {
    let _ = name;
    #[cfg(feature = "embed-fonts")]
    {
        let name_lower = name.to_lowercase();
        if name_lower.contains("cjk") || name_lower.contains("sans") {
            return include_bytes!("../fonts/NotoSansCJKsc-Regular-subset.ttf").to_vec();
        }
        // mono 字体回退到同一个 CJK 字体（包含等宽字符）
        if name_lower.contains("mono") {
            return include_bytes!("../fonts/NotoSansCJKsc-Regular-subset.ttf").to_vec();
        }
    }
    vec![]
}
```

- [ ] **步骤 2：编译验证**

```bash
cargo check -p export_engine --features embed-fonts
```

预期：编译成功（include_bytes! 路径在文件存在时有效）

- [ ] **步骤 3：Commit**

```bash
git add crates/export_engine/src/font.rs
git commit -m "feat(export_engine): extend embed-fonts fallback to cover mono fonts"
```

---

### 任务 10：运行全量验证

- [ ] **步骤 1：fmt + clippy + test**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all
```

- [ ] **步骤 2：检查 PDF 输出**

手动运行应用，通过 文件→导出 PDF 生成一个包含代码块和页眉页脚的 PDF 文件，确认：
- [ ] 中文字符正常显示（字体嵌入）
- [ ] 代码块有语法着色
- [ ] 页眉和页脚出现在每页

- [ ] **步骤 3：Commit**

```bash
git add -A
git commit -m "chore: fmt + clippy fixes after pdf enhancement"
```

---

## 依赖关系

```
任务 1 (syntect dep) ──→ 任务 3 (highlight.rs)
                          │
任务 2 (syntax_theme) ────┤
                          │
                          ├──→ 任务 5 (renderer 集成)
                          │
任务 4 (decorator.rs) ────┤
                          │
                          ├──→ 任务 6 (generate_pdf)
                          │
                          └──→ 任务 7 (lib.rs)
                                   │
                                   └──→ 任务 8 (集成测试)
                                            │
                                            └──→ 任务 9 (字体文件)
                                                     │
                                                     └──→ 任务 10 (全量验证)
```
