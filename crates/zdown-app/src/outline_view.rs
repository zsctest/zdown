//! 大纲/目录面板：从 Document AST 提取标题，侧边栏渲染。

use std::collections::BTreeSet;

use document_model::ast::{Block, Document, Inline};
use editor_engine::Cursor;
use eframe::egui;

use crate::editor_state::EditorState;

/// 大纲项：标题层级、纯文本内容、源码行号。
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineItem {
    pub level: u8,
    pub text: String,
    pub line: usize,
}

/// 大纲面板的折叠状态。
///
/// 维护被折叠标题的索引集合，以及用于检测文档结构变化的结构指纹。
/// 当文档结构变化时，折叠集合自动清除以避免索引失效。
#[derive(Debug, Clone, Default)]
pub struct OutlineFoldState {
    /// 被折叠的标题索引集合。
    pub collapsed: BTreeSet<usize>,
    /// 结构指纹：由标题数量和层级序列计算。
    /// 变化时自动清除 collapsed。
    fingerprint: u64,
}

/// 将行内节点转换为纯文本（去除内联标记）。
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

/// 从 Document AST 提取所有标题项。
pub fn extract_outline(doc: &Document) -> Vec<OutlineItem> {
    doc.blocks
        .iter()
        .filter_map(|bws| match &bws.block {
            Block::Heading(h) => {
                let text = inlines_to_plain(&h.inlines);
                let text = if text.is_empty() {
                    "(空标题)".to_string()
                } else {
                    text
                };
                Some(OutlineItem {
                    level: h.level,
                    text,
                    line: bws.span.start_line,
                })
            }
            _ => None,
        })
        .collect()
}

/// 查找光标所在 section 的标题索引。
/// 返回最后一项 `line <= cursor_line` 的标题索引；若光标在所有标题之前则返回 None。
fn current_heading_index(items: &[OutlineItem], cursor_line: usize) -> Option<usize> {
    items
        .iter()
        .enumerate()
        .rev()
        .find(|(_, item)| item.line <= cursor_line)
        .map(|(i, _)| i)
}

/// 计算大纲结构指纹。
///
/// 基于标题数量和层级序列（不含行号和文本），
/// 因此仅编辑文本或插入空行不会重置折叠状态。
fn compute_outline_fingerprint(items: &[OutlineItem]) -> u64 {
    use std::hash::{DefaultHasher, Hasher};
    let mut hasher = DefaultHasher::new();
    hasher.write_usize(items.len());
    for item in items {
        hasher.write_u8(item.level);
    }
    hasher.finish()
}

/// 判断指定索引的标题是否有子标题。
fn has_children(items: &[OutlineItem], idx: usize) -> bool {
    items
        .get(idx + 1)
        .is_some_and(|next| next.level > items[idx].level)
}

/// 计算大纲项的可见性。
///
/// 返回与 `items` 平行的 `Vec<bool>`，`true` 表示可见。
/// 使用栈追踪被折叠的祖先项：若某一项有被折叠的祖先，则隐藏该项。
pub fn compute_visibility(items: &[OutlineItem], collapsed: &BTreeSet<usize>) -> Vec<bool> {
    let n = items.len();
    let mut visible = vec![true; n];
    let mut collapse_stack: Vec<usize> = Vec::new();

    for i in 0..n {
        // 弹出作用域已结束的折叠祖先
        while let Some(&parent) = collapse_stack.last() {
            if items[i].level <= items[parent].level {
                collapse_stack.pop();
            } else {
                break;
            }
        }

        // 若存在被折叠的祖先，则隐藏该项
        if !collapse_stack.is_empty() {
            visible[i] = false;
        }

        // 若该项自身被折叠，则压入栈
        if collapsed.contains(&i) {
            collapse_stack.push(i);
        }
    }

    visible
}

pub fn show_outline_panel(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    fold_state: &mut OutlineFoldState,
) {
    ui.heading("📑 大纲");

    let doc = state.current_doc();
    let items = extract_outline(&doc);

    // 检测文档结构变化，若指纹不匹配则重置折叠状态
    let fp = compute_outline_fingerprint(&items);
    if fp != fold_state.fingerprint {
        fold_state.collapsed.clear();
        fold_state.fingerprint = fp;
    }
    // 清理越界索引
    fold_state.collapsed.retain(|&i| i < items.len());

    if items.is_empty() {
        ui.label(egui::RichText::new("（无标题）").weak());
        return;
    }

    // 计算可见性和当前标题索引
    let visible = compute_visibility(&items, &fold_state.collapsed);
    let cursor_line = state.editor.cursor.line;
    let current_idx = current_heading_index(&items, cursor_line);

    egui::ScrollArea::vertical()
        .id_salt("outline_scroll")
        .show(ui, |ui| {
            for (i, item) in items.iter().enumerate() {
                if !visible[i] {
                    continue;
                }

                let is_current = current_idx == Some(i);
                let indent = (item.level.saturating_sub(1) as f32) * 16.0;
                let can_fold = has_children(&items, i);
                let is_collapsed = can_fold && fold_state.collapsed.contains(&i);

                ui.horizontal(|ui| {
                    ui.add_space(indent);

                    // 折叠/展开切换图标
                    if can_fold {
                        let toggle_char = if is_collapsed { "▶" } else { "▼" };
                        let toggle = ui.add(
                            egui::Label::new(egui::RichText::new(toggle_char).size(11.0).weak())
                                .sense(egui::Sense::click()),
                        );
                        if toggle.clicked() {
                            if is_collapsed {
                                fold_state.collapsed.remove(&i);
                            } else {
                                fold_state.collapsed.insert(i);
                            }
                        }
                    } else {
                        // 无子项的标题，保留对齐间距
                        ui.add_space(14.0);
                    }

                    let text = if item.level <= 2 {
                        egui::RichText::new(&item.text).size(14.0).strong()
                    } else {
                        egui::RichText::new(&item.text).size(13.0).weak()
                    };

                    let response = ui.selectable_label(is_current, text);
                    if response.clicked() {
                        let cursor = Cursor::new(item.line, 0);
                        let _ = state.editor.set_cursor(cursor);
                    }
                });
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use document_model::ast::{BlockWithSpan, Heading, Span};

    fn doc_from_blocks(blocks: Vec<BlockWithSpan>) -> Document {
        Document { blocks }
    }

    fn bws(block: Block, start_line: usize, end_line: usize) -> BlockWithSpan {
        BlockWithSpan {
            block,
            span: Span {
                start_line,
                end_line,
            },
        }
    }

    #[test]
    fn extract_outline_empty_doc() {
        let doc = doc_from_blocks(vec![]);
        assert_eq!(extract_outline(&doc), vec![]);
    }

    #[test]
    fn extract_outline_single_h1() {
        let doc = doc_from_blocks(vec![bws(
            Block::Heading(Heading {
                level: 1,
                inlines: vec![Inline::Text("简介".into())],
            }),
            0,
            0,
        )]);
        let items = extract_outline(&doc);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].level, 1);
        assert_eq!(items[0].text, "简介");
        assert_eq!(items[0].line, 0);
    }

    #[test]
    fn extract_outline_multiple_levels() {
        let doc = doc_from_blocks(vec![
            bws(
                Block::Heading(Heading {
                    level: 1,
                    inlines: vec![Inline::Text("第一章".into())],
                }),
                0,
                0,
            ),
            bws(
                Block::Heading(Heading {
                    level: 2,
                    inlines: vec![Inline::Text("第一节".into())],
                }),
                2,
                2,
            ),
            bws(
                Block::Heading(Heading {
                    level: 3,
                    inlines: vec![Inline::Text("小节".into())],
                }),
                5,
                5,
            ),
        ]);
        let items = extract_outline(&doc);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].level, 1);
        assert_eq!(items[1].level, 2);
        assert_eq!(items[2].level, 3);
        assert_eq!(items[2].line, 5);
    }

    #[test]
    fn extract_outline_no_headings() {
        let doc = doc_from_blocks(vec![bws(
            Block::Paragraph(document_model::ast::Paragraph {
                inlines: vec![Inline::Text("hello".into())],
            }),
            0,
            0,
        )]);
        assert_eq!(extract_outline(&doc), vec![]);
    }

    #[test]
    fn extract_outline_text_is_plain() {
        let doc = doc_from_blocks(vec![bws(
            Block::Heading(Heading {
                level: 1,
                inlines: vec![
                    Inline::Strong(vec![Inline::Text("重要".into())]),
                    Inline::Text("：".into()),
                    Inline::Link {
                        text: vec![Inline::Text("参考".into())],
                        url: "http://x".into(),
                        title: None,
                    },
                ],
            }),
            0,
            0,
        )]);
        let items = extract_outline(&doc);
        assert_eq!(items[0].text, "重要：参考");
    }

    #[test]
    fn extract_outline_empty_heading_text() {
        let doc = doc_from_blocks(vec![bws(
            Block::Heading(Heading {
                level: 2,
                inlines: vec![],
            }),
            3,
            3,
        )]);
        let items = extract_outline(&doc);
        assert_eq!(items[0].text, "(空标题)");
    }

    #[test]
    fn extract_outline_skips_non_headings() {
        let doc = doc_from_blocks(vec![
            bws(
                Block::Paragraph(document_model::ast::Paragraph {
                    inlines: vec![Inline::Text("text".into())],
                }),
                0,
                0,
            ),
            bws(
                Block::Heading(Heading {
                    level: 1,
                    inlines: vec![Inline::Text("标题".into())],
                }),
                1,
                1,
            ),
            bws(Block::ThematicBreak, 2, 2),
            bws(
                Block::Heading(Heading {
                    level: 2,
                    inlines: vec![Inline::Text("副标题".into())],
                }),
                3,
                3,
            ),
        ]);
        let items = extract_outline(&doc);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text, "标题");
        assert_eq!(items[1].text, "副标题");
    }

    // ---- current_heading_index ----

    #[test]
    fn current_heading_cursor_at_first() {
        let items = vec![
            OutlineItem {
                level: 1,
                text: "A".into(),
                line: 0,
            },
            OutlineItem {
                level: 1,
                text: "B".into(),
                line: 5,
            },
            OutlineItem {
                level: 2,
                text: "C".into(),
                line: 8,
            },
        ];
        assert_eq!(current_heading_index(&items, 0), Some(0));
        assert_eq!(current_heading_index(&items, 4), Some(0));
        assert_eq!(current_heading_index(&items, 5), Some(1));
        assert_eq!(current_heading_index(&items, 7), Some(1));
        assert_eq!(current_heading_index(&items, 8), Some(2));
        assert_eq!(current_heading_index(&items, 100), Some(2));
    }

    #[test]
    fn current_heading_cursor_before_all() {
        let items = vec![OutlineItem {
            level: 1,
            text: "A".into(),
            line: 3,
        }];
        assert_eq!(current_heading_index(&items, 0), None);
        assert_eq!(current_heading_index(&items, 2), None);
    }

    #[test]
    fn current_heading_empty_items() {
        assert_eq!(current_heading_index(&[], 0), None);
        assert_eq!(current_heading_index(&[], 42), None);
    }

    // ---- helpers ----

    #[cfg(test)]
    fn oi(level: u8, line: usize) -> OutlineItem {
        OutlineItem {
            level,
            text: format!("H{level}"),
            line,
        }
    }

    // ---- has_children ----

    #[test]
    fn has_children_next_deeper_is_true() {
        let items = vec![oi(1, 0), oi(2, 2)];
        assert!(has_children(&items, 0));
    }

    #[test]
    fn has_children_next_same_level_is_false() {
        let items = vec![oi(1, 0), oi(1, 2)];
        assert!(!has_children(&items, 0));
    }

    #[test]
    fn has_children_next_shallower_is_false() {
        let items = vec![oi(2, 0), oi(1, 2)];
        assert!(!has_children(&items, 0));
    }

    #[test]
    fn has_children_last_item_is_false() {
        let items = vec![oi(1, 0)];
        assert!(!has_children(&items, 0));
    }

    #[test]
    fn has_children_out_of_bounds_is_false() {
        let items: Vec<OutlineItem> = vec![];
        assert!(!has_children(&items, 0));
    }

    // ---- compute_visibility ----

    #[test]
    fn visibility_empty_items() {
        let items: Vec<OutlineItem> = vec![];
        let collapsed = BTreeSet::new();
        let expected: Vec<bool> = vec![];
        assert_eq!(compute_visibility(&items, &collapsed), expected);
    }

    #[test]
    fn visibility_no_collapsed_all_visible() {
        let items = vec![oi(1, 0), oi(2, 1), oi(3, 2), oi(2, 3), oi(1, 4)];
        let collapsed = BTreeSet::new();
        assert_eq!(
            compute_visibility(&items, &collapsed),
            vec![true, true, true, true, true]
        );
    }

    #[test]
    fn visibility_collapse_h1_hides_all_children() {
        let items = vec![oi(1, 0), oi(2, 2), oi(3, 4), oi(2, 6), oi(1, 8)];
        let mut collapsed = BTreeSet::new();
        collapsed.insert(0);
        assert_eq!(
            compute_visibility(&items, &collapsed),
            vec![true, false, false, false, true]
        );
    }

    #[test]
    fn visibility_collapse_h2_hides_its_children_only() {
        let items = vec![oi(1, 0), oi(2, 2), oi(3, 4), oi(3, 6), oi(2, 8), oi(1, 10)];
        let mut collapsed = BTreeSet::new();
        collapsed.insert(1);
        assert_eq!(
            compute_visibility(&items, &collapsed),
            vec![true, true, false, false, true, true]
        );
    }

    #[test]
    fn visibility_collapse_last_heading_hides_nothing() {
        let items = vec![oi(1, 0), oi(2, 2)];
        let mut collapsed = BTreeSet::new();
        collapsed.insert(1);
        assert_eq!(compute_visibility(&items, &collapsed), vec![true, true]);
    }

    #[test]
    fn visibility_nested_collapse_ancestor_wins() {
        let items = vec![oi(1, 0), oi(2, 2), oi(3, 4), oi(2, 6), oi(1, 8)];
        let mut collapsed = BTreeSet::new();
        collapsed.insert(0);
        collapsed.insert(1);
        assert_eq!(
            compute_visibility(&items, &collapsed),
            vec![true, false, false, false, true]
        );
    }

    #[test]
    fn visibility_collapse_with_gap_in_levels() {
        let items = vec![oi(1, 0), oi(4, 2), oi(1, 4)];
        let mut collapsed = BTreeSet::new();
        collapsed.insert(0);
        assert_eq!(
            compute_visibility(&items, &collapsed),
            vec![true, false, true]
        );
    }

    #[test]
    fn visibility_sibling_collapsed_independent() {
        let items = vec![oi(1, 0), oi(2, 2), oi(2, 4), oi(3, 6)];
        let mut collapsed = BTreeSet::new();
        collapsed.insert(1);
        // idx 1 (H2) 下一个是 idx 2 (也是 H2, level=2 <= 2), 作用域结束
        // idx 3 (H3) 是 idx 2 (H2) 的子项, 不受 idx 1 折叠影响
        assert_eq!(
            compute_visibility(&items, &collapsed),
            vec![true, true, true, true]
        );
    }

    #[test]
    fn visibility_single_item_collapsed_no_children() {
        let items = vec![oi(1, 0)];
        let mut collapsed = BTreeSet::new();
        collapsed.insert(0);
        assert_eq!(compute_visibility(&items, &collapsed), vec![true]);
    }

    // ---- compute_outline_fingerprint ----

    #[test]
    fn fingerprint_same_for_same_structure() {
        let items_a = vec![oi(1, 0), oi(2, 2), oi(3, 5)];
        let items_b = vec![oi(1, 10), oi(2, 20), oi(3, 30)];
        assert_eq!(
            compute_outline_fingerprint(&items_a),
            compute_outline_fingerprint(&items_b)
        );
    }

    #[test]
    fn fingerprint_differs_for_different_levels() {
        let items_a = vec![oi(1, 0), oi(2, 2)];
        let items_b = vec![oi(1, 0), oi(3, 2)];
        assert_ne!(
            compute_outline_fingerprint(&items_a),
            compute_outline_fingerprint(&items_b)
        );
    }

    #[test]
    fn fingerprint_differs_for_different_count() {
        let items_a = vec![oi(1, 0), oi(2, 2)];
        let items_b = vec![oi(1, 0), oi(2, 2), oi(3, 4)];
        assert_ne!(
            compute_outline_fingerprint(&items_a),
            compute_outline_fingerprint(&items_b)
        );
    }

    #[test]
    fn fingerprint_empty_is_consistent() {
        let items: Vec<OutlineItem> = vec![];
        assert_eq!(
            compute_outline_fingerprint(&items),
            compute_outline_fingerprint(&items)
        );
    }

    // ---- OutlineFoldState ----

    #[test]
    fn fold_state_default_is_empty() {
        let fs = OutlineFoldState::default();
        assert!(fs.collapsed.is_empty());
        assert_eq!(fs.fingerprint, 0);
    }
}
