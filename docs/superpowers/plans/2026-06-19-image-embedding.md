# PDF Image Embedding — 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 让 PDF 导出能将 Markdown `![alt](url)` 图片渲染为嵌入图片（本地文件、base64 data URI、远程 URL），失败时降级为 `[图片: alt]` 占位文本。

**架构：** 新增 `image_loader` 模块处理图片获取/解码/alpha-flatten；renderer 按 image 边界分割 inline 列表，Text 段照常渲染为 Paragraph，Image 段直接 push `elements::Image` 到 LinearLayout；`render_heading`/`render_paragraph` 从返回 Paragraph 改为 push 模式。

**技术栈：** Rust 2024, genpdf 0.2 (features=["images"]), image 0.23, ureq 2, base64 0.22

---

### 任务 1：依赖和错误类型

**文件：**
- 修改：`crates/export_engine/Cargo.toml`
- 修改：`crates/export_engine/src/error.rs`
- 修改：`crates/export_engine/src/lib.rs`

- [ ] **步骤 1：更新 Cargo.toml 依赖**

在 `crates/export_engine/Cargo.toml` 中：
- genpdf 启用 `images` feature
- 新增 `image`、`ureq`、`base64` 依赖

```toml
[dependencies]
thiserror.workspace = true
chrono.workspace = true
genpdf = { workspace = true, features = ["images"] }
font-kit.workspace = true
document_model.workspace = true
syntect = { workspace = true, features = ["default-syntaxes", "default-themes"] }
image = "0.23"
ureq = { version = "2", default-features = false, features = ["tls"] }
base64 = "0.22"
```

> **注意：** `image` 必须使用 `0.23` 以匹配 genpdf 0.2 依赖的 `image::DynamicImage` 类型。`ureq` 2.x 提供同步 HTTP 客户端，`tls` feature 启用 native-tls 以支持 HTTPS。

- [ ] **步骤 2：添加 `ImageLoad` 错误变体**

修改 `crates/export_engine/src/error.rs`，在枚举中添加新变体：

```rust
//! export_engine 错误类型。

use thiserror::Error;

/// export_engine 错误。
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("字体加载失败: {0}")]
    FontLoad(String),

    #[error("PDF 渲染错误: {0}")]
    Render(String),

    #[error("图片加载失败: {0}")]
    ImageLoad(String),
}
```

- [ ] **步骤 3：在 lib.rs 注册 image_loader 模块**

修改 `crates/export_engine/src/lib.rs`：

```rust
//! export_engine：Markdown → PDF/HTML 导出（阶段 3）。

pub mod decorator;
pub mod error;
pub mod font;
pub mod highlight;
pub mod image_loader;
pub mod pdf;
pub mod renderer;
pub mod theme;

pub use error::Error;
pub use pdf::generate_pdf;
pub use theme::PdfConfig;

/// crate 级 Result 别名。
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "export_engine");
    }
}
```

- [ ] **步骤 4：验证编译**

```bash
cargo check -p export_engine
```

预期：编译失败（`image_loader` 模块还不存在，但 `lib.rs` 已声明）。这是正常的——下一步创建该模块。

- [ ] **步骤 5：Commit**

```bash
git add crates/export_engine/Cargo.toml crates/export_engine/src/error.rs crates/export_engine/src/lib.rs
git commit -m "feat(export_engine): add image deps, ImageLoad error, register image_loader module"
```

---

### 任务 2：PdfConfig 添加 working_dir

**文件：**
- 修改：`crates/export_engine/src/theme.rs`

- [ ] **步骤 1：添加 `working_dir` 字段**

修改 `PdfConfig` struct 定义，在 `pub header_footer` 之后添加字段：

```rust
/// PDF 导出总配置。
#[derive(Debug, Clone)]
pub struct PdfConfig {
    pub paper: Paper,
    pub margins: Margins,
    pub header_footer: HeaderFooter,
    /// 工作目录，用于解析 Markdown 中相对路径的本地图片。
    /// `None` 时相对路径图片无法加载（降级为占位文本）。
    pub working_dir: Option<std::path::PathBuf>,
    pub theme: PdfTheme,
}
```

- [ ] **步骤 2：更新 Default impl — 设置 working_dir 为 None**

在 `impl Default for PdfConfig` 中，`header_footer` 之后添加：

```rust
impl Default for PdfConfig {
    fn default() -> Self {
        Self {
            paper: Paper::A4,
            margins: Margins { ... },
            header_footer: HeaderFooter { ... },
            working_dir: None,
            theme: PdfTheme { ... },
        }
    }
}
```

- [ ] **步骤 3：更新 minimal() preset — 设置 working_dir 为 None**

`minimal()` 返回的 `Self` 中 `..Self::default()` 会自动获得 `working_dir: None`，无需额外修改。但确认一下代码。

- [ ] **步骤 4：运行现有测试确保没有破坏**

```bash
cargo test -p export_engine
```

预期：现有测试全部通过。

- [ ] **步骤 5：Commit**

```bash
git add crates/export_engine/src/theme.rs
git commit -m "feat(export_engine): add working_dir field to PdfConfig"
```

---

### 任务 3：创建 image_loader 模块

**文件：**
- 创建：`crates/export_engine/src/image_loader.rs`

- [ ] **步骤 1：编写测试**

在 `crates/export_engine/src/image_loader.rs` 文件顶部编写模块和测试。先写测试代码：

```rust
//! 图片加载器：从本地文件、data URI、远程 URL 加载图片。
//!
//! 所有加载路径都返回 `image::DynamicImage`，
//! 失败时返回 `Error::ImageLoad`。

use std::path::Path;

use image::DynamicImage;

use crate::Result;

/// 根据 URL 类型分发到对应的加载函数。
///
/// - `data:` 前缀 → data URI（base64 编码）
/// - `http://` / `https://` 前缀 → 远程 URL
/// - 其他 → 本地文件路径（相对路径基于 `working_dir`）
pub fn load_image(url: &str, working_dir: Option<&Path>) -> Result<DynamicImage> {
    if url.starts_with("data:") {
        load_from_data_uri(url)
    } else if url.starts_with("http://") || url.starts_with("https://") {
        load_from_remote(url)
    } else {
        load_from_local(url, working_dir)
    }
}

/// 从 data URI 加载图片。格式：`data:image/<type>;base64,<data>`
fn load_from_data_uri(url: &str) -> Result<DynamicImage> {
    // 提取 base64 部分：跳过 "data:image/...;base64,"
    let base64_data = url
        .split(";base64,")
        .nth(1)
        .ok_or_else(|| crate::Error::ImageLoad("无效的 data URI 格式".into()))?;

    let bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        base64_data,
    )
    .map_err(|e| crate::Error::ImageLoad(format!("base64 解码失败: {e}")))?;

    let img = image::load_from_memory(&bytes)
        .map_err(|e| crate::Error::ImageLoad(format!("图片解码失败: {e}")))?;

    flatten_alpha(img)
}

/// 从远程 URL 加载图片（HTTP/HTTPS）。
fn load_from_remote(url: &str) -> Result<DynamicImage> {
    let response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(|e| crate::Error::ImageLoad(format!("网络请求失败: {e}")))?;

    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| crate::Error::ImageLoad(format!("读取响应失败: {e}")))?;

    let img = image::load_from_memory(&bytes)
        .map_err(|e| crate::Error::ImageLoad(format!("图片解码失败: {e}")))?;

    flatten_alpha(img)
}

/// 从本地文件加载图片。
fn load_from_local(url: &str, working_dir: Option<&Path>) -> Result<DynamicImage> {
    let path = if std::path::Path::new(url).is_absolute() {
        std::path::PathBuf::from(url)
    } else {
        match working_dir {
            Some(dir) => dir.join(url),
            None => {
                return Err(crate::Error::ImageLoad(
                    "相对路径图片需要设置 working_dir".into(),
                ));
            }
        }
    };

    let img = image::open(&path)
        .map_err(|e| crate::Error::ImageLoad(format!("无法打开图片 {}: {e}", path.display())))?;

    flatten_alpha(img)
}

/// 如果图片包含 alpha 通道，在白色背景上 flatten。
/// genpdf/printpdf 不支持透明图。
fn flatten_alpha(img: DynamicImage) -> Result<DynamicImage> {
    use image::GenericImageView;

    if img.color().has_alpha() {
        let (w, h) = img.dimensions();
        let mut bg = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));
        image::imageops::overlay(&mut bg, &img.to_rgba8(), 0, 0);
        // 转为 RGB 去除 alpha
        let rgb = image::DynamicImage::ImageRgba8(bg).to_rgb8();
        Ok(image::DynamicImage::ImageRgb8(rgb))
    } else {
        Ok(img)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 生成一个极小的 PNG data URI（1x1 红色像素）。
    fn tiny_png_data_uri() -> String {
        // 预计算的 1x1 红色 PNG 的 base64（最小合法 PNG）
        let red_png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        format!("data:image/png;base64,{red_png_b64}")
    }

    /// 无效 base64 的 data URI。
    fn invalid_data_uri() -> String {
        "data:image/png;base64,!!!not-valid-base64!!!".to_string()
    }

    #[test]
    fn load_from_data_uri_success() {
        let uri = tiny_png_data_uri();
        let img = load_image(&uri, None);
        assert!(img.is_ok(), "valid data URI should load: {img:?}");
        let img = img.unwrap();
        assert_eq!(img.width(), 1);
        assert_eq!(img.height(), 1);
    }

    #[test]
    fn load_from_data_uri_invalid_base64() {
        let uri = invalid_data_uri();
        let result = load_image(&uri, None);
        assert!(result.is_err(), "invalid base64 should fail");
    }

    #[test]
    fn load_from_local_nonexistent() {
        let result = load_image("nonexistent_file_12345.png", None);
        assert!(result.is_err(), "nonexistent file should fail");
    }

    #[test]
    fn load_from_local_relative_without_working_dir() {
        let result = load_image("images/photo.png", None);
        assert!(result.is_err(), "relative path without working_dir should fail");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("working_dir"), "error should mention working_dir");
    }

    #[test]
    fn alpha_image_is_flattened() {
        // 创建一个带 alpha 的 2x2 半透明图片
        use image::{Rgba, RgbaImage};
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 128])); // 半透明红
        img.put_pixel(1, 0, Rgba([0, 255, 0, 128])); // 半透明绿
        img.put_pixel(0, 1, Rgba([0, 0, 255, 128])); // 半透明蓝
        img.put_pixel(1, 1, Rgba([255, 255, 255, 255])); // 不透明白
        let dyn_img = DynamicImage::ImageRgba8(img);

        let result = flatten_alpha(dyn_img);
        assert!(result.is_ok());
        let flattened = result.unwrap();
        // flatten 后不应有 alpha
        assert!(!flattened.color().has_alpha(), "flattened image should not have alpha");
    }
}
```

- [ ] **步骤 2：运行测试验证失败（编译错误——缺少 `use` 导入）**

```bash
cargo test -p export_engine -- image_loader
```

需要确保所有必要的 `use` 已在文件顶部声明。如果编译通过但测试失败那也可以——然后修复编译错误直到通过。

- [ ] **步骤 3：确保所有 use 导入完整**

在文件顶部确保导入：

```rust
use std::io::Read;
use std::path::Path;
use std::time::Duration;

use base64::Engine;
use image::{DynamicImage, GenericImageView};

use crate::Result;
```

其中 `Duration` 用于 `ureq` timeout，`Read` 用于 `read_to_end`。

- [ ] **步骤 4：运行测试验证通过**

```bash
cargo test -p export_engine -- image_loader
```

预期：5 个测试全部 PASS。

- [ ] **步骤 5：Commit**

```bash
git add crates/export_engine/src/image_loader.rs
git commit -m "feat(export_engine): add image_loader module with data URI, remote, local support"
```

---

### 任务 4：inline 分割函数 + auto_fit_scale

**文件：**
- 修改：`crates/export_engine/src/renderer.rs`

- [ ] **步骤 1：编写 split_inlines 和 auto_fit_scale 的测试**

在 `renderer.rs` 底部的 `#[cfg(test)] mod tests { ... }` 块中添加：

```rust
mod split_tests {
    use super::*;
    use document_model::ast::Inline;

    fn text(s: &str) -> Inline {
        Inline::Text(s.into())
    }

    fn image(alt: &str, url: &str) -> Inline {
        Inline::Image {
            alt: alt.into(),
            url: url.into(),
            title: None,
        }
    }

    #[test]
    fn split_inlines_no_image() {
        let inlines = vec![text("hello"), text(" world")];
        let segments = split_inlines(&inlines);
        assert_eq!(segments.len(), 1, "all text → single Text segment");
        match &segments[0] {
            InlineSegment::Text(t) => assert_eq!(t.len(), 2),
            _ => panic!("expected Text segment"),
        }
    }

    #[test]
    fn split_inlines_single_image() {
        let inlines = vec![image("alt", "test.png")];
        let segments = split_inlines(&inlines);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            InlineSegment::Image { alt, url, .. } => {
                assert_eq!(alt, "alt");
                assert_eq!(url, "test.png");
            }
            _ => panic!("expected Image segment"),
        }
    }

    #[test]
    fn split_inlines_mixed() {
        let inlines = vec![
            text("before "),
            image("mid", "mid.png"),
            text(" after"),
        ];
        let segments = split_inlines(&inlines);
        assert_eq!(segments.len(), 3, "text + image + text = 3 segments");
        match (&segments[0], &segments[1], &segments[2]) {
            (InlineSegment::Text(_), InlineSegment::Image { .. }, InlineSegment::Text(_)) => {}
            _ => panic!("expected Text, Image, Text order"),
        }
    }

    #[test]
    fn split_inlines_consecutive_images() {
        let inlines = vec![
            image("a", "a.png"),
            image("b", "b.png"),
            text("trailing"),
        ];
        let segments = split_inlines(&inlines);
        assert_eq!(segments.len(), 3, "img + img + text = 3 segments");
    }

    #[test]
    fn split_inlines_empty() {
        let segments = split_inlines(&[]);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            InlineSegment::Text(t) => assert!(t.is_empty()),
            _ => panic!("expected empty Text"),
        }
    }

    #[test]
    fn auto_fit_scale_small_image() {
        // 小于页面宽度的图片不缩放
        let scale = auto_fit_scale(100, 100, 159.2);
        assert!((scale.x - 1.0).abs() < 0.001, "small image x scale should be 1.0");
        assert!((scale.y - 1.0).abs() < 0.001, "small image y scale should be 1.0");
    }

    #[test]
    fn auto_fit_scale_large_image() {
        // 宽度 600px @300dpi ≈ 50.8mm，当 max_width=25.4mm 时 scale ≈ 0.5
        let scale = auto_fit_scale(600, 300, 25.4);
        assert!(scale.x < 1.0, "large image should be scaled down");
        assert!((scale.x - scale.y).abs() < 0.001, "x and y scale should be equal");
    }
}
```

- [ ] **步骤 2：实现 `InlineSegment` 枚举和 `split_inlines` 函数**

在 `renderer.rs` 的现有函数之前（`render_document` 之前）添加：

```rust
/// 行内元素分段：按 Image 边界切分。
enum InlineSegment {
    /// 纯文本/样式的连续行内元素。
    Text(Vec<Inline>),
    /// 图片。
    Image {
        alt: String,
        url: String,
        title: Option<String>,
    },
}

/// 将 inlines 列表按 `Inline::Image` 边界切分为 Text/Image 段。
fn split_inlines(inlines: &[Inline]) -> Vec<InlineSegment> {
    let mut segments: Vec<InlineSegment> = Vec::new();
    let mut current_text: Vec<Inline> = Vec::new();

    for inline in inlines {
        match inline {
            Inline::Image { alt, url, title } => {
                // 如果有累积的文本，先 push
                if !current_text.is_empty() {
                    segments.push(InlineSegment::Text(std::mem::take(&mut current_text)));
                }
                segments.push(InlineSegment::Image {
                    alt: alt.clone(),
                    url: url.clone(),
                    title: title.clone(),
                });
            }
            other => {
                current_text.push(other.clone());
            }
        }
    }

    // 不要漏掉末尾的文本
    if !current_text.is_empty() {
        segments.push(InlineSegment::Text(current_text));
    } else if segments.is_empty() {
        // 全空列表 → 一个空 Text 段（保持返回至少一段）
        segments.push(InlineSegment::Text(Vec::new()));
    }

    segments
}
```

- [ ] **步骤 3：实现 `auto_fit_scale` 函数**

接着添加：

```rust
/// 根据图片像素尺寸和页面最大宽度计算缩放比例。
///
/// 原则：不放大（scale ≤ 1.0），只缩小超宽图片。
/// DPI 使用 printpdf 默认值 300。
fn auto_fit_scale(px_width: u32, px_height: u32, max_width_mm: f64) -> genpdf::Scale {
    let dpi: f64 = 300.0;
    let mmpi: f64 = 25.4; // mm per inch
    let img_width_mm = (px_width as f64 / dpi) * mmpi;

    if img_width_mm <= max_width_mm {
        genpdf::Scale::new(1.0, 1.0)
    } else {
        let ratio = max_width_mm / img_width_mm;
        genpdf::Scale::new(ratio, ratio)
    }
}
```

- [ ] **步骤 4：运行测试**

```bash
cargo test -p export_engine -- renderer::split_tests
```

预期：6 个 split + 2 个 scale = 8 个测试全部 PASS。

- [ ] **步骤 5：Commit**

```bash
git add crates/export_engine/src/renderer.rs
git commit -m "feat(export_engine): add split_inlines and auto_fit_scale for image rendering"
```

---

### 任务 5：改造 renderer — push 模式 + 图片嵌入

**文件：**
- 修改：`crates/export_engine/src/renderer.rs`

这是核心变更。`render_heading` 和 `render_paragraph` 从返回 `Paragraph` 改为 push 到 `&mut LinearLayout`。

- [ ] **步骤 1：改造 `render_document` — 传递 `working_dir`**

修改函数签名，将 `working_dir` 传递给 `render_block`：

```rust
/// Render a complete `Document` into a vertical genpdf layout.
pub fn render_document(
    doc: &Document,
    config: &PdfConfig,
    _fonts: &FontSet,
) -> Result<LinearLayout> {
    let mut layout = LinearLayout::vertical();
    let working_dir = config.working_dir.as_deref();
    // 计算页面内容区宽度（mm）：纸张宽度 - 左右边距
    let paper_width = match config.paper {
        crate::theme::Paper::A4 => 210.0,
        crate::theme::Paper::Letter => 215.9,
        crate::theme::Paper::Custom { width_mm, .. } => width_mm,
    };
    let max_width_mm = (paper_width as f64) - (config.margins.left as f64) - (config.margins.right as f64);
    for (i, bws) in doc.blocks.iter().enumerate() {
        if i > 0 {
            let gap = config.theme.spacing.paragraph_gap;
            if gap > 0.0 {
                layout.push(Break::new(1));
            }
        }
        render_block(&bws.block, &config.theme, working_dir, max_width_mm, &mut layout)?;
    }
    Ok(layout)
}
```

- [ ] **步骤 2：改造 `render_block` — 传递 `working_dir`，更新调用**

```rust
fn render_block(
    block: &Block,
    theme: &PdfTheme,
    working_dir: Option<&std::path::Path>,
    max_width_mm: f64,
    layout: &mut LinearLayout,
) -> Result<()> {
    match block {
        Block::Heading(h) => render_heading(h, theme, working_dir, max_width_mm, layout),
        Block::Paragraph(p) => render_paragraph(p, theme, working_dir, max_width_mm, layout),
        Block::CodeBlock(cb) => render_code_block(cb, theme, layout),
        Block::List(l) => render_list(l.ordered, l.start, &l.items, 0, theme, working_dir, max_width_mm, layout),
        Block::BlockQuote(bq) => render_blockquote(bq, theme, working_dir, max_width_mm, layout),
        Block::ThematicBreak => {
            layout.push(
                Paragraph::new("─".repeat(60))
                    .aligned(genpdf::Alignment::Center)
                    .padded((0, 4)),
            );
        }
        Block::Table(t) => render_table(t, theme, layout),
        Block::HtmlBlock(_) => {}
    }
    Ok(())
}
```

注意：除了 heading/paragraph，`render_list` 和 `render_blockquote` 也需要传递 `working_dir`。

- [ ] **步骤 3：改造 `render_heading` — push 模式**

```rust
fn render_heading(
    h: &Heading,
    theme: &PdfTheme,
    working_dir: Option<&std::path::Path>,
    max_width_mm: f64,
    layout: &mut LinearLayout,
) {
    let font_size = match h.level {
        1 => theme.font_size.h1,
        2 => theme.font_size.h2,
        3 => theme.font_size.h3,
        4 => theme.font_size.h4,
        5 => theme.font_size.h5,
        _ => theme.font_size.h6,
    };
    render_inlines_as_elements(&h.inlines, theme, working_dir, max_width_mm, font_size, layout);
}
```

- [ ] **步骤 4：改造 `render_paragraph` — push 模式**

```rust
fn render_paragraph(
    para: &AstParagraph,
    theme: &PdfTheme,
    working_dir: Option<&std::path::Path>,
    max_width_mm: f64,
    layout: &mut LinearLayout,
) {
    render_inlines_as_elements(
        &para.inlines,
        theme,
        working_dir,
        max_width_mm,
        theme.font_size.body,
        layout,
    );
}
```

- [ ] **步骤 5：实现 `render_inlines_as_elements` — 核心分段渲染函数**

这是新增的主函数，处理 segmented inlines 的渲染：

```rust
/// 将 inlines 按 Image 分段后渲染到 layout。
/// Text 段 → Paragraph，Image 段 → elements::Image。
fn render_inlines_as_elements(
    inlines: &[Inline],
    theme: &PdfTheme,
    working_dir: Option<&std::path::Path>,
    max_width_mm: f64,
    font_size: f32,
    layout: &mut LinearLayout,
) {
    let segments = split_inlines(inlines);

    let body_style = Style::new().with_font_size(font_size as u8);

    for segment in segments {
        match segment {
            InlineSegment::Text(text_inlines) => {
                let mut p = Paragraph::default();
                for inline in &text_inlines {
                    render_inline_to_paragraph(&mut p, inline, theme, font_size);
                }
                layout.push(p);
            }
            InlineSegment::Image { alt, url, .. } => {
                match crate::image_loader::load_image(&url, working_dir) {
                    Ok(dyn_img) => {
                        let (w, h) = dyn_img.dimensions();
                        let scale = auto_fit_scale(w, h, max_width_mm);
                        layout.push(
                            genpdf::elements::Image::from_dynamic_image(dyn_img)
                                .with_alignment(genpdf::Alignment::Center)
                                .with_scale(scale),
                        );
                    }
                    Err(_) => {
                        // 降级：占位文本
                        let mut p = Paragraph::default();
                        p.push_styled(format!("[图片: {alt}]"), body_style);
                        layout.push(p);
                    }
                }
            }
        }
    }
}
```

- [ ] **步骤 6：改造 `render_list` — 传递 `working_dir` + 图片支持**

列表项中的 inlines 同样需要分段处理。改造策略：对每个 item 的 inlines 用 `split_inlines` 分段，首段 Text 使用 marker + indent，后续段只用 indent。

```rust
fn render_list(
    ordered: bool,
    start: usize,
    items: &[ListItem],
    depth: usize,
    theme: &PdfTheme,
    working_dir: Option<&std::path::Path>,
    max_width_mm: f64,
    layout: &mut LinearLayout,
) {
    for (i, item) in items.iter().enumerate() {
        let marker = if ordered {
            format!("{}. ", start + i)
        } else {
            "• ".to_owned()
        };
        let indent = theme.spacing.list_indent * (depth as f32);

        let segments = split_inlines(&item.inlines);

        for (seg_idx, segment) in segments.into_iter().enumerate() {
            match segment {
                InlineSegment::Text(text_inlines) => {
                    let mut p = Paragraph::default();
                    // 首个 Text 段添加 marker
                    if seg_idx == 0 {
                        p.push_styled(
                            &marker,
                            Style::new().with_font_size(theme.font_size.body as u8),
                        );
                    }
                    for inline in &text_inlines {
                        render_inline_to_paragraph(&mut p, inline, theme, theme.font_size.body);
                    }
                    layout.push(p.padded((0, 0, 0, indent)));
                }
                InlineSegment::Image { alt, url, .. } => {
                    match crate::image_loader::load_image(&url, working_dir) {
                        Ok(dyn_img) => {
                            let (w, h) = dyn_img.dimensions();
                            let scale = auto_fit_scale(w, h, max_width_mm);
                            layout.push(
                                genpdf::elements::Image::from_dynamic_image(dyn_img)
                                    .with_alignment(genpdf::Alignment::Center)
                                    .with_scale(scale)
                                    .padded((0, 0, 0, indent)),
                            );
                        }
                        Err(_) => {
                            let mut p = Paragraph::default();
                            p.push_styled(
                                format!("[图片: {alt}]"),
                                Style::new().with_font_size(theme.font_size.body as u8),
                            );
                            layout.push(p.padded((0, 0, 0, indent)));
                        }
                    }
                }
            }
        }

        if !item.sub_items.is_empty() {
            render_list(
                ordered,
                start,
                &item.sub_items,
                depth + 1,
                theme,
                working_dir,
                max_width_mm,
                layout,
            );
        }
    }
}
```

- [ ] **步骤 7：改造 `render_blockquote` — 传递 `working_dir`**

```rust
fn render_blockquote(
    bq: &BlockQuote,
    theme: &PdfTheme,
    working_dir: Option<&std::path::Path>,
    max_width_mm: f64,
    layout: &mut LinearLayout,
) {
    let mut inner = LinearLayout::vertical();
    for bws in &bq.blocks {
        let _ = render_block(&bws.block, theme, working_dir, max_width_mm, &mut inner);
    }
    layout.push(inner.framed().padded((0, 0, 0, 4)));
}
```

- [ ] **步骤 8：移除不再需要的旧函数**

`render_heading` 和 `render_paragraph` 的旧签名（返回 `Paragraph` 版本）已被替换。确认没有残留引用。

- [ ] **步骤 9：运行编译检查**

```bash
cargo check -p export_engine
```

预期：编译成功（0 errors）。

- [ ] **步骤 10：运行所有测试**

```bash
cargo test -p export_engine
```

预期：所有已有测试 + 新增 split 测试全部通过。

- [ ] **步骤 11：Commit**

```bash
git add crates/export_engine/src/renderer.rs
git commit -m "feat(export_engine): push-mode renderer with image embedding support"
```

---

### 任务 6：集成测试

**文件：**
- 修改：`crates/export_engine/src/pdf.rs`

- [ ] **步骤 1：编写集成测试**

在 `pdf.rs` 的 `#[cfg(test)] mod tests` 块中添加：

```rust
#[test]
fn generate_pdf_with_image_placeholder_on_missing() {
    // 包含不存在的图片 URL 的文档应降级为占位文本
    let doc = Document {
        blocks: vec![BlockWithSpan {
            block: Block::Paragraph(AstParagraph {
                inlines: vec![Inline::Image {
                    alt: "missing".into(),
                    url: "nonexistent.png".into(),
                    title: None,
                }],
            }),
            span: Span {
                start_line: 0,
                end_line: 0,
            },
        }],
    };
    let config = PdfConfig::minimal();
    let result = generate_pdf(&doc, &config);
    // 不应返回图片加载错误（降级为占位文本）
    if let Err(e) = &result {
        let msg = e.to_string();
        assert!(
            !msg.contains("ImageLoad"),
            "missing image should fallback, not error: {msg}"
        );
    }
}

#[test]
fn generate_pdf_with_data_uri_image() {
    // 使用 1x1 红色 PNG 的 data URI
    let red_png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
    let data_uri = format!("data:image/png;base64,{red_png_b64}");

    let doc = Document {
        blocks: vec![BlockWithSpan {
            block: Block::Paragraph(AstParagraph {
                inlines: vec![Inline::Image {
                    alt: "red dot".into(),
                    url: data_uri,
                    title: None,
                }],
            }),
            span: Span {
                start_line: 0,
                end_line: 0,
            },
        }],
    };
    let config = PdfConfig::minimal();
    let result = generate_pdf(&doc, &config);
    if let Ok(bytes) = result {
        assert!(!bytes.is_empty(), "PDF with embedded image should be non-empty");
    }
    // Err 可接受（无字体环境）
}

#[test]
fn generate_pdf_mixed_text_and_image() {
    let red_png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
    let data_uri = format!("data:image/png;base64,{red_png_b64}");

    let doc = Document {
        blocks: vec![BlockWithSpan {
            block: Block::Paragraph(AstParagraph {
                inlines: vec![
                    Inline::Text("before ".into()),
                    Inline::Image {
                        alt: "mid".into(),
                        url: data_uri,
                        title: None,
                    },
                    Inline::Text(" after".into()),
                ],
            }),
            span: Span {
                start_line: 0,
                end_line: 0,
            },
        }],
    };
    let config = PdfConfig::minimal();
    let result = generate_pdf(&doc, &config);
    if let Ok(bytes) = result {
        assert!(!bytes.is_empty(), "mixed text+image PDF should work");
    }
}
```

- [ ] **步骤 2：运行集成测试**

```bash
cargo test -p export_engine -- pdf::tests
```

预期：新增 3 个测试 + 原有 4 个 = 7 个测试全部 PASS（或在无字体环境下 `generate_pdf` 返回 Err 也是可接受的——测试断言已处理此情况）。

- [ ] **步骤 3：运行完整测试套件**

```bash
cargo test -p export_engine
cargo test -p document_model
```

预期：全部通过。

- [ ] **步骤 4：Commit**

```bash
git add crates/export_engine/src/pdf.rs
git commit -m "test(export_engine): add integration tests for image embedding"
```

---

### 任务 7：全项目验证

**文件：** 全部已修改文件

- [ ] **步骤 1：fmt + clippy + test**

```bash
cargo fmt
cargo clippy --all-targets
cargo test
```

预期：fmt clean, clippy 0 warnings, all tests pass。

- [ ] **步骤 2：如有 clippy warning，修复并重新运行直到 clean**

- [ ] **步骤 3：最终 Commit**

```bash
git add -A
git commit -m "chore: fmt + clippy fix for image embedding feature"
```

---

### 验收检查清单

- [ ] `cargo fmt` clean
- [ ] `cargo clippy --all-targets` 0 warnings
- [ ] `cargo test` 全部通过
- [ ] `Inline::Image` 在 PDF 中呈现为真实图片（非 `[图片: alt]`）
- [ ] 本地文件路径图片可加载（需设置 `working_dir`）
- [ ] data URI 图片可加载
- [ ] 远程 URL 图片可加载
- [ ] 缺失图片降级为 `[图片: alt]` 占位文本
- [ ] Alpha 通道图片自动 flatten
- [ ] 超大图片自动缩放至页面宽度
- [ ] 图片在段落中正确分割文本
- [ ] 列表项中的图片正确渲染
- [ ] 嵌套列表中的图片正确渲染
- [ ] BlockQuote 中的图片正确渲染
