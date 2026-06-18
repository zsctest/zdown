//! Markdown 解析：pulldown-cmark Event 流 → 嵌套 AST。
//!
//! pulldown-cmark 输出 flat event 流，嵌套结构通过 Start/End 事件对标记。
//! 本模块维护一个 builder 栈，遇到 Start 推入对应 builder，遇到 End 弹出并组装。
//!
//! pulldown-cmark 0.13 的 `Event::End` 携带 `TagEnd`（仅含类别，不含 url/title 等
//! 附带数据），因此 Link/Image 的 url/title 必须在 `Start` 时存入专用 frame。
//!
//! ## 已知限制（阶段 1）
//!
//! - **列表项内子 block 丢失**：list item 内的代码块、子段落、子引用会被静默丢弃
//!   （AST 的 `ListItem` 不容纳子 block，需在阶段 2 扩展 AST 后修复）
//! - **Loose list 多段落合并**：loose list 多段落会被合并为单一 inlines 数组，
//!   段落边界丢失（同根源）
//! - **嵌套列表类型信息丢失**：有序/无序混合嵌套列表的内层 ordered/start 信息
//!   不保留（`sub_items: Vec<ListItem>` 不携带 List 元信息）

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::{
    Result,
    ast::{
        Alignment, Block, BlockQuote, BlockWithSpan, CodeBlock, Document, Heading, Inline, List,
        ListItem, Paragraph, Span, Table, TableCell,
    },
};

/// 解析 Markdown 源码为 `Document`。
///
/// 启用 CommonMark + 表格 + 删除线扩展。
/// 解析失败目前不会发生（pulldown-cmark 容错），保留 `Result` 为后续扩展留接口。
pub fn parse(src: &str) -> Result<Document> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(src, options);

    let mut builder = BuilderStack::new();
    for event in parser {
        builder.handle(event);
    }

    Ok(builder.finish())
}

/// builder 栈，处理 pulldown-cmark 的 flat event 流。
struct BuilderStack {
    /// 从根到当前最深层的 builder 链。栈底是根（收集顶层 blocks）。
    stack: Vec<Frame>,
    /// 当前行号（0-based）。
    current_line: usize,
}

/// 栈中每一帧对应一个正在构建的容器。
#[derive(Debug)]
enum Frame {
    /// 文档根，收集顶层 blocks。
    Root(Vec<BlockWithSpan>),
    /// 段落，收集 inlines。
    Paragraph {
        inlines: Vec<Inline>,
        start_line: usize,
    },
    /// 标题，收集 inlines（level 已知）。
    Heading {
        level: u8,
        inlines: Vec<Inline>,
        start_line: usize,
    },
    /// 代码块（语言已知，content 累积中）。
    CodeBlock {
        language: Option<String>,
        content: String,
        start_line: usize,
    },
    /// 块级 HTML，累积原始 HTML 文本。
    HtmlBlock { content: String, start_line: usize },
    /// 引用块，收集内部 blocks。
    BlockQuote {
        blocks: Vec<BlockWithSpan>,
        start_line: usize,
    },
    /// 列表（ordered / start 已知，items 累积中）。
    List {
        ordered: bool,
        start: usize,
        items: Vec<ListItem>,
        start_line: usize,
    },
    /// 列表项，inlines 累积中，sub_items 在嵌套时填充。
    ListItem {
        inlines: Vec<Inline>,
        sub_items: Vec<ListItem>,
    },
    /// 表格，header / rows / alignments 累积中。
    Table {
        header: Vec<TableCell>,
        rows: Vec<Vec<TableCell>>,
        alignments: Vec<Option<Alignment>>,
        start_line: usize,
    },
    /// 表格行（累积中），cells 收集后并入 Table。
    TableRow(Vec<TableCell>),
    /// 表格单元格，inlines 累积中。
    TableCell(Vec<Inline>),
    /// 链接，收集内部 inlines，url/title 在 Start 时已知。
    Link {
        inlines: Vec<Inline>,
        url: String,
        title: Option<String>,
    },
    /// 图片，收集内部 inlines 作为 alt，url/title 在 Start 时已知。
    Image {
        inlines: Vec<Inline>,
        url: String,
        title: Option<String>,
    },
}

impl BuilderStack {
    fn new() -> Self {
        Self {
            stack: vec![Frame::Root(vec![])],
            current_line: 0,
        }
    }

    fn finish(mut self) -> Document {
        debug_assert!(
            matches!(self.stack.last(), Some(Frame::Root(_))),
            "解析结束时栈底应为 Root，实际: {:?}",
            self.stack.last()
        );
        // 栈底应是 Root，弹出即得顶层 blocks。
        match self.stack.pop() {
            Some(Frame::Root(blocks)) => Document { blocks },
            _ => Document { blocks: vec![] },
        }
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(s) => {
                let s = s.into_string();
                let newlines = s.chars().filter(|&c| c == '\n').count();
                self.current_line += newlines;
                self.push_inline(Inline::Text(s));
            }
            Event::Code(s) => self.push_inline(Inline::Code(s.into_string())),
            Event::Html(s) => self.push_html(s.into_string()),
            Event::InlineHtml(s) => self.push_inline(Inline::Html(s.into_string())),
            Event::SoftBreak => {
                self.current_line += 1;
                self.push_inline(Inline::SoftBreak);
            }
            Event::HardBreak => {
                self.current_line += 1;
                self.push_inline(Inline::HardBreak);
            }
            Event::Rule => {
                let span = Span {
                    start_line: self.current_line,
                    end_line: self.current_line,
                };
                self.push_block_with_span(Block::ThematicBreak, span);
            }
            Event::FootnoteReference(_) | Event::TaskListMarker(_) => {
                // 阶段 1 不支持脚注与任务列表，忽略。
            }
            Event::DisplayMath(_) | Event::InlineMath(_) => {
                // 阶段 1 不支持数学，忽略。
            }
        }
    }

    fn start_tag(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => self.stack.push(Frame::Paragraph {
                inlines: vec![],
                start_line: self.current_line,
            }),
            Tag::Heading { level, .. } => {
                let lvl = heading_level_to_u8(level);
                self.stack.push(Frame::Heading {
                    level: lvl,
                    inlines: vec![],
                    start_line: self.current_line,
                });
            }
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let s = lang.into_string();
                        if s.is_empty() { None } else { Some(s) }
                    }
                    CodeBlockKind::Indented => None,
                };
                self.stack.push(Frame::CodeBlock {
                    language,
                    content: String::new(),
                    start_line: self.current_line,
                });
            }
            Tag::HtmlBlock => self.stack.push(Frame::HtmlBlock {
                content: String::new(),
                start_line: self.current_line,
            }),
            Tag::BlockQuote(_) => self.stack.push(Frame::BlockQuote {
                blocks: vec![],
                start_line: self.current_line,
            }),
            Tag::List(start) => {
                let (ordered, start_num) = match start {
                    Some(n) => (true, n as usize),
                    None => (false, 0),
                };
                self.stack.push(Frame::List {
                    ordered,
                    start: start_num,
                    items: vec![],
                    start_line: self.current_line,
                });
            }
            Tag::Item => self.stack.push(Frame::ListItem {
                inlines: vec![],
                sub_items: vec![],
            }),
            Tag::Table(alignments) => {
                let aligns = alignments.iter().map(alignment_to_option).collect();
                self.stack.push(Frame::Table {
                    header: vec![],
                    rows: vec![],
                    alignments: aligns,
                    start_line: self.current_line,
                });
            }
            Tag::TableHead => self.stack.push(Frame::TableRow(vec![])),
            Tag::TableRow => self.stack.push(Frame::TableRow(vec![])),
            Tag::TableCell => self.stack.push(Frame::TableCell(vec![])),
            // Emph/Strong/Strikethrough 用临时 Paragraph frame 收集内部 inlines，
            // End 时转换为对应 inline。
            Tag::Emphasis | Tag::Strong | Tag::Strikethrough => {
                self.stack.push(Frame::Paragraph {
                    inlines: vec![],
                    start_line: self.current_line,
                });
            }
            // Superscript/Subscript 阶段 1 不保留语义，用临时 frame 收集后丢弃。
            Tag::Superscript | Tag::Subscript => {
                self.stack.push(Frame::Paragraph {
                    inlines: vec![],
                    start_line: self.current_line,
                });
            }
            Tag::Link {
                dest_url, title, ..
            } => {
                self.stack.push(Frame::Link {
                    inlines: vec![],
                    url: dest_url.into_string(),
                    title: cowstr_to_option(title),
                });
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                self.stack.push(Frame::Image {
                    inlines: vec![],
                    url: dest_url.into_string(),
                    title: cowstr_to_option(title),
                });
            }
            // 阶段 1 不支持的容器：不推 frame，对应 End 也会忽略。
            Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::MetadataBlock(_) => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                if let Some(Frame::Paragraph {
                    inlines,
                    start_line,
                }) = self.stack.pop()
                {
                    let p = Paragraph { inlines };
                    let span = Span {
                        start_line,
                        end_line: self.current_line,
                    };
                    // 列表项内的段落（loose list）扁平化为 item inlines。
                    match self.stack.last_mut() {
                        Some(Frame::ListItem {
                            inlines: item_inlines,
                            ..
                        }) => {
                            item_inlines.extend(p.inlines);
                        }
                        _ => self.push_block_with_span(Block::Paragraph(p), span),
                    }
                }
            }
            TagEnd::Heading(_) => {
                if let Some(Frame::Heading {
                    level,
                    inlines,
                    start_line,
                }) = self.stack.pop()
                {
                    let h = Heading { level, inlines };
                    let span = Span {
                        start_line,
                        end_line: self.current_line,
                    };
                    self.push_block_with_span(Block::Heading(h), span);
                }
            }
            TagEnd::CodeBlock => {
                if let Some(Frame::CodeBlock {
                    language,
                    content,
                    start_line,
                }) = self.stack.pop()
                {
                    let cb = CodeBlock { language, content };
                    let span = Span {
                        start_line,
                        end_line: self.current_line,
                    };
                    self.push_block_with_span(Block::CodeBlock(cb), span);
                }
            }
            TagEnd::HtmlBlock => {
                if let Some(Frame::HtmlBlock {
                    content,
                    start_line,
                }) = self.stack.pop()
                {
                    let span = Span {
                        start_line,
                        end_line: self.current_line,
                    };
                    self.push_block_with_span(Block::HtmlBlock(content), span);
                }
            }
            TagEnd::BlockQuote(_) => {
                if let Some(Frame::BlockQuote { blocks, start_line }) = self.stack.pop() {
                    let bq = BlockQuote { blocks };
                    let span = Span {
                        start_line,
                        end_line: self.current_line,
                    };
                    self.push_block_with_span(Block::BlockQuote(bq), span);
                }
            }
            TagEnd::List(_) => {
                if let Some(Frame::List {
                    ordered,
                    start,
                    items,
                    start_line,
                }) = self.stack.pop()
                {
                    // 嵌套列表（父为 ListItem）：items 直接并入父 item 的 sub_items
                    // （AST 设计上 ListItem 不容纳子 block，嵌套列表被扁平化）。
                    match self.stack.last_mut() {
                        Some(Frame::ListItem { sub_items, .. }) => sub_items.extend(items),
                        _ => {
                            let l = List {
                                ordered,
                                start,
                                items,
                            };
                            let span = Span {
                                start_line,
                                end_line: self.current_line,
                            };
                            self.push_block_with_span(Block::List(l), span);
                        }
                    }
                }
            }
            TagEnd::Item => {
                if let Some(Frame::ListItem { inlines, sub_items }) = self.stack.pop() {
                    let item = ListItem { inlines, sub_items };
                    match self.stack.last_mut() {
                        Some(Frame::List { items, .. }) => items.push(item),
                        Some(Frame::ListItem { sub_items, .. }) => sub_items.push(item),
                        _ => {}
                    }
                }
            }
            TagEnd::Table => {
                if let Some(Frame::Table {
                    header,
                    rows,
                    alignments,
                    start_line,
                }) = self.stack.pop()
                {
                    let t = Table {
                        header,
                        rows,
                        alignments,
                    };
                    let span = Span {
                        start_line,
                        end_line: self.current_line,
                    };
                    self.push_block_with_span(Block::Table(t), span);
                }
            }
            TagEnd::TableHead => {
                if let Some(Frame::TableRow(cells)) = self.stack.pop() {
                    if let Some(Frame::Table { header, .. }) = self.stack.last_mut() {
                        *header = cells;
                    }
                }
            }
            TagEnd::TableRow => {
                if let Some(Frame::TableRow(cells)) = self.stack.pop() {
                    if let Some(Frame::Table { rows, .. }) = self.stack.last_mut() {
                        rows.push(cells);
                    }
                }
            }
            TagEnd::TableCell => {
                if let Some(Frame::TableCell(inlines)) = self.stack.pop() {
                    let cell = TableCell { inlines };
                    if let Some(Frame::TableRow(cells)) = self.stack.last_mut() {
                        cells.push(cell);
                    }
                }
            }
            TagEnd::Emphasis => {
                if let Some(Frame::Paragraph { inlines, .. }) = self.stack.pop() {
                    self.push_inline(Inline::Emph(inlines));
                }
            }
            TagEnd::Strong => {
                if let Some(Frame::Paragraph { inlines, .. }) = self.stack.pop() {
                    self.push_inline(Inline::Strong(inlines));
                }
            }
            TagEnd::Strikethrough => {
                if let Some(Frame::Paragraph { inlines, .. }) = self.stack.pop() {
                    // Inline 不含 Strikethrough 变体，阶段 1 退化为 Text（保留文本，丢失语义）。
                    let s = inlines_into_text(inlines);
                    self.push_inline(Inline::Text(s));
                }
            }
            TagEnd::Link => {
                if let Some(Frame::Link {
                    inlines,
                    url,
                    title,
                }) = self.stack.pop()
                {
                    self.push_inline(Inline::Link {
                        text: inlines,
                        url,
                        title,
                    });
                }
            }
            TagEnd::Image => {
                if let Some(Frame::Image {
                    inlines,
                    url,
                    title,
                }) = self.stack.pop()
                {
                    let alt = inlines_into_text(inlines);
                    self.push_inline(Inline::Image { alt, url, title });
                }
            }
            TagEnd::Superscript | TagEnd::Subscript => {
                // 弹出临时 Paragraph frame，丢弃内容（阶段 1 不保留语义）。
                if matches!(self.stack.last(), Some(Frame::Paragraph { .. })) {
                    self.stack.pop();
                }
            }
            // 阶段 1 不支持的容器：Start 时未推 frame，End 时无需弹出。
            TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::MetadataBlock(_) => {}
        }
    }

    /// 将 block 推入最近的能容纳 block 的父 frame（Root 或 BlockQuote）。
    fn push_block_with_span(&mut self, block: Block, span: Span) {
        match self.stack.last_mut() {
            Some(Frame::Root(blocks)) | Some(Frame::BlockQuote { blocks, .. }) => {
                blocks.push(BlockWithSpan { block, span });
            }
            // TODO(阶段 2): ListItem AST 扩展后支持子 block。当前 list item 内的代码块/子段落/子引用会被丢弃。
            _ => {}
        }
    }

    /// 将 inline 推入最近的能容纳 inline 的父 frame。
    fn push_inline(&mut self, inline: Inline) {
        match self.stack.last_mut() {
            Some(Frame::Paragraph { inlines, .. })
            | Some(Frame::Heading { inlines, .. })
            | Some(Frame::TableCell(inlines))
            | Some(Frame::Link { inlines, .. })
            | Some(Frame::Image { inlines, .. }) => inlines.push(inline),
            Some(Frame::CodeBlock { content, .. }) => {
                // CodeBlock 内的 Text 事件是代码内容。
                if let Inline::Text(s) = inline {
                    content.push_str(&s);
                }
            }
            Some(Frame::HtmlBlock { content, .. }) => {
                if let Inline::Text(s) = inline {
                    content.push_str(&s);
                }
            }
            Some(Frame::ListItem { inlines, .. }) => inlines.push(inline),
            _ => {}
        }
    }

    /// HTML 事件：在 HtmlBlock frame 内累积为块内容；否则作为行内 HTML。
    fn push_html(&mut self, s: String) {
        match self.stack.last_mut() {
            Some(Frame::HtmlBlock { content, .. }) => content.push_str(&s),
            _ => self.push_inline(Inline::Html(s)),
        }
    }
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn alignment_to_option(a: &pulldown_cmark::Alignment) -> Option<Alignment> {
    match a {
        pulldown_cmark::Alignment::None => None,
        pulldown_cmark::Alignment::Left => Some(Alignment::Left),
        pulldown_cmark::Alignment::Center => Some(Alignment::Center),
        pulldown_cmark::Alignment::Right => Some(Alignment::Right),
    }
}

/// CowStr → Option<String>：空字符串视为 None。
fn cowstr_to_option(s: pulldown_cmark::CowStr) -> Option<String> {
    let s = s.into_string();
    if s.is_empty() { None } else { Some(s) }
}

/// 把 inlines 中所有 Text 拼接为单个 String（用于 Strikethrough/Image alt）。
fn inlines_into_text(inlines: Vec<Inline>) -> String {
    inlines
        .into_iter()
        .map(|i| match i {
            Inline::Text(t) => t,
            _ => String::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::ast::*;

    #[test]
    fn parse_empty_returns_empty_doc() {
        let doc = parse("").expect("空字符串应解析为空文档");
        assert!(doc.blocks.is_empty());
    }

    #[test]
    fn parse_heading_atx() {
        let doc = parse("# 标题").expect("解析标题失败");
        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::Heading(h),
                ..
            } => {
                assert_eq!(h.level, 1);
                assert_eq!(h.inlines, vec![Inline::Text("标题".into())]);
            }
            other => panic!("期望 Heading，实际 {other:?}"),
        }
    }

    #[test]
    fn parse_heading_level_3() {
        let doc = parse("### 三级").expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::Heading(h),
                ..
            } => assert_eq!(h.level, 3),
            _ => panic!("期望 Heading"),
        }
    }

    #[test]
    fn parse_paragraph() {
        let doc = parse("hello world").expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::Paragraph(p),
                ..
            } => {
                assert_eq!(p.inlines, vec![Inline::Text("hello world".into())]);
            }
            _ => panic!("期望 Paragraph"),
        }
    }

    #[test]
    fn parse_fenced_code_block_with_language() {
        let src = "```rust\nfn main() {}\n```\n";
        let doc = parse(src).expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::CodeBlock(cb),
                ..
            } => {
                assert_eq!(cb.language.as_deref(), Some("rust"));
                assert_eq!(cb.content, "fn main() {}\n");
            }
            _ => panic!("期望 CodeBlock"),
        }
    }

    #[test]
    fn parse_unordered_list() {
        let src = "- 一\n- 二\n- 三\n";
        let doc = parse(src).expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::List(l),
                ..
            } => {
                assert!(!l.ordered);
                assert_eq!(l.items.len(), 3);
                assert_eq!(l.items[0].inlines, vec![Inline::Text("一".into())]);
                assert!(l.items[0].sub_items.is_empty());
            }
            _ => panic!("期望 List"),
        }
    }

    #[test]
    fn parse_nested_list() {
        let src = "- 顶层\n  - 嵌套\n- 顶层2\n";
        let doc = parse(src).expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::List(l),
                ..
            } => {
                assert_eq!(l.items.len(), 2);
                assert_eq!(l.items[0].sub_items.len(), 1);
                assert_eq!(
                    l.items[0].sub_items[0].inlines,
                    vec![Inline::Text("嵌套".into())]
                );
            }
            _ => panic!("期望 List"),
        }
    }

    #[test]
    fn parse_blockquote() {
        let src = "> 引用文本\n";
        let doc = parse(src).expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::BlockQuote(bq),
                ..
            } => {
                assert_eq!(bq.blocks.len(), 1);
                match &bq.blocks[0] {
                    BlockWithSpan {
                        block: Block::Paragraph(p),
                        ..
                    } => {
                        assert_eq!(p.inlines, vec![Inline::Text("引用文本".into())]);
                    }
                    _ => panic!("引用内期望 Paragraph"),
                }
            }
            _ => panic!("期望 BlockQuote"),
        }
    }

    #[test]
    fn parse_thematic_break() {
        for src in ["---\n", "***\n", "___\n"] {
            let doc = parse(src).expect("解析失败");
            assert!(
                matches!(
                    &doc.blocks[0],
                    BlockWithSpan {
                        block: Block::ThematicBreak,
                        ..
                    }
                ),
                "期望 ThematicBreak，src={src}"
            );
        }
    }

    #[test]
    fn parse_emph_and_strong() {
        let src = "*emph* **strong**\n";
        let doc = parse(src).expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::Paragraph(p),
                ..
            } => {
                assert!(
                    p.inlines
                        .contains(&Inline::Emph(vec![Inline::Text("emph".into())]))
                );
                assert!(
                    p.inlines
                        .contains(&Inline::Strong(vec![Inline::Text("strong".into())]))
                );
            }
            _ => panic!("期望 Paragraph"),
        }
    }

    #[test]
    fn parse_inline_code() {
        let doc = parse("`code`").expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::Paragraph(p),
                ..
            } => {
                assert!(p.inlines.contains(&Inline::Code("code".into())));
            }
            _ => panic!("期望 Paragraph"),
        }
    }

    #[test]
    fn parse_link() {
        let src = "[text](https://example.com)\n";
        let doc = parse(src).expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::Paragraph(p),
                ..
            } => {
                let found = p.inlines.iter().any(|i| {
                    matches!(
                        i,
                        Inline::Link { url, .. } if url == "https://example.com"
                    )
                });
                assert!(found, "未找到 Link: {:?}", p.inlines);
            }
            _ => panic!("期望 Paragraph"),
        }
    }

    #[test]
    fn parse_image() {
        let src = "![alt](https://example.com/x.png)\n";
        let doc = parse(src).expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::Paragraph(p),
                ..
            } => {
                let found = p.inlines.iter().any(|i| {
                    matches!(
                        i,
                        Inline::Image { url, .. } if url == "https://example.com/x.png"
                    )
                });
                assert!(found, "未找到 Image: {:?}", p.inlines);
            }
            _ => panic!("期望 Paragraph"),
        }
    }

    #[test]
    fn parse_table() {
        let src = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let doc = parse(src).expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::Table(t),
                ..
            } => {
                assert_eq!(t.header.len(), 2);
                assert_eq!(t.rows.len(), 1);
                assert_eq!(t.rows[0].len(), 2);
            }
            _ => panic!("期望 Table"),
        }
    }

    #[test]
    fn parse_soft_break() {
        let src = "line1\nline2\n";
        let doc = parse(src).expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::Paragraph(p),
                ..
            } => {
                assert!(
                    p.inlines.contains(&Inline::SoftBreak),
                    "未找到 SoftBreak: {:?}",
                    p.inlines
                );
            }
            _ => panic!("期望 Paragraph"),
        }
    }

    #[test]
    #[ignore = "AST 限制：ListItem 不容纳子 block，阶段 2 扩展后启用"]
    fn parse_list_item_with_code_block() {
        let src = "- 项\n  ```rust\n  fn x() {}\n  ```\n";
        let doc = parse(src).expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::List(l),
                ..
            } => {
                // 期望：ListItem 含子 block，但当前 AST 不支持
                // 占位测试，AST 扩展后补充断言
                assert_eq!(l.items.len(), 1);
            }
            _ => panic!("期望 List"),
        }
    }

    #[test]
    #[ignore = "AST 限制：Loose list 多段落合并，阶段 2 扩展后启用"]
    fn parse_loose_list_multiple_paragraphs() {
        let src = "- 一段\n\n  二段\n";
        let doc = parse(src).expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::List(l),
                ..
            } => {
                assert_eq!(l.items.len(), 1);
                // 期望：item 含两个段落，但当前合并为单一 inlines
                // 占位测试，AST 扩展后补充断言
            }
            _ => panic!("期望 List"),
        }
    }

    #[test]
    #[ignore = "AST 限制：嵌套列表类型信息丢失，阶段 2 扩展后启用"]
    fn parse_mixed_nested_list() {
        let src = "1. 顶层\n   - 嵌套\n";
        let doc = parse(src).expect("解析失败");
        match &doc.blocks[0] {
            BlockWithSpan {
                block: Block::List(l),
                ..
            } => {
                assert!(l.ordered);
                assert_eq!(l.items.len(), 1);
                // 期望：sub_items 保留内层 ordered=false 信息，但当前丢失
                // 占位测试，AST 扩展后补充断言
            }
            _ => panic!("期望 List"),
        }
    }
}
