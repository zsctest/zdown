//! Workspace：文件读写，持有当前路径。

use std::fs;
use std::path::{Path, PathBuf};

use document_model::{Document, parse, to_markdown};

use crate::{Error, Result};

/// 文件工作区。持有当前打开文件的路径。
pub struct Workspace {
    current_path: Option<PathBuf>,
}

impl Workspace {
    /// 空工作区（无当前路径）。
    pub fn new() -> Self {
        Self { current_path: None }
    }

    /// 打开指定路径文件，解析为 `Document`，并记录为当前路径。
    pub fn open(&mut self, path: &Path) -> Result<Document> {
        let src = fs::read_to_string(path)?;
        let doc = parse(&src)?;
        self.current_path = Some(path.to_path_buf());
        Ok(doc)
    }

    /// 保存到当前路径。无当前路径返回 `Err`。
    pub fn save(&self, doc: &Document) -> Result<()> {
        let path = self.current_path.as_ref().ok_or(Error::NoCurrentPath)?;
        let md = to_markdown(doc);
        fs::write(path, md)?;
        Ok(())
    }

    /// 保存到指定路径（不更新内部 current_path）。
    /// 用于多标签页场景，每个标签页独立管理自己的路径。
    pub fn save_to(&self, path: &Path, doc: &Document) -> Result<()> {
        let md = to_markdown(doc);
        fs::write(path, md)?;
        Ok(())
    }

    /// 另存为指定路径，并更新当前路径。
    pub fn save_as(&mut self, path: &Path, doc: &Document) -> Result<()> {
        let md = to_markdown(doc);
        fs::write(path, md)?;
        self.current_path = Some(path.to_path_buf());
        Ok(())
    }

    /// 当前路径。
    pub fn current_path(&self) -> Option<&Path> {
        self.current_path.as_deref()
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use document_model::ast::{Block, BlockWithSpan, Document, Inline, Paragraph, Span};

    fn bws(block: Block) -> BlockWithSpan {
        BlockWithSpan {
            block,
            span: Span {
                start_line: 0,
                end_line: 0,
            },
        }
    }
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn sample_doc() -> Document {
        Document {
            blocks: vec![bws(Block::Paragraph(Paragraph {
                inlines: vec![Inline::Text("hello".into())],
            }))],
        }
    }

    fn write_temp(content: &str) -> (NamedTempFile, PathBuf) {
        let mut f = NamedTempFile::new().expect("tempfile");
        f.write_all(content.as_bytes()).expect("write");
        let path = f.path().to_path_buf();
        (f, path)
    }

    #[test]
    fn new_workspace_has_no_path() {
        let ws = Workspace::new();
        assert!(ws.current_path().is_none());
    }

    #[test]
    fn open_reads_and_parses() {
        let (_f, path) = write_temp("# 标题\n");
        let mut ws = Workspace::new();
        let doc = ws.open(&path).expect("open");
        assert_eq!(ws.current_path(), Some(path.as_path()));
        assert_eq!(doc.blocks.len(), 1);
        assert!(matches!(
            &doc.blocks[0],
            BlockWithSpan {
                block: Block::Heading(_),
                ..
            }
        ));
    }

    #[test]
    fn open_switches_current_path() {
        let (_f1, path1) = write_temp("# a\n");
        let (_f2, path2) = write_temp("# b\n");
        let mut ws = Workspace::new();
        ws.open(&path1).expect("open 1");
        assert_eq!(ws.current_path(), Some(path1.as_path()));
        ws.open(&path2).expect("open 2");
        assert_eq!(ws.current_path(), Some(path2.as_path()));
    }

    #[test]
    fn open_nonexistent_returns_io_error() {
        let mut ws = Workspace::new();
        let err = ws.open(Path::new("/nonexistent/path/xyz.md")).unwrap_err();
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn save_without_path_returns_no_current_path() {
        let ws = Workspace::new();
        let doc = sample_doc();
        let err = ws.save(&doc).unwrap_err();
        assert!(matches!(err, Error::NoCurrentPath));
    }

    #[test]
    fn save_after_open_writes() {
        let (_f, path) = write_temp("old");
        let mut ws = Workspace::new();
        ws.open(&path).expect("open");
        let doc = sample_doc();
        ws.save(&doc).expect("save");
        let content = std::fs::read_to_string(&path).expect("read");
        assert_eq!(content, "hello\n");
    }

    #[test]
    fn save_as_updates_path() {
        let mut ws = Workspace::new();
        let doc = sample_doc();
        let (_f, new_path) = write_temp("");
        ws.save_as(&new_path, &doc).expect("save_as");
        assert_eq!(ws.current_path(), Some(new_path.as_path()));
        let content = std::fs::read_to_string(&new_path).expect("read");
        assert_eq!(content, "hello\n");
    }

    #[test]
    fn open_save_reopen_roundtrip() {
        let (_f, path) = write_temp("");
        let mut ws = Workspace::new();
        let doc = sample_doc();
        ws.save_as(&path, &doc).expect("save_as");
        let reopened = ws.open(&path).expect("reopen");
        assert_eq!(reopened, doc);
    }

    #[test]
    fn save_to_writes_file_without_updating_current_path() {
        let ws = Workspace::new();
        let doc = sample_doc();
        let (_f, path) = write_temp("");
        ws.save_to(&path, &doc).expect("save_to");
        let content = std::fs::read_to_string(&path).expect("read");
        assert_eq!(content, "hello\n");
        // current_path remains None — save_to doesn't change it
        assert!(ws.current_path().is_none());
    }

    #[test]
    fn save_as_to_new_path_switches() {
        let (_f, path1) = write_temp("a");
        let (_f2, path2) = write_temp("b");
        let mut ws = Workspace::new();
        ws.open(&path1).expect("open 1");
        assert_eq!(ws.current_path(), Some(path1.as_path()));
        let doc = sample_doc();
        ws.save_as(&path2, &doc).expect("save_as");
        assert_eq!(ws.current_path(), Some(path2.as_path()));
        assert_eq!(std::fs::read_to_string(&path1).expect("read 1"), "a");
    }
}
