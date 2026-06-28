//! AST → egui widget 渲染。
//!
//! 对外暴露 `render(ui, doc)`，按 Block/Inline 分发。
//! 注意：用 `egui`（非 `eframe::egui`），markdown_renderer 只依赖 egui crate。

use document_model::ast::{
    Alignment, Block, BlockQuote, CodeBlock, Document, Heading, Inline, ListItem, Paragraph, Table,
    TableCell,
};

use std::path::Path;
use std::sync::Arc;

use crate::image_cache::ImageCache;
use crate::source::SourceHighlighter;

/// 将 `Document` 渲染到 egui UI。
pub fn render(
    ui: &mut egui::Ui,
    doc: &Document,
    image_cache: &mut ImageCache,
    working_dir: Option<&Path>,
) {
    image_cache.poll_pending(ui.ctx());
    for bws in &doc.blocks {
        render_block(ui, &bws.block, image_cache, working_dir);
    }
}

fn render_block(
    ui: &mut egui::Ui,
    block: &Block,
    image_cache: &mut ImageCache,
    working_dir: Option<&Path>,
) {
    match block {
        Block::Heading(h) => render_heading(ui, h, image_cache, working_dir),
        Block::Paragraph(p) => render_paragraph(ui, p, image_cache, working_dir),
        Block::CodeBlock(cb) => render_code_block(ui, cb),
        Block::List(l) => render_list(
            ui,
            l.ordered,
            l.start,
            &l.items,
            0,
            image_cache,
            working_dir,
        ),
        Block::BlockQuote(bq) => render_blockquote(ui, bq, image_cache, working_dir),
        Block::ThematicBreak => {
            ui.separator();
        }
        Block::Table(t) => render_table(ui, t),
        Block::HtmlBlock(s) => {
            html_renderer::render_block_html(ui, s);
        }
    }
}

fn render_heading(
    ui: &mut egui::Ui,
    h: &Heading,
    image_cache: &mut ImageCache,
    working_dir: Option<&Path>,
) {
    let font_id = heading_font_id(ui, h.level);
    ui.horizontal_wrapped(|ui| {
        render_inlines(ui, &h.inlines, &font_id, image_cache, working_dir);
    });
}

fn heading_font_id(ui: &egui::Ui, level: u8) -> egui::FontId {
    // 用 heading 样式的字号，strong 权重
    let heading_style = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Heading)
        .cloned()
        .unwrap_or_else(|| egui::FontId::monospace(20.0));
    match level {
        1 => heading_style,
        2 => egui::FontId::new(24.0, heading_style.family),
        3 => egui::FontId::new(20.0, heading_style.family),
        4 => egui::FontId::new(18.0, heading_style.family),
        5 => egui::FontId::new(16.0, heading_style.family),
        _ => egui::FontId::new(14.0, heading_style.family),
    }
}

fn render_paragraph(
    ui: &mut egui::Ui,
    p: &Paragraph,
    image_cache: &mut ImageCache,
    working_dir: Option<&Path>,
) {
    let font_id = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .cloned()
        .unwrap_or_else(egui::FontId::default);

    // 按 Image 边界切分：文本段走 horizontal_wrapped，图片段独立全宽渲染
    let segments = split_inlines_by_image(&p.inlines);
    ui.vertical(|ui| {
        for seg in &segments {
            match seg {
                InlineSegment::Text(inlines) => {
                    if !inlines.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            render_inlines(ui, inlines, &font_id, image_cache, working_dir);
                        });
                    }
                }
                InlineSegment::Image { alt, url, .. } => {
                    render_image_block(ui, alt, url, image_cache, working_dir);
                }
            }
        }
    });
}

/// 按 `Inline::Image` 边界切分 inlines 为文本/图片段。
#[derive(Debug, Clone)]
enum InlineSegment {
    Text(Vec<Inline>),
    Image { alt: String, url: String },
}

fn split_inlines_by_image(inlines: &[Inline]) -> Vec<InlineSegment> {
    let mut segments: Vec<InlineSegment> = Vec::new();
    let mut current_text: Vec<Inline> = Vec::new();

    for inline in inlines {
        match inline {
            Inline::Image { alt, url, .. } => {
                if !current_text.is_empty() {
                    segments.push(InlineSegment::Text(std::mem::take(&mut current_text)));
                }
                segments.push(InlineSegment::Image {
                    alt: alt.clone(),
                    url: url.clone(),
                });
            }
            other => {
                current_text.push(other.clone());
            }
        }
    }

    if !current_text.is_empty() {
        segments.push(InlineSegment::Text(current_text));
    }

    segments
}

/// 逐 inline 渲染片段，支持 emph/strong/code/link/image 样式。
fn render_inlines(
    ui: &mut egui::Ui,
    inlines: &[Inline],
    font_id: &egui::FontId,
    image_cache: &mut ImageCache,
    working_dir: Option<&Path>,
) {
    for inline in inlines {
        match inline {
            Inline::Text(s) => {
                ui.label(egui::RichText::new(s).font(font_id.clone()));
            }
            Inline::Emph(inner) => {
                ui.label(
                    egui::RichText::new(inlines_to_plain(inner))
                        .italics()
                        .font(font_id.clone()),
                );
            }
            Inline::Strong(inner) => {
                ui.label(
                    egui::RichText::new(inlines_to_plain(inner))
                        .strong()
                        .font(font_id.clone()),
                );
            }
            Inline::Code(s) => {
                ui.label(egui::RichText::new(s).code().font(font_id.clone()));
            }
            Inline::Link { text, url, .. } => {
                ui.hyperlink_to(
                    egui::RichText::new(inlines_to_plain(text)).font(font_id.clone()),
                    url,
                );
            }
            Inline::Image { alt, url, .. } => {
                render_image_block(ui, alt, url, image_cache, working_dir);
            }
            Inline::Html(s) => {
                html_renderer::render_inline_html(ui, s, font_id);
            }
            Inline::SoftBreak => {
                ui.label(egui::RichText::new(" ").font(font_id.clone()));
            }
            Inline::HardBreak => {
                ui.label(egui::RichText::new("\n").font(font_id.clone()));
            }
        }
    }
}

/// 渲染图片 block：从缓存加载并自动缩放适配宽度。
///
/// 纹理句柄仅首次创建时通过 `load_texture` 注册到 egui，
/// 后续帧直接使用缓存的句柄避免每帧重传像素数据导致的闪烁。
/// **必须存储 `TextureHandle` 而非 `TextureId`**：handle 的 Drop 会释放纹理。
fn render_image_block(
    ui: &mut egui::Ui,
    alt: &str,
    url: &str,
    image_cache: &mut ImageCache,
    working_dir: Option<&Path>,
) {
    let Some(color_image) = image_cache.get_or_load(url, working_dir) else {
        // 加载失败，显示占位文本
        let font_id = ui
            .style()
            .text_styles
            .get(&egui::TextStyle::Body)
            .cloned()
            .unwrap_or_else(egui::FontId::default);
        ui.label(egui::RichText::new(format!("[图片: {alt}]")).font(font_id));
        return;
    };

    let texture_size = egui::vec2(color_image.size[0] as f32, color_image.size[1] as f32);

    // 优先使用缓存的纹理句柄，避免每帧 load_texture 触发 GPU 重上传（闪烁）
    // 注意：必须存储 TextureHandle 而非 TextureId，handle 的 Drop 会释放纹理
    let texture_id = if let Some(id) = image_cache.get_texture_id(url) {
        id
    } else {
        let image_data = egui::ImageData::Color(Arc::clone(&color_image));
        let texture_name = format!("img_{:016x}", hash_src(url));
        let handle =
            ui.ctx()
                .load_texture(texture_name, image_data, egui::TextureOptions::default());
        let id = handle.id();
        image_cache.register_texture_handle(url, handle);
        id
    };

    let available = ui.available_width().min(texture_size.x);
    let scale = if texture_size.x > 0.0 {
        (available / texture_size.x).min(1.0)
    } else {
        1.0
    };
    let display_size = egui::vec2(
        (texture_size.x * scale).max(1.0),
        (texture_size.y * scale).max(1.0),
    );
    ui.add_sized(
        display_size,
        egui::Image::from_texture((texture_id, texture_size)),
    );
}

fn render_code_block(ui: &mut egui::Ui, cb: &CodeBlock) {
    // 检测 Mermaid 图表并渲染为 SVG
    if mermaid_renderer::MermaidRenderer::is_mermaid(cb.language.as_deref()) {
        if let Some(image_data) = render_mermaid_to_egui_image(&cb.content) {
            let content_hash = hash_src(&cb.content);
            // 从 image_data 提取尺寸（在 move 之前）
            let texture_size = match &image_data {
                egui::ImageData::Color(img) => egui::vec2(img.size[0] as f32, img.size[1] as f32),
            };
            // 优先使用缓存的纹理 ID，避免每帧 load_texture 触发 GPU 重上传
            let texture_id = MERMAID_TEXTURE_IDS.with(|cache| {
                if let Some(&id) = cache.borrow().get(&content_hash) {
                    id
                } else {
                    let name = format!("mermaid_{:016x}", content_hash);
                    let handle =
                        ui.ctx()
                            .load_texture(name, image_data, egui::TextureOptions::default());
                    let id = handle.id();
                    cache.borrow_mut().insert(content_hash, id);
                    id
                }
            });
            let available = ui.available_width().min(texture_size.x);
            let scale = if texture_size.x > 0.0 {
                available / texture_size.x
            } else {
                1.0
            };
            let display_size = egui::vec2(available, texture_size.y * scale);
            ui.add_sized(
                display_size,
                egui::Image::from_texture((texture_id, texture_size)),
            );
            return;
        }
    }

    let highlighter = SourceHighlighter::new().ok();
    if let Some(h) = &highlighter {
        let lines = h.highlight(&cb.content, cb.language.as_deref());
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                for line in &lines {
                    ui.horizontal(|ui| {
                        for (style, text) in line {
                            let color = egui::Color32::from_rgb(
                                style.foreground.r,
                                style.foreground.g,
                                style.foreground.b,
                            );
                            ui.label(egui::RichText::new(*text).color(color).monospace());
                        }
                    });
                }
            });
    } else {
        // fallback：不高亮
        let mut text = cb.content.clone();
        ui.add(
            egui::TextEdit::multiline(&mut text)
                .code_editor()
                .interactive(false)
                .desired_width(f32::INFINITY),
        );
    }
}

/// 渲染列表。签名传 `&[ListItem]` 引用避免递归 clone（参考阶段 1 serialize.rs 修复）。
#[allow(clippy::only_used_in_recursion)]
fn render_list(
    ui: &mut egui::Ui,
    ordered: bool,
    start: usize,
    items: &[ListItem],
    indent: usize,
    image_cache: &mut ImageCache,
    working_dir: Option<&Path>,
) {
    ui.vertical(|ui| {
        for (i, item) in items.iter().enumerate() {
            let marker = if ordered {
                format!("{}. ", start + i)
            } else {
                "• ".to_owned()
            };
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&marker).strong());
                ui.label(inlines_to_richtext(&item.inlines));
            });
            if !item.sub_items.is_empty() {
                ui.indent(egui::Id::new(format!("list_{indent}_{i}")), |ui| {
                    // 递归传 &item.sub_items（非父 List），避免无限递归
                    // 子列表序号从 1 开始，不应继承父级 start
                    render_list(
                        ui,
                        ordered,
                        1,
                        &item.sub_items,
                        indent + 1,
                        image_cache,
                        working_dir,
                    );
                });
            }
        }
    });
}

fn render_blockquote(
    ui: &mut egui::Ui,
    bq: &BlockQuote,
    image_cache: &mut ImageCache,
    working_dir: Option<&Path>,
) {
    egui::Frame::group(ui.style())
        .stroke(egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            for bws in &bq.blocks {
                render_block(ui, &bws.block, image_cache, working_dir);
            }
        });
}

fn render_table(ui: &mut egui::Ui, t: &Table) {
    // 用指针地址生成唯一 id，避免同帧多表格冲突
    let table_id = egui::Id::new(format!("table_{:p}", t as *const Table));
    egui::Grid::new(table_id).striped(true).show(ui, |ui| {
        // 表头（应用对齐）
        for (col_idx, cell) in t.header.iter().enumerate() {
            let align = t.alignments.get(col_idx).copied().flatten();
            render_table_cell(ui, cell, align, true);
        }
        ui.end_row();
        // 数据行
        for row in &t.rows {
            for (col_idx, cell) in row.iter().enumerate() {
                let align = t.alignments.get(col_idx).copied().flatten();
                render_table_cell(ui, cell, align, false);
            }
            ui.end_row();
        }
    });
}

/// 渲染表格单元格，应用对齐。
///
/// egui 0.34 API：`RichText::into_layout_job` 为私有，改用
/// `WidgetText::into_galley`（公开）转换。再用 `Painter::galley` 绘制。
fn render_table_cell(
    ui: &mut egui::Ui,
    cell: &TableCell,
    align: Option<Alignment>,
    is_header: bool,
) {
    let richtext = inlines_to_richtext(&cell.inlines);
    let richtext = if is_header {
        richtext.strong()
    } else {
        richtext
    };
    let widget_text: egui::WidgetText = richtext.into();
    let galley = widget_text.into_galley(ui, None, f32::INFINITY, egui::FontSelection::Default);
    let (rect, response) = ui.allocate_at_least(galley.size(), egui::Sense::hover());
    let align_x = match align {
        Some(Alignment::Left) | None => egui::Align::LEFT,
        Some(Alignment::Center) => egui::Align::Center,
        Some(Alignment::Right) => egui::Align::RIGHT,
    };
    let pos = egui::Align2([align_x, egui::Align::TOP])
        .align_size_within_rect(galley.size(), rect)
        .min;
    ui.painter().galley(pos, galley, ui.visuals().text_color());
    let _ = response;
}

/// 将 Inline 列表转为 egui RichText（标题/表头用，emph/strong 退化为纯文本）。
fn inlines_to_richtext(inlines: &[Inline]) -> egui::RichText {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(s) => text.push_str(s),
            Inline::Emph(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Strong(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Code(s) => text.push_str(s),
            Inline::Link {
                text: link_text, ..
            } => text.push_str(&inlines_to_plain(link_text)),
            Inline::Image { alt, .. } => {
                text.push_str("[图片: ");
                text.push_str(alt);
                text.push(']');
            }
            Inline::Html(s) => text.push_str(s),
            Inline::SoftBreak => text.push('\n'),
            Inline::HardBreak => text.push('\n'),
        }
    }
    egui::RichText::new(text)
}

/// 将 Inline 列表转为纯文本（无样式）。
fn inlines_to_plain(inlines: &[Inline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(s) => text.push_str(s),
            Inline::Emph(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Strong(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Code(s) => text.push_str(s),
            Inline::Link {
                text: link_text, ..
            } => text.push_str(&inlines_to_plain(link_text)),
            Inline::Image { alt, .. } => {
                // 与 inlines_to_richtext 保持一致：UI 占位文本
                text.push_str("[图片: ");
                text.push_str(alt);
                text.push(']');
            }
            Inline::Html(s) => text.push_str(s),
            Inline::SoftBreak => text.push('\n'),
            Inline::HardBreak => text.push('\n'),
        }
    }
    text
}

use std::collections::{HashMap, VecDeque};

/// 渲染缓存。key 为源码 hash，value 为解析后的 Document。
/// LRU 上限 10 条，超出丢弃最旧。
/// 无 Mutex（egui 单线程），用 &mut self。
pub struct RenderCache {
    cache: HashMap<u64, Document>,
    lru_keys: VecDeque<u64>,
    max_entries: usize,
}

impl RenderCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            lru_keys: VecDeque::new(),
            max_entries: 10,
        }
    }

    /// 解析源码，缓存结果。若同 hash 已缓存则直接返回。
    pub fn parse_cached(&mut self, src: &str) -> Document {
        let hash = hash_src(src);
        if let Some(doc) = self.cache.get(&hash) {
            // LRU 更新：移到队首
            self.lru_keys.retain(|&k| k != hash);
            self.lru_keys.push_front(hash);
            return doc.clone();
        }
        let doc = document_model::parse(src).unwrap_or(Document { blocks: vec![] });
        // 超限丢弃最旧
        while self.lru_keys.len() >= self.max_entries {
            if let Some(old_key) = self.lru_keys.pop_back() {
                self.cache.remove(&old_key);
            }
        }
        self.cache.insert(hash, doc.clone());
        self.lru_keys.push_front(hash);
        doc
    }

    /// 清空缓存（文档切换时调用）。
    pub fn clear(&mut self) {
        self.cache.clear();
        self.lru_keys.clear();
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}

fn hash_src(src: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut hasher);
    hasher.finish()
}

// ---------------------------------------------------------------------------
// Mermaid SVG 渲染辅助
// ---------------------------------------------------------------------------

use std::cell::RefCell;

thread_local! {
    static MERMAID_RENDERER: RefCell<mermaid_renderer::MermaidRenderer> =
        RefCell::new(mermaid_renderer::MermaidRenderer::new());
    /// Mermaid 纹理 ID 缓存，避免每帧 `load_texture` 重上传（与 ImageCache 同理）。
    static MERMAID_TEXTURE_IDS: RefCell<std::collections::HashMap<u64, egui::TextureId>> =
        RefCell::new(std::collections::HashMap::new());
}

/// 将 Mermaid 源码渲染为 egui ImageData。
/// 内部调用 MermaidRenderer 获取 SVG，再光栅化为位图。
fn render_mermaid_to_egui_image(source: &str) -> Option<egui::ImageData> {
    let svg = MERMAID_RENDERER.with(|r| r.borrow_mut().render(source).ok())?;
    render_svg_to_image_data(&svg, 1.0)
}

/// 使用 resvg + tiny-skia 将 SVG 字符串光栅化为 egui ColorImage。
fn render_svg_to_image_data(svg: &str, scale: f32) -> Option<egui::ImageData> {
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg, &options).ok()?;
    let size = tree.size();
    let width = (size.width() * scale).ceil() as u32;
    let height = (size.height() * scale).ceil() as u32;
    let width = width.max(1);
    let height = height.max(1);

    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Pixmap::data() 返回 &[u8]，字节序 RGBA 预乘 alpha
    let color_image =
        egui::ColorImage::from_rgba_premultiplied([width as usize, height as usize], pixmap.data());
    Some(egui::ImageData::Color(std::sync::Arc::new(color_image)))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn inlines_to_plain_text() {
        let inlines = vec![
            Inline::Text("hello".into()),
            Inline::SoftBreak,
            Inline::Text("world".into()),
        ];
        assert_eq!(inlines_to_plain(&inlines), "hello\nworld");
    }

    #[test]
    fn inlines_to_plain_emph_strong() {
        let inlines = vec![
            Inline::Emph(vec![Inline::Text("emph".into())]),
            Inline::Strong(vec![Inline::Text("strong".into())]),
        ];
        assert_eq!(inlines_to_plain(&inlines), "emphstrong");
    }

    #[test]
    fn inlines_to_plain_code() {
        let inlines = vec![Inline::Code("code".into())];
        assert_eq!(inlines_to_plain(&inlines), "code");
    }

    #[test]
    fn inlines_to_plain_link() {
        let inlines = vec![Inline::Link {
            text: vec![Inline::Text("text".into())],
            url: "https://x.com".into(),
            title: None,
        }];
        assert_eq!(inlines_to_plain(&inlines), "text");
    }

    #[test]
    fn inlines_to_plain_image() {
        let inlines = vec![Inline::Image {
            alt: "alt".into(),
            url: "https://x.com/x.png".into(),
            title: None,
        }];
        assert_eq!(inlines_to_plain(&inlines), "[图片: alt]");
    }
}
