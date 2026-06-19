//! EditorState：聚合多标签页 + RecentFiles + Workspace。
//!
//! 对 UI 层提供高层操作：new_file / open / save / save_as / undo / redo。
//! 标签页切换保持各文档独立的 undo/redo 历史和光标位置。

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use document_model::{Document, parse};
use editor_engine::{Command, Editor};
use workspace::{RecentFiles, Workspace};

/// 单个标签页：独立编辑器 + 可选文件路径。
pub struct DocumentTab {
    pub editor: Editor,
    pub path: Option<PathBuf>,
}

impl DocumentTab {
    /// 空标签页（未命名、无修改）。
    pub fn empty() -> Self {
        let mut editor = Editor::empty();
        editor.mark_saved();
        Self { editor, path: None }
    }

    /// 从已有 Editor 和路径创建标签页。
    pub fn from_editor(editor: Editor, path: Option<PathBuf>) -> Self {
        Self { editor, path }
    }
}

/// 编辑器顶层状态。
pub struct EditorState {
    tabs: Vec<DocumentTab>,
    active_tab: usize,
    pub recent: RecentFiles,
    workspace: Workspace,
    /// 标记是否应退出（Quit 菜单触发）。
    pub should_exit: bool,
}

/// open / save 等操作的结果。
pub type OperationResult = Result<(), String>;

// ---- 委托方法：调用方通过方法而非直接访问字段来获取活跃标签页 ----
impl EditorState {
    /// 活跃标签页的 Editor（不可变）。
    pub fn editor(&self) -> &Editor {
        &self.tabs[self.active_tab].editor
    }

    /// 活跃标签页的 Editor（可变）。
    pub fn editor_mut(&mut self) -> &mut Editor {
        &mut self.tabs[self.active_tab].editor
    }

    /// 活跃标签页的文件路径。
    pub fn current_path(&self) -> Option<&Path> {
        self.tabs[self.active_tab].path.as_deref()
    }

    /// 所有标签页的切片（用于标签栏渲染）。
    pub fn tabs(&self) -> &[DocumentTab] {
        &self.tabs
    }

    /// 标签页数量。
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// 活跃标签页索引。
    pub fn active_tab_index(&self) -> usize {
        self.active_tab
    }

    /// 切换到指定索引的标签页。
    pub fn switch_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab = index;
        }
    }

    /// 切换到下一个标签页（循环）。
    pub fn next_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
        }
    }

    /// 切换到上一个标签页（循环）。
    pub fn prev_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.active_tab = (self.active_tab + self.tabs.len() - 1) % self.tabs.len();
        }
    }

    /// 关闭指定索引的标签页。
    ///
    /// 返回 `true` 表示标签页已被移除。
    /// 返回 `false` 表示这是最后一个标签页（调用方应创建新的空标签页或请求退出）。
    pub fn close_tab(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        self.tabs.remove(index);
        // 调整活跃标签页索引
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if index < self.active_tab {
            self.active_tab -= 1;
        }
        true
    }

    /// 指定索引标签页的显示名称。
    pub fn tab_title(&self, index: usize) -> String {
        self.tabs
            .get(index)
            .and_then(|t| t.path.as_ref())
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "未命名".to_string())
    }

    /// 指定索引标签页是否有未保存修改。
    pub fn tab_is_dirty(&self, index: usize) -> bool {
        self.tabs.get(index).is_some_and(|t| t.editor.is_dirty())
    }

    /// 是否有任意标签页存在未保存修改。
    pub fn any_dirty(&self) -> bool {
        self.tabs.iter().any(|t| t.editor.is_dirty())
    }
}

impl EditorState {
    /// 空编辑器（包含一个空标签页）。
    pub fn new() -> Self {
        let tab = DocumentTab::empty();
        Self {
            tabs: vec![tab],
            active_tab: 0,
            recent: RecentFiles::load(),
            workspace: Workspace::new(),
            should_exit: false,
        }
    }

    /// 新建文件：创建新标签页并切换到该页。
    pub fn new_file(&mut self) {
        self.tabs.push(DocumentTab::empty());
        self.active_tab = self.tabs.len() - 1;
    }

    /// 打开指定路径。
    ///
    /// 若当前标签页为空、洁净且未命名，则复用该标签页；否则创建新标签页。
    pub fn open(&mut self, path: &Path) -> OperationResult {
        let doc = self.workspace.open(path).map_err(|e| e.to_string())?;
        let mut editor = Editor::new(&document_model::to_markdown(&doc));
        editor.mark_saved();
        let tab = DocumentTab::from_editor(editor, Some(path.to_path_buf()));

        // 若当前标签页为空、洁净且未命名，则复用
        let current = &self.tabs[self.active_tab];
        let reuse = current.path.is_none()
            && !current.editor.is_dirty()
            && current.editor.to_string().is_empty();

        if reuse {
            self.tabs[self.active_tab] = tab;
        } else {
            self.tabs.push(tab);
            self.active_tab = self.tabs.len() - 1;
        }

        self.recent.add(path.to_path_buf());
        let _ = self.recent.save();
        Ok(())
    }

    /// 保存到活跃标签页的当前路径。无路径返回 Err（UI 应调 save_as）。
    pub fn save(&mut self) -> OperationResult {
        let tab = &self.tabs[self.active_tab];
        let path = tab
            .path
            .as_ref()
            .ok_or_else(|| "未设置当前路径".to_string())?;
        let doc = self.current_doc();
        self.workspace
            .save_to(path, &doc)
            .map_err(|e| e.to_string())?;
        self.tabs[self.active_tab].editor.mark_saved();
        Ok(())
    }

    /// 另存为。
    pub fn save_as(&mut self, path: &Path) -> OperationResult {
        let doc = self.current_doc();
        self.workspace
            .save_as(path, &doc)
            .map_err(|e| e.to_string())?;
        self.tabs[self.active_tab].editor.mark_saved();
        self.tabs[self.active_tab].path = Some(path.to_path_buf());
        self.recent.add(path.to_path_buf());
        let _ = self.recent.save();
        Ok(())
    }

    /// 应用编辑命令到活跃标签页。
    pub fn apply(&mut self, cmd: Command) -> OperationResult {
        self.editor_mut().apply(cmd).map_err(|e| e.to_string())
    }

    /// 撤销活跃标签页的最后一个操作。
    pub fn undo(&mut self) -> OperationResult {
        self.editor_mut()
            .undo()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// 重做活跃标签页的最后一个撤销操作。
    pub fn redo(&mut self) -> OperationResult {
        self.editor_mut()
            .redo()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// 活跃标签页是否有未保存修改。
    pub fn is_dirty(&self) -> bool {
        self.editor().is_dirty()
    }

    /// 窗口标题（活跃标签页文件名 + dirty 标记）。
    pub fn title(&self) -> String {
        let name = self
            .tabs
            .get(self.active_tab)
            .and_then(|t| t.path.as_ref())
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "未命名".to_string());
        let dirty = if self.is_dirty() { " *" } else { "" };
        format!("{name}{dirty} - zdown")
    }

    /// 当前文档（从活跃标签页的 editor 缓冲序列化为 Document）。
    pub fn current_doc(&self) -> Document {
        let src = self.editor().to_string();
        parse(&src).unwrap_or(Document { blocks: vec![] })
    }

    /// 请求退出。
    pub fn quit(&mut self) {
        self.should_exit = true;
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use editor_engine::Cursor;
    use tempfile::TempDir;

    // ---- 基础测试 ----

    #[test]
    fn new_state_is_empty_not_dirty() {
        let s = EditorState::new();
        assert!(!s.is_dirty());
        assert!(s.current_path().is_none());
        assert_eq!(s.title(), "未命名 - zdown");
        assert_eq!(s.tab_count(), 1);
        assert_eq!(s.active_tab_index(), 0);
    }

    #[test]
    fn title_shows_dirty_after_edit() {
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "x".into(),
        })
        .expect("apply");
        assert!(s.is_dirty());
        assert_eq!(s.title(), "未命名 * - zdown");
    }

    #[test]
    fn save_then_dirty_clears() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("a.md");
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "hello".into(),
        })
        .expect("apply");
        assert!(s.is_dirty());
        s.save_as(&path).expect("save");
        assert!(!s.is_dirty());
        assert_eq!(s.title(), "a.md - zdown");
    }

    #[test]
    fn open_sets_path_and_recent() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("doc.md");
        std::fs::write(&path, "# 标题\n").expect("write");
        let mut s = EditorState::new();
        s.open(&path).expect("open");
        assert_eq!(s.current_path(), Some(path.as_path()));
        assert!(!s.is_dirty());
        assert_eq!(s.title(), "doc.md - zdown");
        assert!(s.recent.list().contains(&path));
    }

    #[test]
    fn save_without_path_returns_err() {
        let mut s = EditorState::new();
        let err = s.save().unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn open_nonexistent_returns_err() {
        let mut s = EditorState::new();
        let err = s.open(Path::new("/nonexistent/xyz.md")).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn new_file_resets_state() {
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "hello".into(),
        })
        .expect("apply");
        s.new_file();
        // 原标签页仍在，但已切换到新标签页
        assert!(!s.is_dirty());
        assert!(s.current_path().is_none());
        assert_eq!(s.editor().to_string(), "");
        assert_eq!(s.tab_count(), 2);
    }

    #[test]
    fn edit_save_reopen_content_consistent() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("round.md");
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "# 标题\n\n段落\n".into(),
        })
        .expect("apply");
        s.save_as(&path).expect("save");

        let mut s2 = EditorState::new();
        s2.open(&path).expect("reopen");
        assert_eq!(s.editor().to_string(), s2.editor().to_string());
    }

    #[test]
    fn undo_redo_via_state() {
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "abc".into(),
        })
        .expect("apply");
        s.undo().expect("undo");
        assert_eq!(s.editor().to_string(), "");
        s.redo().expect("redo");
        assert_eq!(s.editor().to_string(), "abc");
    }

    #[test]
    fn quit_sets_should_exit() {
        let mut s = EditorState::new();
        assert!(!s.should_exit);
        s.quit();
        assert!(s.should_exit);
    }

    // ---- DocumentTab ----

    #[test]
    fn empty_tab_has_no_path_and_not_dirty() {
        let tab = DocumentTab::empty();
        assert!(tab.path.is_none());
        assert!(!tab.editor.is_dirty());
        assert_eq!(tab.editor.to_string(), "");
    }

    #[test]
    fn from_editor_preserves_state() {
        let mut editor = Editor::empty();
        editor
            .apply(Command::Insert {
                pos: Cursor::new(0, 0),
                text: "hello".into(),
            })
            .expect("insert");
        let path = PathBuf::from("/tmp/test.md");
        let tab = DocumentTab::from_editor(editor, Some(path.clone()));
        assert_eq!(tab.path, Some(path));
        assert_eq!(tab.editor.to_string(), "hello");
        assert!(tab.editor.is_dirty());
    }

    // ---- 多标签页 ----

    #[test]
    fn new_file_creates_new_tab() {
        let mut s = EditorState::new();
        assert_eq!(s.tab_count(), 1);
        s.new_file();
        assert_eq!(s.tab_count(), 2);
        assert_eq!(s.active_tab_index(), 1);
    }

    #[test]
    fn open_reuses_empty_clean_tab() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("doc.md");
        std::fs::write(&path, "# hi\n").expect("write");
        let mut s = EditorState::new();
        s.open(&path).expect("open");
        assert_eq!(s.tab_count(), 1); // 复用而非新增
        assert_eq!(s.current_path(), Some(path.as_path()));
    }

    #[test]
    fn open_creates_new_tab_when_current_has_content() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("doc.md");
        std::fs::write(&path, "# hi\n").expect("write");
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "x".into(),
        })
        .expect("apply");
        s.open(&path).expect("open");
        assert_eq!(s.tab_count(), 2);
        assert_eq!(s.active_tab_index(), 1);
        // 原标签页内容未被覆盖
        assert_eq!(s.tabs[0].editor.to_string(), "x");
    }

    #[test]
    fn switch_tab_preserves_editor_state() {
        let mut s = EditorState::new();
        // 在 tab 0 编辑
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "tab0".into(),
        })
        .expect("apply");
        // 创建 tab 1 并编辑
        s.new_file();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "tab1".into(),
        })
        .expect("apply");
        // 切换回 tab 0
        s.switch_tab(0);
        assert_eq!(s.editor().to_string(), "tab0");
        // 再切换到 tab 1
        s.switch_tab(1);
        assert_eq!(s.editor().to_string(), "tab1");
    }

    #[test]
    fn close_tab_removes_correct_tab() {
        let mut s = EditorState::new();
        s.new_file();
        s.new_file();
        assert_eq!(s.tab_count(), 3);
        // 关闭中间的标签页 (index 1)
        s.close_tab(1);
        assert_eq!(s.tab_count(), 2);
        // 原 tabs[0] 仍在，原 tabs[2] 移到 tabs[1]
        assert_eq!(s.active_tab_index(), 1); // 活跃标签页保持在 index 2 → 1
    }

    #[test]
    fn close_last_tab_returns_false() {
        let mut s = EditorState::new();
        assert_eq!(s.tab_count(), 1);
        let removed = s.close_tab(0);
        assert!(!removed);
        assert_eq!(s.tab_count(), 1);
    }

    #[test]
    fn close_tab_adjusts_active_index() {
        let mut s = EditorState::new();
        s.new_file();
        s.new_file(); // tabs: [空, 空, 空], active=2
        s.switch_tab(1); // active=1
        let removed = s.close_tab(1);
        assert!(removed);
        assert_eq!(s.tab_count(), 2);
        assert_eq!(s.active_tab_index(), 1); // 原 tabs[2] 移到 tabs[1]
    }

    #[test]
    fn undo_redo_per_tab() {
        let mut s = EditorState::new();
        // tab 0: insert "A"
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "A".into(),
        })
        .expect("apply");
        s.undo().expect("undo"); // tab 0: empty
        // tab 1: insert "B"
        s.new_file();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "B".into(),
        })
        .expect("apply");
        // 切回 tab 0，重做
        s.switch_tab(0);
        assert_eq!(s.editor().to_string(), "");
        s.redo().expect("redo");
        assert_eq!(s.editor().to_string(), "A");
        // 切回 tab 1，不做任何操作，内容应保持
        s.switch_tab(1);
        assert_eq!(s.editor().to_string(), "B");
    }

    #[test]
    fn save_uses_active_tab_path() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("one.md");
        std::fs::write(&path, "# one\n").expect("write");
        let mut s = EditorState::new();
        s.open(&path).expect("open");
        // 在文档开头插入文本
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "modified ".into(),
        })
        .expect("apply");
        assert!(s.is_dirty());
        s.save().expect("save");
        assert!(!s.is_dirty());
        let content = std::fs::read_to_string(&path).expect("read");
        assert!(content.contains("modified"));
    }

    #[test]
    fn recent_shared_across_tabs() {
        let dir = TempDir::new().expect("tempdir");
        let p1 = dir.path().join("a.md");
        let p2 = dir.path().join("b.md");
        std::fs::write(&p1, "# A\n").expect("write");
        std::fs::write(&p2, "# B\n").expect("write");
        let mut s = EditorState::new();
        s.open(&p1).expect("open 1");
        assert!(s.recent.list().contains(&p1));
        s.new_file();
        s.open(&p2).expect("open 2");
        assert!(s.recent.list().contains(&p1));
        assert!(s.recent.list().contains(&p2));
    }

    #[test]
    fn any_dirty_true_when_any_tab_dirty() {
        let mut s = EditorState::new();
        assert!(!s.any_dirty());
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "x".into(),
        })
        .expect("apply");
        assert!(s.any_dirty());
        s.new_file();
        assert!(s.any_dirty()); // tab 0 仍 dirty
        s.switch_tab(0);
        s.undo().expect("undo");
        assert!(!s.any_dirty());
    }

    #[test]
    fn active_tab_dirty_independent() {
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "x".into(),
        })
        .expect("apply");
        assert!(s.is_dirty());
        s.new_file();
        assert!(!s.is_dirty()); // 新标签页清洁
        s.switch_tab(0);
        assert!(s.is_dirty()); // 旧标签页 dirty
    }

    #[test]
    fn title_reflects_active_tab() {
        let dir = TempDir::new().expect("tempdir");
        let p1 = dir.path().join("alpha.md");
        let p2 = dir.path().join("beta.md");
        std::fs::write(&p1, "# A\n").expect("write");
        std::fs::write(&p2, "# B\n").expect("write");
        let mut s = EditorState::new();
        s.open(&p1).expect("open 1");
        assert_eq!(s.title(), "alpha.md - zdown");
        s.new_file();
        s.open(&p2).expect("open 2");
        assert_eq!(s.title(), "beta.md - zdown");
        s.switch_tab(0);
        assert_eq!(s.title(), "alpha.md - zdown");
    }

    #[test]
    fn save_as_updates_active_tab_path() {
        let dir = TempDir::new().expect("tempdir");
        let p1 = dir.path().join("x.md");
        let p2 = dir.path().join("y.md");
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "content".into(),
        })
        .expect("apply");
        s.save_as(&p1).expect("save_as 1");
        assert_eq!(s.current_path(), Some(p1.as_path()));

        s.new_file();
        s.save_as(&p2).expect("save_as 2");
        assert_eq!(s.current_path(), Some(p2.as_path()));
        s.switch_tab(0);
        assert_eq!(s.current_path(), Some(p1.as_path()));
    }

    #[test]
    fn open_preserves_other_tabs() {
        let dir = TempDir::new().expect("tempdir");
        let p = dir.path().join("open.md");
        std::fs::write(&p, "# new\n").expect("write");
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "existing content".into(),
        })
        .expect("apply");
        assert_eq!(s.tab_count(), 1);
        s.open(&p).expect("open");
        assert_eq!(s.tab_count(), 2);
        assert_eq!(s.tabs[0].editor.to_string(), "existing content");
        assert_eq!(s.tabs[1].editor.to_string(), "# new\n");
    }

    #[test]
    fn next_prev_tab_cycles() {
        let mut s = EditorState::new();
        s.new_file();
        s.new_file();
        // tabs: [0:空, 1:空, 2:空], active=2
        s.next_tab();
        assert_eq!(s.active_tab_index(), 0);
        s.next_tab();
        assert_eq!(s.active_tab_index(), 1);
        s.prev_tab();
        assert_eq!(s.active_tab_index(), 0);
        s.prev_tab();
        assert_eq!(s.active_tab_index(), 2);
    }
}
