//! EditorState：聚合 Editor + 当前路径 + RecentFiles + Workspace。
//!
//! 对 UI 层提供高层操作：new_file / open / save / save_as / undo / redo。
//! UI 事件转发到 Editor 的 Command。
//!
//! 注：当前阶段（Plan 4 任务 2）仅落地聚合层与测试，UI 接入在任务 3+ 完成，
//! 故大量 API 暂未被外部调用，dead_code 通过 `allow(dead_code)` 标注，
//! 任务 3+ 接入后移除。

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use document_model::{Document, parse};
use editor_engine::{Command, Editor};
use workspace::{RecentFiles, Workspace};

/// 编辑器顶层状态。
pub struct EditorState {
    pub editor: Editor,
    pub current_path: Option<PathBuf>,
    pub recent: RecentFiles,
    workspace: Workspace,
    /// 标记是否应退出（Quit 菜单触发）。
    pub should_exit: bool,
}

/// open / save 等操作的结果。
pub type OperationResult = Result<(), String>;

impl EditorState {
    /// 空编辑器。
    pub fn new() -> Self {
        Self {
            editor: Editor::empty(),
            current_path: None,
            recent: RecentFiles::load(),
            workspace: Workspace::new(),
            should_exit: false,
        }
    }

    /// 新建文件。要求调用方先确认未保存修改（UI 弹对话框）。
    pub fn new_file(&mut self) {
        self.editor = Editor::empty();
        self.current_path = None;
        self.editor.mark_saved();
    }

    /// 打开指定路径。
    pub fn open(&mut self, path: &Path) -> OperationResult {
        let doc = self.workspace.open(path).map_err(|e| e.to_string())?;
        self.editor = Editor::new(&document_model::to_markdown(&doc));
        self.editor.mark_saved();
        self.current_path = Some(path.to_path_buf());
        self.recent.add(path.to_path_buf());
        let _ = self.recent.save();
        Ok(())
    }

    /// 保存到当前路径。无路径返回 Err（UI 应调 save_as）。
    pub fn save(&mut self) -> OperationResult {
        let doc = self.current_doc();
        self.workspace.save(&doc).map_err(|e| e.to_string())?;
        self.editor.mark_saved();
        Ok(())
    }

    /// 另存为。
    pub fn save_as(&mut self, path: &Path) -> OperationResult {
        let doc = self.current_doc();
        self.workspace
            .save_as(path, &doc)
            .map_err(|e| e.to_string())?;
        self.editor.mark_saved();
        self.current_path = Some(path.to_path_buf());
        self.recent.add(path.to_path_buf());
        let _ = self.recent.save();
        Ok(())
    }

    /// 应用编辑命令。
    pub fn apply(&mut self, cmd: Command) -> OperationResult {
        self.editor.apply(cmd).map_err(|e| e.to_string())
    }

    /// 撤销。
    pub fn undo(&mut self) -> OperationResult {
        self.editor.undo().map(|_| ()).map_err(|e| e.to_string())
    }

    /// 重做。
    pub fn redo(&mut self) -> OperationResult {
        self.editor.redo().map(|_| ()).map_err(|e| e.to_string())
    }

    /// 是否有未保存修改。
    pub fn is_dirty(&self) -> bool {
        self.editor.is_dirty()
    }

    /// 窗口标题（文件名 + dirty 标记）。
    pub fn title(&self) -> String {
        let name = self
            .current_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "未命名".to_string());
        let dirty = if self.is_dirty() { " *" } else { "" };
        format!("{name}{dirty} - zdown")
    }

    /// 当前文档（从 editor 缓冲序列化为 Document）。
    pub fn current_doc(&self) -> Document {
        let src = self.editor.to_string();
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

    #[test]
    fn new_state_is_empty_not_dirty() {
        let s = EditorState::new();
        assert!(!s.is_dirty());
        assert!(s.current_path.is_none());
        assert_eq!(s.title(), "未命名 - zdown");
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
        assert_eq!(s.current_path, Some(path.clone()));
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
        assert!(!s.is_dirty());
        assert!(s.current_path.is_none());
        assert_eq!(s.editor.to_string(), "");
    }

    #[test]
    fn edit_save_reopen_content_consistent() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("round.md");
        let mut s = EditorState::new();
        // 使用规范化形式的输入（to_markdown 输出规则：标题/段落间空行、文档以 \n 结尾），
        // 这样 round-trip 经过 parse + to_markdown 后内容保持一致。
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "# 标题\n\n段落\n".into(),
        })
        .expect("apply");
        s.save_as(&path).expect("save");

        let mut s2 = EditorState::new();
        s2.open(&path).expect("reopen");
        assert_eq!(s.editor.to_string(), s2.editor.to_string());
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
        assert_eq!(s.editor.to_string(), "");
        s.redo().expect("redo");
        assert_eq!(s.editor.to_string(), "abc");
    }

    #[test]
    fn quit_sets_should_exit() {
        let mut s = EditorState::new();
        assert!(!s.should_exit);
        s.quit();
        assert!(s.should_exit);
    }
}
