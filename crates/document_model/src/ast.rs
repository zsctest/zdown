//! Markdown AST 节点类型。
//!
//! 分 Block / Inline 两层。节点用 struct，便于后续扩展字段。

use serde::{Deserialize, Serialize};

/// 源码 span（行范围，0-based，含两端）。
/// 用于 hybrid 模式按 block 边界分割。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start_line: usize,
    pub end_line: usize,
}

/// 带 span 的 Block。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockWithSpan {
    pub block: Block,
    pub span: Span,
}

/// 文档根类型。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// 顶层块级节点（带 span），按文档顺序排列。
    pub blocks: Vec<BlockWithSpan>,
}

/// 块级节点。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Block {
    Heading(Heading),
    Paragraph(Paragraph),
    CodeBlock(CodeBlock),
    List(List),
    BlockQuote(BlockQuote),
    ThematicBreak,
    Table(Table),
    HtmlBlock(String),
}

/// 标题（级别 1-6）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Heading {
    pub level: u8,
    pub inlines: Vec<Inline>,
}

/// 段落。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    pub inlines: Vec<Inline>,
}

/// 代码块（fenced）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeBlock {
    /// 语言标识（fenced 起始标注的语言），无则为 `None`。
    pub language: Option<String>,
    /// 原始代码内容（不含 fence）。
    pub content: String,
}

/// 列表。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List {
    /// `true` = 有序列表，`false` = 无序列表。
    pub ordered: bool,
    /// 有序列表起始序号（无序列表忽略此字段）。
    pub start: usize,
    /// 列表项，按文档顺序。
    pub items: Vec<ListItem>,
}

/// 列表项。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListItem {
    /// 本项的行内内容。
    pub inlines: Vec<Inline>,
    /// 嵌套子列表项（缩进更深的项）。
    pub sub_items: Vec<ListItem>,
}

/// 引用块。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockQuote {
    /// 引用内的块级节点（带 span）。
    pub blocks: Vec<BlockWithSpan>,
}

/// 表格。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub header: Vec<TableCell>,
    pub rows: Vec<Vec<TableCell>>,
    /// 每列对齐方式，长度与列数一致。
    pub alignments: Vec<Option<Alignment>>,
}

/// 表格列对齐。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Alignment {
    Left,
    Center,
    Right,
}

/// 表格单元格。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    pub inlines: Vec<Inline>,
}

/// 行内节点。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Inline {
    /// 普通文本。
    Text(String),
    /// *强调*。
    Emph(Vec<Inline>),
    /// **粗体**。
    Strong(Vec<Inline>),
    /// `行内代码`。
    Code(String),
    /// 链接。
    Link {
        text: Vec<Inline>,
        url: String,
        title: Option<String>,
    },
    /// 图片。
    Image {
        alt: String,
        url: String,
        title: Option<String>,
    },
    /// 行内 HTML。
    Html(String),
    /// 软换行（源码换行，渲染时按空格或换行）。
    SoftBreak,
    /// 硬换行（反斜杠结尾或两空格 + 换行）。
    HardBreak,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn document_default_is_empty() {
        let doc = Document { blocks: vec![] };
        assert!(doc.blocks.is_empty());
    }

    #[test]
    fn span_serializes_to_json() {
        let span = Span {
            start_line: 0,
            end_line: 1,
        };
        let json = serde_json::to_string(&span).expect("serialize Span");
        assert!(json.contains("\"start_line\":0"));
        assert!(json.contains("\"end_line\":1"));
    }

    #[test]
    fn heading_serializes_to_json() {
        let h = Heading {
            level: 2,
            inlines: vec![Inline::Text("标题".into())],
        };
        let json =
            serde_json::to_string(&h).expect("serde_json 不在 dev-dependencies，本测试会编译失败");
        assert!(json.contains("\"level\":2"));
    }
}
