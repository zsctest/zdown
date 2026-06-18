//! workspace 集成测试：Workspace + RecentFiles 协作。

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use tempfile::TempDir;
use workspace::{RecentFiles, Workspace};

use document_model::Document;
use document_model::ast::*;

fn bws(block: Block) -> BlockWithSpan {
    BlockWithSpan {
        block,
        span: Span {
            start_line: 0,
            end_line: 0,
        },
    }
}

fn sample_doc() -> Document {
    Document {
        blocks: vec![
            bws(Block::Heading(Heading {
                level: 1,
                inlines: vec![Inline::Text("标题".into())],
            })),
            bws(Block::Paragraph(Paragraph {
                inlines: vec![Inline::Text("内容".into())],
            })),
        ],
    }
}

#[test]
fn save_adds_to_recent_and_reopens() {
    let dir = TempDir::new().expect("tempdir");
    let storage = dir.path().join("recent.toml");
    let file_path = dir.path().join("doc.md");

    // 1. save_as 写文件
    let mut ws = Workspace::new();
    let doc = sample_doc();
    ws.save_as(&file_path, &doc).expect("save_as");

    // 2. 加入最近文件
    let mut recent = RecentFiles::load_from(&storage);
    recent.add(file_path.clone());
    recent.save().expect("save recent");

    // 3. 从最近文件加载并重新打开
    let reloaded_recent = RecentFiles::load_from(&storage);
    assert_eq!(reloaded_recent.list(), std::slice::from_ref(&file_path));

    let mut ws2 = Workspace::new();
    let reopened = ws2.open(&reloaded_recent.list()[0]).expect("reopen");

    // 4. 内容一致
    assert_eq!(reopened, doc);
}

#[test]
fn multiple_files_recent_order() {
    let dir = TempDir::new().expect("tempdir");
    let storage = dir.path().join("recent.toml");
    let mut recent = RecentFiles::load_from(&storage);

    for name in ["a.md", "b.md", "c.md"] {
        let path = dir.path().join(name);
        let mut ws = Workspace::new();
        ws.save_as(&path, &sample_doc()).expect("save");
        recent.add(path);
    }
    recent.save().expect("save");

    let reloaded = RecentFiles::load_from(&storage);
    // c.md 最后添加应在队首
    assert_eq!(reloaded.list()[0], dir.path().join("c.md"));
    assert_eq!(reloaded.list()[1], dir.path().join("b.md"));
    assert_eq!(reloaded.list()[2], dir.path().join("a.md"));
}

#[test]
fn workspace_current_path_tracks_save_as() {
    let dir = TempDir::new().expect("tempdir");
    let p1 = dir.path().join("a.md");
    let p2 = dir.path().join("b.md");

    let mut ws = Workspace::new();
    assert!(ws.current_path().is_none());

    ws.save_as(&p1, &sample_doc()).expect("save 1");
    assert_eq!(ws.current_path(), Some(p1.as_path()));

    ws.save_as(&p2, &sample_doc()).expect("save 2");
    assert_eq!(ws.current_path(), Some(p2.as_path()));
}

#[test]
fn open_nonexistent_does_not_set_path() {
    let mut ws = Workspace::new();
    let _ = ws.open(std::path::Path::new("/nonexistent/xyz.md"));
    assert!(ws.current_path().is_none());
}

#[test]
fn save_after_open_persists_changes() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("doc.md");

    // 初始保存
    let mut ws = Workspace::new();
    let doc1 = sample_doc();
    ws.save_as(&path, &doc1).expect("save");

    // 修改后保存
    let doc2 = Document {
        blocks: vec![bws(Block::Paragraph(Paragraph {
            inlines: vec![Inline::Text("modified".into())],
        }))],
    };
    ws.save(&doc2).expect("save again");

    // 重开应是 doc2
    let mut ws2 = Workspace::new();
    let reopened = ws2.open(&path).expect("reopen");
    assert_eq!(reopened, doc2);
}

#[test]
fn pathbuf_to_string_lossy_roundtrip() {
    // 验证 to_string_lossy 在 RecentFiles save/load 往返中无损
    let dir = TempDir::new().expect("tempdir");
    let storage = dir.path().join("recent.toml");
    let mut recent = RecentFiles::load_from(&storage);
    let p = PathBuf::from("/tmp/含中文.md");
    recent.add(p.clone());
    recent.save().expect("save");

    let reloaded = RecentFiles::load_from(&storage);
    assert_eq!(reloaded.list(), &[p]);
}
