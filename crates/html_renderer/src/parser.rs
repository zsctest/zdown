//! HTML → HtmlNode 树：使用 html5ever 解析 HTML 片段。

use std::borrow::Cow;
use std::collections::HashMap;

use html5ever::interface::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{Attribute, ExpandedName, QualName, parse_document};

use crate::css::{self, CssStyle};

// ---- 标签枚举 ----

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum InlineTag {
    B,
    I,
    U,
    Code,
    Del,
    Mark,
    Sub,
    Sup,
    A,
    Span,
    Br,
    Small,
    Big,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum BlockTag {
    Div,
    P,
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
    Pre,
    Hr,
    Table,
    Blockquote,
    Ul,
    Ol,
    Li,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TagKind {
    Inline(InlineTag),
    Block(BlockTag),
    Unknown,
}

// ---- HtmlNode ----

/// DOM 节点。
#[derive(Debug, Clone)]
pub(crate) enum HtmlNode {
    Inline {
        tag: InlineTag,
        attrs: HashMap<String, String>,
        style: CssStyle,
        children: Vec<HtmlNode>,
    },
    Block {
        tag: BlockTag,
        attrs: HashMap<String, String>,
        style: CssStyle,
        children: Vec<HtmlNode>,
    },
    Text(String),
}

// ---- DOM node types for TreeSink ----

type Handle = usize;

#[derive(Debug)]
enum DomNodeData {
    Document,
    Doctype,
    Text {
        contents: StrTendril,
    },
    Comment,
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
    },
    ProcessingInstruction,
}

#[derive(Debug)]
struct DomNode {
    parent: Option<Handle>,
    children: Vec<Handle>,
    data: DomNodeData,
}

/// The tree sink that accumulates parsed DOM into flat nodes.
struct Sink {
    nodes: Vec<DomNode>,
    document: Handle,
    quirks_mode: QuirksMode,
}

impl Sink {
    fn new() -> Self {
        let doc = DomNode {
            parent: None,
            children: vec![],
            data: DomNodeData::Document,
        };
        Sink {
            nodes: vec![doc],
            document: 0,
            quirks_mode: QuirksMode::NoQuirks,
        }
    }

    fn alloc_node(&mut self, data: DomNodeData) -> Handle {
        let id = self.nodes.len();
        self.nodes.push(DomNode {
            parent: None,
            children: vec![],
            data,
        });
        id
    }

    fn append_common(
        &mut self,
        child: NodeOrText<Handle>,
        parent: Handle,
        previous: Option<Handle>,
    ) {
        match child {
            NodeOrText::AppendText(text) => {
                // Merge with previous sibling if it's also text
                if let Some(prev_id) = previous {
                    if let DomNodeData::Text { contents } = &mut self.nodes[prev_id].data {
                        contents.push_tendril(&text);
                        return;
                    }
                }
                let id = self.alloc_node(DomNodeData::Text { contents: text });
                self.append_child(parent, id);
            }
            NodeOrText::AppendNode(id) => {
                // Detach from old parent
                if let Some(old_parent) = self.nodes[id].parent {
                    self.nodes[old_parent].children.retain(|c| *c != id);
                }
                self.nodes[id].parent = Some(parent);
                self.append_child(parent, id);
            }
        }
    }

    fn append_child(&mut self, parent: Handle, child: Handle) {
        self.nodes[parent].children.push(child);
        self.nodes[child].parent = Some(parent);
    }

    fn insert_before(&mut self, sibling: Handle, new_node: Handle) {
        let parent = self.nodes[sibling].parent;
        // Detach from old parent
        if let Some(old_parent) = self.nodes[new_node].parent {
            self.nodes[old_parent].children.retain(|c| *c != new_node);
        }
        self.nodes[new_node].parent = parent;

        if let Some(p) = parent {
            let children = &mut self.nodes[p].children;
            if let Some(pos) = children.iter().position(|c| *c == sibling) {
                children.insert(pos, new_node);
            } else {
                children.push(new_node);
            }
        }
    }

    fn find_body(&self) -> Option<Handle> {
        // Walk from document to find <html> → <body>
        for &html_id in &self.nodes[self.document].children {
            if let DomNodeData::Element { ref name, .. } = self.nodes[html_id].data {
                if name.local.as_ref() == "html" {
                    for &body_id in &self.nodes[html_id].children {
                        if let DomNodeData::Element { ref name, .. } = self.nodes[body_id].data {
                            if name.local.as_ref() == "body" {
                                return Some(body_id);
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

impl TreeSink for Sink {
    type Handle = Handle;
    type Output = Self;

    fn finish(self) -> Self {
        self
    }

    fn parse_error(&mut self, _msg: Cow<'static, str>) {
        // Silently ignore parse errors for graceful degradation
    }

    fn get_document(&mut self) -> Handle {
        self.document
    }

    fn set_quirks_mode(&mut self, mode: QuirksMode) {
        self.quirks_mode = mode;
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        x == y
    }

    fn elem_name<'a>(&'a self, target: &'a Handle) -> ExpandedName<'a> {
        match &self.nodes[*target].data {
            DomNodeData::Element { name, .. } => name.expanded(),
            _ => panic!("not an element"),
        }
    }

    fn get_template_contents(&mut self, _target: &Handle) -> Handle {
        // Templates not supported; return a dummy
        self.alloc_node(DomNodeData::Document)
    }

    fn is_mathml_annotation_xml_integration_point(&self, _target: &Handle) -> bool {
        false
    }

    fn create_element(
        &mut self,
        name: QualName,
        attrs: Vec<Attribute>,
        _flags: ElementFlags,
    ) -> Handle {
        self.alloc_node(DomNodeData::Element { name, attrs })
    }

    fn create_comment(&mut self, _text: StrTendril) -> Handle {
        self.alloc_node(DomNodeData::Comment)
    }

    fn create_pi(&mut self, _target: StrTendril, _data: StrTendril) -> Handle {
        self.alloc_node(DomNodeData::ProcessingInstruction)
    }

    fn append(&mut self, parent: &Handle, child: NodeOrText<Handle>) {
        let parent = *parent;
        let prev = self.nodes[parent].children.last().copied();
        self.append_common(child, parent, prev);
    }

    fn append_before_sibling(&mut self, sibling: &Handle, child: NodeOrText<Handle>) {
        let sibling = *sibling;
        // Find previous sibling
        let parent = self.nodes[sibling].parent;
        let prev = parent.and_then(|p| {
            let children = &self.nodes[p].children;
            children.iter().position(|c| *c == sibling).and_then(|pos| {
                if pos > 0 {
                    Some(children[pos - 1])
                } else {
                    None
                }
            })
        });
        match child {
            NodeOrText::AppendText(text) => {
                if let Some(prev_id) = prev {
                    if let DomNodeData::Text { contents } = &mut self.nodes[prev_id].data {
                        contents.push_tendril(&text);
                        return;
                    }
                }
                let id = self.alloc_node(DomNodeData::Text { contents: text });
                self.insert_before(sibling, id);
            }
            NodeOrText::AppendNode(id) => {
                self.insert_before(sibling, id);
            }
        }
    }

    fn append_based_on_parent_node(
        &mut self,
        element: &Handle,
        prev_element: &Handle,
        child: NodeOrText<Handle>,
    ) {
        let element = *element;
        let prev_element = *prev_element;
        if self.nodes[element].parent.is_some() {
            self.append_before_sibling(&element, child)
        } else {
            self.append(&prev_element, child)
        }
    }

    fn append_doctype_to_document(
        &mut self,
        _name: StrTendril,
        _public_id: StrTendril,
        _system_id: StrTendril,
    ) {
        let id = self.alloc_node(DomNodeData::Doctype);
        self.append_child(self.document, id);
    }

    fn add_attrs_if_missing(&mut self, target: &Handle, attrs: Vec<Attribute>) {
        let target = *target;
        if let DomNodeData::Element {
            attrs: existing, ..
        } = &mut self.nodes[target].data
        {
            let existing_names: Vec<_> = existing.iter().map(|a| a.name.clone()).collect();
            for attr in attrs {
                if !existing_names.contains(&attr.name) {
                    existing.push(attr);
                }
            }
        }
    }

    fn remove_from_parent(&mut self, target: &Handle) {
        let target = *target;
        if let Some(parent) = self.nodes[target].parent {
            self.nodes[parent].children.retain(|c| *c != target);
            self.nodes[target].parent = None;
        }
    }

    fn reparent_children(&mut self, node: &Handle, new_parent: &Handle) {
        let node = *node;
        let new_parent = *new_parent;
        let children: Vec<Handle> = self.nodes[node].children.drain(..).collect();
        for child in children {
            self.append_child(new_parent, child);
        }
    }
}

// ---- 标签分类 ----

fn classify(tag: &str) -> TagKind {
    match tag.to_lowercase().as_str() {
        "b" | "strong" => TagKind::Inline(InlineTag::B),
        "i" | "em" => TagKind::Inline(InlineTag::I),
        "u" | "ins" => TagKind::Inline(InlineTag::U),
        "code" => TagKind::Inline(InlineTag::Code),
        "del" | "s" | "strike" => TagKind::Inline(InlineTag::Del),
        "mark" => TagKind::Inline(InlineTag::Mark),
        "sub" => TagKind::Inline(InlineTag::Sub),
        "sup" => TagKind::Inline(InlineTag::Sup),
        "a" => TagKind::Inline(InlineTag::A),
        "span" | "font" => TagKind::Inline(InlineTag::Span),
        "br" => TagKind::Inline(InlineTag::Br),
        "small" => TagKind::Inline(InlineTag::Small),
        "big" => TagKind::Inline(InlineTag::Big),

        "div" => TagKind::Block(BlockTag::Div),
        "p" => TagKind::Block(BlockTag::P),
        "h1" => TagKind::Block(BlockTag::H1),
        "h2" => TagKind::Block(BlockTag::H2),
        "h3" => TagKind::Block(BlockTag::H3),
        "h4" => TagKind::Block(BlockTag::H4),
        "h5" => TagKind::Block(BlockTag::H5),
        "h6" => TagKind::Block(BlockTag::H6),
        "pre" => TagKind::Block(BlockTag::Pre),
        "hr" => TagKind::Block(BlockTag::Hr),
        "table" => TagKind::Block(BlockTag::Table),
        "blockquote" => TagKind::Block(BlockTag::Blockquote),
        "ul" => TagKind::Block(BlockTag::Ul),
        "ol" => TagKind::Block(BlockTag::Ol),
        "li" => TagKind::Block(BlockTag::Li),

        "html" | "head" | "body" | "meta" | "title" | "link" | "script" | "style" | "noscript" => {
            TagKind::Unknown
        }
        _ => TagKind::Unknown,
    }
}

// ---- 入口 ----

/// 解析 HTML 片段为 HtmlNode 列表（内联上下文）。
pub(crate) fn parse_inline(html: &str) -> Vec<HtmlNode> {
    let wrapped = format!("<html><body>{html}</body></html>");
    parse_fragment(&wrapped)
}

/// 解析 HTML 片段为 HtmlNode 列表（块级上下文）。
pub(crate) fn parse_block(html: &str) -> Vec<HtmlNode> {
    let wrapped = format!("<html><body>{html}</body></html>");
    parse_fragment(&wrapped)
}

fn parse_fragment(html: &str) -> Vec<HtmlNode> {
    let sink = Sink::new();
    let result = parse_document(sink, Default::default())
        .from_utf8()
        .one(html.as_bytes());

    let body = match result.find_body() {
        Some(b) => b,
        None => return vec![HtmlNode::Text(html.to_string())],
    };

    convert_children(&result, body)
}

fn convert_children(sink: &Sink, parent: Handle) -> Vec<HtmlNode> {
    let mut out = vec![];
    for &child_id in &sink.nodes[parent].children {
        if let Some(node) = convert_node(sink, child_id) {
            out.push(node);
        }
    }
    out
}

fn convert_node(sink: &Sink, handle: Handle) -> Option<HtmlNode> {
    match &sink.nodes[handle].data {
        DomNodeData::Text { contents } => {
            let text = contents.to_string();
            if text.trim().is_empty() {
                None
            } else {
                Some(HtmlNode::Text(text))
            }
        }
        DomNodeData::Element { name, attrs } => {
            let tag_name = name.local.as_ref();
            let kind = classify(tag_name);
            let attr_map = attrs_to_map(attrs);

            let style_str = attr_map.get("style").cloned().unwrap_or_default();
            let style = css::parse_style(&style_str);

            match kind {
                TagKind::Inline(tag) => {
                    let children = convert_children(sink, handle);
                    Some(HtmlNode::Inline {
                        tag,
                        attrs: attr_map,
                        style,
                        children,
                    })
                }
                TagKind::Block(tag) => {
                    let children = convert_children(sink, handle);
                    Some(HtmlNode::Block {
                        tag,
                        attrs: attr_map,
                        style,
                        children,
                    })
                }
                TagKind::Unknown => {
                    // 未知标签：透传子节点
                    let children = convert_children(sink, handle);
                    if children.is_empty() {
                        None
                    } else {
                        // Flatten children into output
                        return Some(flatten_or_first(children));
                    }
                }
            }
        }
        // Skip Document, Doctype, Comment, ProcessingInstruction
        _ => {
            let children = convert_children(sink, handle);
            if children.is_empty() {
                None
            } else {
                Some(flatten_or_first(children))
            }
        }
    }
}

/// Return a single node or wrap multiple in a text-based representation.
fn flatten_or_first(mut children: Vec<HtmlNode>) -> HtmlNode {
    if children.len() == 1 {
        children.remove(0)
    } else {
        // Join text content from children
        let text: String = children
            .iter()
            .filter_map(|n| match n {
                HtmlNode::Text(s) => Some(s.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");
        if text.is_empty() {
            // If no text content, return first child
            children
                .into_iter()
                .next()
                .unwrap_or(HtmlNode::Text(String::new()))
        } else {
            HtmlNode::Text(text)
        }
    }
}

fn attrs_to_map(attrs: &[Attribute]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for attr in attrs {
        map.insert(attr.name.local.to_string(), attr.value.to_string());
    }
    map
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn classify_inline_tags() {
        assert_eq!(classify("b"), TagKind::Inline(InlineTag::B));
        assert_eq!(classify("strong"), TagKind::Inline(InlineTag::B));
        assert_eq!(classify("i"), TagKind::Inline(InlineTag::I));
        assert_eq!(classify("em"), TagKind::Inline(InlineTag::I));
        assert_eq!(classify("u"), TagKind::Inline(InlineTag::U));
        assert_eq!(classify("code"), TagKind::Inline(InlineTag::Code));
        assert_eq!(classify("del"), TagKind::Inline(InlineTag::Del));
        assert_eq!(classify("mark"), TagKind::Inline(InlineTag::Mark));
        assert_eq!(classify("sub"), TagKind::Inline(InlineTag::Sub));
        assert_eq!(classify("sup"), TagKind::Inline(InlineTag::Sup));
        assert_eq!(classify("a"), TagKind::Inline(InlineTag::A));
        assert_eq!(classify("span"), TagKind::Inline(InlineTag::Span));
        assert_eq!(classify("br"), TagKind::Inline(InlineTag::Br));
    }

    #[test]
    fn classify_block_tags() {
        assert_eq!(classify("div"), TagKind::Block(BlockTag::Div));
        assert_eq!(classify("p"), TagKind::Block(BlockTag::P));
        assert_eq!(classify("h1"), TagKind::Block(BlockTag::H1));
        assert_eq!(classify("h6"), TagKind::Block(BlockTag::H6));
        assert_eq!(classify("pre"), TagKind::Block(BlockTag::Pre));
        assert_eq!(classify("hr"), TagKind::Block(BlockTag::Hr));
        assert_eq!(classify("table"), TagKind::Block(BlockTag::Table));
        assert_eq!(classify("blockquote"), TagKind::Block(BlockTag::Blockquote));
        assert_eq!(classify("ul"), TagKind::Block(BlockTag::Ul));
        assert_eq!(classify("ol"), TagKind::Block(BlockTag::Ol));
        assert_eq!(classify("li"), TagKind::Block(BlockTag::Li));
    }

    #[test]
    fn classify_case_insensitive() {
        assert_eq!(classify("DIV"), TagKind::Block(BlockTag::Div));
        assert_eq!(classify("Strong"), TagKind::Inline(InlineTag::B));
    }

    #[test]
    fn classify_unknown() {
        assert_eq!(classify("custom-element"), TagKind::Unknown);
        assert_eq!(classify("script"), TagKind::Unknown);
        assert_eq!(classify("style"), TagKind::Unknown);
    }

    #[test]
    fn parse_simple_text() {
        let nodes = parse_inline("hello");
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            HtmlNode::Text(s) => assert_eq!(s, "hello"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn parse_bold() {
        let nodes = parse_inline("<b>bold</b>");
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            HtmlNode::Inline { tag, children, .. } => {
                assert_eq!(*tag, InlineTag::B);
                match &children[0] {
                    HtmlNode::Text(s) => assert_eq!(s, "bold"),
                    _ => panic!("expected Text"),
                }
            }
            _ => panic!("expected Inline"),
        }
    }

    #[test]
    fn parse_nested() {
        let nodes = parse_inline("<b><i>bold italic</i></b>");
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            HtmlNode::Inline { tag, children, .. } => {
                assert_eq!(*tag, InlineTag::B);
                match &children[0] {
                    HtmlNode::Inline { tag, children, .. } => {
                        assert_eq!(*tag, InlineTag::I);
                        match &children[0] {
                            HtmlNode::Text(s) => assert_eq!(s, "bold italic"),
                            _ => panic!("expected Text"),
                        }
                    }
                    _ => panic!("expected Inline"),
                }
            }
            _ => panic!("expected Inline"),
        }
    }

    #[test]
    fn parse_block_div() {
        let nodes = parse_block("<div>content</div>");
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            HtmlNode::Block { tag, .. } => assert_eq!(*tag, BlockTag::Div),
            _ => panic!("expected Block"),
        }
    }

    #[test]
    fn parse_style_attribute() {
        let nodes = parse_inline("<span style=\"color: red\">text</span>");
        match &nodes[0] {
            HtmlNode::Inline { tag, style, .. } => {
                assert_eq!(*tag, InlineTag::Span);
                assert_eq!(style.color, Some(egui::Color32::RED));
            }
            _ => panic!("expected Inline"),
        }
    }

    #[test]
    fn parse_unknown_tag_passthrough() {
        let nodes = parse_inline("<custom>text</custom>");
        // custom tag is unknown, children are passed through
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            HtmlNode::Text(s) => assert_eq!(s, "text"),
            _ => panic!("expected Text from passthrough"),
        }
    }
}
