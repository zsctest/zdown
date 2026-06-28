//! 大纲/目录面板：从 Document AST 提取标题，侧边栏渲染。

use std::collections::BTreeSet;

use document_model::ast::{Block, BlockWithSpan, Document, Inline};
use document_model::to_markdown;
use editor_engine::Cursor;
use eframe::egui;
use fluent_bundle::FluentArgs;
use i18n::I18n;

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

/// 大纲拖拽排序状态。
#[derive(Debug, Clone, Default)]
pub struct OutlineDragState {
    /// 当前正在拖拽的标题索引。
    pub dragged_index: Option<usize>,
    /// 拖拽悬停的目标位置索引。
    pub drop_target: Option<usize>,
}

/// 大纲搜索/过滤状态。
#[derive(Debug, Clone, Default)]
pub struct OutlineFilterState {
    /// 过滤查询字符串。
    pub query: String,
    /// 输入框是否需要获取焦点。
    pub focus: bool,
}

/// 过滤大纲项：大小写不敏感子串匹配。
/// 返回匹配项的索引列表。空查询返回全部。
pub fn filter_outline_items(items: &[OutlineItem], query: &str) -> Vec<usize> {
    if query.trim().is_empty() {
        return (0..items.len()).collect();
    }
    let query_lower = query.to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.text.to_lowercase().contains(&query_lower))
        .map(|(i, _)| i)
        .collect()
}

/// 自动展开匹配项的所有祖先，确保过滤结果可见。
pub fn expand_ancestors_for_filter(
    items: &[OutlineItem],
    collapsed: &mut BTreeSet<usize>,
    matched_indices: &[usize],
) {
    for &idx in matched_indices {
        auto_expand_ancestors(items, collapsed, idx);
    }
}

/// 将行内节点转换为纯文本（去除内联标记）。
fn inlines_to_plain(inlines: &[Inline], i18n: &I18n) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(s) => text.push_str(s),
            Inline::Emph(inner) => text.push_str(&inlines_to_plain(inner, i18n)),
            Inline::Strong(inner) => text.push_str(&inlines_to_plain(inner, i18n)),
            Inline::Code(s) => text.push_str(s),
            Inline::Link {
                text: link_text, ..
            } => text.push_str(&inlines_to_plain(link_text, i18n)),
            Inline::Image { alt, .. } => {
                text.push('[');
                text.push_str(&i18n.t("outline-image-prefix"));
                text.push(' ');
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
pub fn extract_outline(doc: &Document, i18n: &I18n) -> Vec<OutlineItem> {
    doc.blocks
        .iter()
        .filter_map(|bws| match &bws.block {
            Block::Heading(h) => {
                let text = inlines_to_plain(&h.inlines, i18n);
                let text = if text.is_empty() {
                    i18n.t("outline-empty-heading")
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

/// 自动展开所有隐藏当前标题的折叠祖先。
///
/// 从当前标题向前遍历，展开所有层级小于当前标题且被折叠的祖先项，
/// 确保光标所在章节在大纲中始终可见。
fn auto_expand_ancestors(
    items: &[OutlineItem],
    collapsed: &mut BTreeSet<usize>,
    target_idx: usize,
) {
    let target_level = items[target_idx].level;
    let mut min_level = target_level;
    for i in (0..target_idx).rev() {
        if items[i].level < min_level {
            // 这是目标标题的一个祖先
            collapsed.remove(&i);
            min_level = items[i].level;
        }
    }
}

/// 重排文档块：将大纲索引 `from_idx` 对应的 section 移动到 `to_idx`。
///
/// `to_idx` 是移动后 section 在大纲中的新索引位置。
/// Section 范围包括标题及其后所有块，直到下一个标题。
///
/// 返回新的 Markdown 文本，若无需操作则返回 `None`。
pub fn reorder_blocks(
    doc: &Document,
    items: &[OutlineItem],
    from_idx: usize,
    to_idx: usize,
) -> Option<String> {
    if from_idx == to_idx || from_idx >= items.len() || to_idx >= items.len() || items.len() < 2 {
        return None;
    }

    // 建立每个 heading 对应的 block 范围（section）
    let heading_block_indices: Vec<usize> = doc
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(i, bws)| matches!(bws.block, Block::Heading(_)).then_some(i))
        .collect();

    if heading_block_indices.len() != items.len() {
        return None;
    }

    // 为每个 outline 项计算其 block 范围 [start, end)
    let sections: Vec<(usize, usize)> = heading_block_indices
        .iter()
        .enumerate()
        .map(|(i, &start)| {
            let end = heading_block_indices
                .get(i + 1)
                .copied()
                .unwrap_or(doc.blocks.len());
            (start, end)
        })
        .collect();

    // 构建新的 outline 顺序
    let mut new_order: Vec<usize> = (0..items.len()).collect();
    let moved = new_order.remove(from_idx);
    new_order.insert(to_idx, moved);

    // 按新顺序拼接所有 section 的 blocks
    let mut new_blocks: Vec<BlockWithSpan> = Vec::with_capacity(doc.blocks.len());
    for &idx in &new_order {
        let (start, end) = sections[idx];
        for j in start..end {
            new_blocks.push(doc.blocks[j].clone());
        }
    }

    let new_doc = Document { blocks: new_blocks };
    Some(to_markdown(&new_doc))
}

/// 从大纲项生成 Markdown 目录（TOC）。
///
/// 返回 (toc_string, heading_count)，仅包含可见项。
pub fn generate_toc(items: &[OutlineItem], visible: &[bool]) -> (String, usize) {
    let mut toc = String::new();
    let mut count = 0;
    for (i, item) in items.iter().enumerate() {
        if i < visible.len() && !visible[i] {
            continue;
        }
        count += 1;
        let indent = "  ".repeat(item.level.saturating_sub(1) as usize);
        // 生成 anchor：小写，去除非字母数字，空格转连字符
        let anchor = item
            .text
            .chars()
            .filter_map(|c| {
                if c.is_alphanumeric() {
                    Some(c.to_ascii_lowercase())
                } else if c == ' ' || c == '-' {
                    Some('-')
                } else {
                    None
                }
            })
            .collect::<String>();
        toc.push_str(&format!("{}- [{}](#{})\n", indent, item.text, anchor));
    }
    (toc, count)
}

/// 显示大纲面板。
pub fn show_outline_panel(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    fold_state: &mut OutlineFoldState,
    drag_state: &mut OutlineDragState,
    filter_state: &mut OutlineFilterState,
    i18n: &I18n,
) {
    let doc = state.current_doc();
    let items = extract_outline(&doc, i18n);

    let mut args = FluentArgs::new();
    args.set("count", items.len() as i64);

    ui.horizontal(|ui| {
        ui.heading(i18n.tr("outline-heading", Some(&args)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("📋")
                .on_hover_text(i18n.t("outline-copy-toc"))
                .clicked()
            {
                let vis = compute_visibility(&items, &fold_state.collapsed);
                let (toc, count) = generate_toc(&items, &vis);
                ui.ctx().copy_text(toc);
                let mut status_args = FluentArgs::new();
                status_args.set("count", count as i64);
                state.status_message = i18n.tr("outline-toc-copied", Some(&status_args));
            }
        });
    });

    // 过滤输入框
    let filter_id = egui::Id::new("outline_filter_input");
    ui.add(
        egui::TextEdit::singleline(&mut filter_state.query)
            .id(filter_id)
            .hint_text(i18n.t("outline-filter-placeholder"))
            .desired_width(f32::INFINITY),
    );
    if filter_state.focus {
        ui.ctx().memory_mut(|m| m.request_focus(filter_id));
        filter_state.focus = false;
    }

    // 过滤并自动展开匹配项的祖先
    let matched = filter_outline_items(&items, &filter_state.query);
    let is_filtering = !filter_state.query.trim().is_empty();
    if is_filtering {
        expand_ancestors_for_filter(&items, &mut fold_state.collapsed, &matched);
    }

    // 检测文档结构变化，若指纹不匹配则重置折叠状态
    let fp = compute_outline_fingerprint(&items);
    if fp != fold_state.fingerprint {
        fold_state.collapsed.clear();
        fold_state.fingerprint = fp;
    }
    // 清理越界索引
    fold_state.collapsed.retain(|&i| i < items.len());

    if items.is_empty() {
        ui.label(egui::RichText::new(i18n.t("outline-empty")).weak());
        return;
    }

    // 计算可见性和当前标题索引
    let visible = compute_visibility(&items, &fold_state.collapsed);
    let cursor_line = state.editor().cursor.line;
    let current_idx = current_heading_index(&items, cursor_line);

    // 若当前标题被折叠隐藏，自动展开其祖先链
    if let Some(idx) = current_idx {
        if !visible[idx] {
            auto_expand_ancestors(&items, &mut fold_state.collapsed, idx);
        }
    }

    // 构建过滤匹配集合（用于快速查找）
    let matched_set: BTreeSet<usize> = matched.iter().copied().collect();

    egui::ScrollArea::vertical()
        .id_salt("outline_scroll")
        .show(ui, |ui| {
            for (i, item) in items.iter().enumerate() {
                // 过滤模式下，仅显示匹配项及其祖先（维持树结构上下文）
                if is_filtering && !matched_set.contains(&i) {
                    // 检查是否为某个匹配项的祖先（需要在可见性链上）。
                    // 必须验证 i 和 m 之间没有同级或更低级标题打断祖先关系。
                    let is_ancestor_of_match = matched_set.iter().any(|&m| {
                        m > i
                            && items[i].level < items[m].level
                            && !items[i + 1..m]
                                .iter()
                                .any(|item| item.level <= items[i].level)
                    });
                    if !is_ancestor_of_match {
                        continue;
                    }
                }

                if !visible[i] {
                    continue;
                }

                let is_current = current_idx == Some(i);
                let indent = (item.level.saturating_sub(1) as f32) * 16.0;
                let can_fold = has_children(&items, i);
                let is_collapsed = can_fold && fold_state.collapsed.contains(&i);

                let is_dragging = drag_state.dragged_index == Some(i);
                let is_drop_target = drag_state.drop_target == Some(i);

                // 拖拽目标指示线
                if is_drop_target && !is_dragging {
                    ui.add_space(1.0);
                    ui.separator();
                }

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

                    // 拖拽中高亮被拖拽的项
                    let text = if is_dragging {
                        text.background_color(egui::Color32::from_rgba_premultiplied(
                            80, 120, 200, 80,
                        ))
                    } else {
                        text
                    };

                    let response = ui.selectable_label(is_current, text);

                    // 拖拽交互：只在非拖拽状态时允许点击导航
                    if response.clicked() && drag_state.dragged_index.is_none() {
                        let cursor = Cursor::new(item.line, 0);
                        let _ = state.editor_mut().set_cursor(cursor);
                        state.needs_scroll_cursor = true;
                    }

                    // 拖拽开始检测
                    if response.drag_started() {
                        drag_state.dragged_index = Some(i);
                    }

                    // 拖拽悬停检测
                    if response.contains_pointer() && drag_state.dragged_index.is_some() {
                        drag_state.drop_target = Some(i);
                    }
                });
            }
        });

    // 拖拽释放检测：在 ScrollArea 之外统一处理
    if drag_state.dragged_index.is_some()
        && ui
            .ctx()
            .input(|i| i.pointer.button_released(egui::PointerButton::Primary))
    {
        if let Some(dragged) = drag_state.dragged_index {
            if let Some(target) = drag_state.drop_target {
                if target < items.len() && dragged < items.len() {
                    if let Some(new_md) = reorder_blocks(&doc, &items, dragged, target) {
                        let _ = state.apply(editor_engine::Command::ReplaceAll { text: new_md });
                    }
                }
            }
        }
        drag_state.dragged_index = None;
        drag_state.drop_target = None;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
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

    fn i18n_zh() -> I18n {
        I18n::with_lang(i18n::Lang::ZhCN)
    }

    #[test]
    fn extract_outline_empty_doc() {
        let doc = doc_from_blocks(vec![]);
        assert_eq!(extract_outline(&doc, &i18n_zh()), vec![]);
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
        let items = extract_outline(&doc, &i18n_zh());
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
        let items = extract_outline(&doc, &i18n_zh());
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
        assert_eq!(extract_outline(&doc, &i18n_zh()), vec![]);
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
        let items = extract_outline(&doc, &i18n_zh());
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
        let items = extract_outline(&doc, &i18n_zh());
        assert_eq!(items[0].text, "（空标题）");
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
        let items = extract_outline(&doc, &i18n_zh());
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

    // ---- auto_expand_ancestors ----

    #[test]
    fn auto_expand_single_ancestor() {
        // H1 (collapsed), H2, H3 <-- target
        let items = vec![oi(1, 0), oi(2, 2), oi(3, 4)];
        let mut collapsed = BTreeSet::new();
        collapsed.insert(0); // H1 collapsed
        auto_expand_ancestors(&items, &mut collapsed, 2);
        assert!(collapsed.is_empty());
    }

    #[test]
    fn auto_expand_multi_level_ancestors() {
        // H1 (collapsed), H2 (collapsed), H3, H4 <-- target
        let items = vec![oi(1, 0), oi(2, 2), oi(3, 4), oi(4, 6)];
        let mut collapsed = BTreeSet::new();
        collapsed.insert(0); // H1
        collapsed.insert(1); // H2
        auto_expand_ancestors(&items, &mut collapsed, 3);
        assert!(collapsed.is_empty());
    }

    #[test]
    fn auto_expand_preserves_unrelated_collapse() {
        // H1, H2 (collapsed), H3 <-- target, H1_b, H2_b (collapsed)
        let items = vec![oi(1, 0), oi(2, 2), oi(3, 4), oi(1, 6), oi(2, 8)];
        let mut collapsed = BTreeSet::new();
        collapsed.insert(1); // first H2
        collapsed.insert(4); // second H2 (unrelated to target)
        auto_expand_ancestors(&items, &mut collapsed, 2);
        assert!(!collapsed.contains(&1)); // expanded
        assert!(collapsed.contains(&4)); // preserved — unrelated
    }

    #[test]
    fn auto_expand_already_visible_does_nothing() {
        let items = vec![oi(1, 0), oi(2, 2)];
        let mut collapsed = BTreeSet::new();
        // nothing collapsed
        auto_expand_ancestors(&items, &mut collapsed, 1);
        assert!(collapsed.is_empty());
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

    // ---- reorder_blocks ----

    fn heading_doc(level: u8, text: &str, line: usize) -> BlockWithSpan {
        bws(
            Block::Heading(Heading {
                level,
                inlines: vec![Inline::Text(text.into())],
            }),
            line,
            line,
        )
    }

    fn para_doc(text: &str, line: usize) -> BlockWithSpan {
        bws(
            Block::Paragraph(document_model::ast::Paragraph {
                inlines: vec![Inline::Text(text.into())],
            }),
            line,
            line,
        )
    }

    #[test]
    fn reorder_move_first_after_second() {
        // Document: # A, # B → # B, # A
        let doc = doc_from_blocks(vec![heading_doc(1, "A", 0), heading_doc(1, "B", 1)]);
        let items = extract_outline(&doc, &i18n_zh());
        let result = reorder_blocks(&doc, &items, 0, 1).expect("reorder");
        let new_doc = document_model::parse(&result).expect("parse");
        let new_items = extract_outline(&new_doc, &i18n_zh());
        assert_eq!(new_items[0].text, "B");
        assert_eq!(new_items[1].text, "A");
    }

    #[test]
    fn reorder_noop_same_position() {
        let doc = doc_from_blocks(vec![heading_doc(1, "A", 0), heading_doc(1, "B", 1)]);
        let items = extract_outline(&doc, &i18n_zh());
        assert!(reorder_blocks(&doc, &items, 0, 0).is_none());
    }

    #[test]
    fn reorder_preserves_non_heading_blocks() {
        // Document: # A, paragraph, # B → # B, # A, paragraph
        let doc = doc_from_blocks(vec![
            heading_doc(1, "A", 0),
            para_doc("content A", 1),
            heading_doc(1, "B", 2),
        ]);
        let items = extract_outline(&doc, &i18n_zh());
        let result = reorder_blocks(&doc, &items, 0, 1).expect("reorder");
        // After reorder: # B, # A, paragraph (content moves with heading A)
        let new_doc = document_model::parse(&result).expect("parse");
        let new_items = extract_outline(&new_doc, &i18n_zh());
        assert_eq!(new_items[0].text, "B");
        assert_eq!(new_items[1].text, "A");
        // Verify the paragraph is after heading A (which is now at index 1)
        assert!(new_doc.blocks.len() >= 3);
    }

    #[test]
    fn reorder_move_heading_with_content_to_end() {
        let doc = doc_from_blocks(vec![
            heading_doc(1, "A", 0),
            para_doc("content A", 1),
            heading_doc(1, "B", 2),
            para_doc("content B", 3),
        ]);
        let items = extract_outline(&doc, &i18n_zh());
        // Move H1 "A" (with its paragraph) after H1 "B"
        let result = reorder_blocks(&doc, &items, 0, 1).expect("reorder");
        let new_doc = document_model::parse(&result).expect("parse");
        let new_items = extract_outline(&new_doc, &i18n_zh());
        assert_eq!(new_items[0].text, "B");
        assert_eq!(new_items[1].text, "A");
    }

    #[test]
    fn reorder_out_of_bounds_returns_none() {
        let doc = doc_from_blocks(vec![heading_doc(1, "A", 0), heading_doc(1, "B", 1)]);
        let items = extract_outline(&doc, &i18n_zh());
        assert!(reorder_blocks(&doc, &items, 0, 5).is_none());
        assert!(reorder_blocks(&doc, &items, 5, 0).is_none());
    }

    #[test]
    fn reorder_single_heading_returns_none() {
        let doc = doc_from_blocks(vec![heading_doc(1, "Only", 0)]);
        let items = extract_outline(&doc, &i18n_zh());
        assert!(reorder_blocks(&doc, &items, 0, 0).is_none());
    }

    // ---- filter_outline_items ----

    fn make_item(text: &str) -> OutlineItem {
        OutlineItem {
            level: 1,
            text: text.into(),
            line: 0,
        }
    }

    #[test]
    fn filter_empty_returns_all() {
        let items = vec![make_item("简介"), make_item("安装"), make_item("配置")];
        let result = filter_outline_items(&items, "");
        assert_eq!(result, vec![0, 1, 2]);
    }

    #[test]
    fn filter_case_insensitive() {
        let items = vec![
            make_item("Introduction"),
            make_item("INSTALL"),
            make_item("Config"),
        ];
        let result = filter_outline_items(&items, "in");
        assert_eq!(result, vec![0, 1]); // Introduction and INSTALL both contain "in" case-insensitively
    }

    #[test]
    fn filter_whitespace_only_returns_all() {
        let items = vec![make_item("A"), make_item("B")];
        let result = filter_outline_items(&items, "   ");
        assert_eq!(result, vec![0, 1]);
    }

    #[test]
    fn filter_no_match_returns_empty() {
        let items = vec![make_item("Hello"), make_item("World")];
        let result = filter_outline_items(&items, "xyz");
        assert!(result.is_empty());
    }

    #[test]
    fn filter_chinese_characters() {
        let items = vec![
            make_item("简介"),
            make_item("安装指南"),
            make_item("配置说明"),
            make_item("Introduction"),
        ];
        let result = filter_outline_items(&items, "安");
        assert_eq!(result, vec![1]); // Only "安装指南" contains "安"
    }

    // ---- generate_toc ----

    #[test]
    fn toc_flat_headings() {
        let items = vec![oi(1, 0), oi(1, 2), oi(1, 4)];
        let visible = vec![true; 3];
        let (toc, count) = generate_toc(&items, &visible);
        assert_eq!(count, 3);
        assert!(toc.contains("- [H1](#h1)"));
    }

    #[test]
    fn toc_nested_indentation() {
        let items = vec![oi(1, 0), oi(2, 2), oi(3, 4)];
        let visible = vec![true; 3];
        let (toc, count) = generate_toc(&items, &visible);
        assert_eq!(count, 3);
        assert!(toc.contains("- [H1](#h1)"));
        assert!(toc.contains("  - [H2](#h2)"));
        assert!(toc.contains("    - [H3](#h3)"));
    }

    #[test]
    fn toc_respects_visibility() {
        let items = vec![oi(1, 0), oi(2, 2), oi(1, 4)];
        let visible = vec![true, false, true];
        let (toc, count) = generate_toc(&items, &visible);
        assert_eq!(count, 2);
        assert!(!toc.contains("H2")); // hidden item should not appear
    }

    #[test]
    fn toc_anchor_chinese_chars_kept() {
        let items = vec![OutlineItem {
            level: 1,
            text: "简介 Intro".into(),
            line: 0,
        }];
        let visible = vec![true];
        let (toc, count) = generate_toc(&items, &visible);
        assert_eq!(count, 1);
        // 中文被视为 alphabetic，空格转为连字符
        assert!(toc.contains("(#简介-intro)"), "TOC: {}", toc);
    }

    // ---- 完整管道测试：EditorState → current_doc() → extract_outline() ----

    /// 测试从 EditorState 到 extract_outline 的完整管道。
    #[test]
    fn pipeline_editor_state_to_outline() {
        let mut state = EditorState::default();
        let i18n = i18n_zh();

        // 初始状态：空文档 → 空大纲
        {
            let doc = state.current_doc();
            let items = extract_outline(&doc, &i18n);
            assert!(items.is_empty(), "空编辑器应产生空大纲");
        }

        // 设置 markdown 内容（含中文标题）
        let _ = state.apply(editor_engine::Command::ReplaceAll {
            text: "# 简介\n\n一些文字。\n\n## 安装\n\n安装说明。\n\n### 配置\n\n配置说明。\n"
                .into(),
        });

        // 验证大纲提取
        {
            let doc = state.current_doc();
            let items = extract_outline(&doc, &i18n);
            assert_eq!(items.len(), 3, "应有 3 个标题");
            assert_eq!(items[0].level, 1);
            assert_eq!(items[0].text, "简介");
            assert_eq!(items[0].line, 0);
            assert_eq!(items[1].level, 2);
            assert_eq!(items[1].text, "安装");
            assert_eq!(items[2].level, 3);
            assert_eq!(items[2].text, "配置");
        }

        // 修改内容（替换为不同的标题结构）
        let _ = state.apply(editor_engine::Command::ReplaceAll {
            text: "# 新标题\n\n内容。\n".into(),
        });

        // 验证大纲更新
        {
            let doc = state.current_doc();
            let items = extract_outline(&doc, &i18n);
            assert_eq!(items.len(), 1, "修改后应有 1 个标题");
            assert_eq!(items[0].text, "新标题");
        }
    }

    /// 测试编辑后立即反映到大纲中。
    #[test]
    fn pipeline_outline_reflects_edits() {
        let mut state = EditorState::default();
        let i18n = i18n_zh();

        // 逐步构建 markdown
        let _ = state.apply(editor_engine::Command::ReplaceAll {
            text: "# 标题\n".into(),
        });
        {
            let items = extract_outline(&state.current_doc(), &i18n);
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].text, "标题");
        }

        // 添加更多标题
        let _ = state.apply(editor_engine::Command::ReplaceAll {
            text: "# 标题\n\n## 子标题\n\n### 嵌套\n\n# 另一个\n".into(),
        });
        {
            let items = extract_outline(&state.current_doc(), &i18n);
            assert_eq!(items.len(), 4, "应有 4 个标题");
            assert_eq!(items[1].text, "子标题");
            assert_eq!(items[2].text, "嵌套");
            assert_eq!(items[3].text, "另一个");
        }
    }

    /// 测试没有标题的 markdown 返回空大纲。
    #[test]
    fn pipeline_no_headings_returns_empty() {
        let mut state = EditorState::default();
        let i18n = i18n_zh();

        let _ = state.apply(editor_engine::Command::ReplaceAll {
            text: "这是一段纯文本，没有任何标题。\n\n另一段。\n".into(),
        });

        let items = extract_outline(&state.current_doc(), &i18n);
        assert!(items.is_empty(), "无标题文档应返回空大纲");
    }

    /// 测试英中混合标题。
    #[test]
    fn pipeline_mixed_language_headings() {
        let mut state = EditorState::default();
        let i18n = i18n_zh();

        let _ = state.apply(editor_engine::Command::ReplaceAll {
            text: "# Introduction\n\n## 安装 Installation\n\n### 步骤 1: Setup\n".into(),
        });

        let items = extract_outline(&state.current_doc(), &i18n);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].text, "Introduction");
        assert_eq!(items[1].text, "安装 Installation");
        assert_eq!(items[2].text, "步骤 1: Setup");
    }
}
