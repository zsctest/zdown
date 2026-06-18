//! 阶段 1 完整链路集成测试：编辑→保存→重开内容一致。
//!
//! 覆盖 workspace + document_model + editor_engine 协作：
//! 1. 打开文件 → Editor 编辑 → 保存 → 重开 → 内容一致
//! 2. 新建 → 编辑 → 另存为 → 重开 → 内容一致
//! 3. 编辑 → 撤销 → 保存 → 重开 → 内容为撤销后状态

#![allow(clippy::expect_used, clippy::unwrap_used, unused_imports)]

use tempfile::TempDir;

use document_model::{parse, to_markdown};
use editor_engine::{Command, Cursor, Editor};
use workspace::Workspace;

#[test]
fn edit_save_reopen_content_consistent() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("edit_save_reopen.md");

    // 1. 初始写入文件
    let initial = "# 原始标题\n\n原始段落\n";
    std::fs::write(&path, initial).expect("write");

    // 2. 打开文件 → Editor
    let mut ws = Workspace::new();
    let doc = ws.open(&path).expect("open");
    let src = to_markdown(&doc);
    let mut editor = Editor::new(&src);
    editor.mark_saved();

    // 3. 编辑：在标题后插入文本
    editor
        .apply(Command::Insert {
            pos: Cursor::new(0, 5),
            text: "（已编辑）".into(),
        })
        .expect("apply insert");
    assert!(editor.is_dirty());

    // 4. 保存
    let edited_doc = parse(&editor.to_string()).expect("parse edited");
    ws.save(&edited_doc).expect("save");

    // 5. 重开
    let mut ws2 = Workspace::new();
    let reopened = ws2.open(&path).expect("reopen");

    // 6. 内容一致（重开后的 Document 应与保存前的一致）
    assert_eq!(reopened, edited_doc);

    // 7. 文件内容包含编辑
    let file_content = std::fs::read_to_string(&path).expect("read");
    assert!(
        file_content.contains("（已编辑）"),
        "文件应含编辑内容: {file_content}"
    );
}

#[test]
fn new_edit_save_as_reopen_consistent() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("new_save_as.md");

    // 1. 新建空 Editor
    let mut editor = Editor::empty();
    editor.mark_saved();

    // 2. 编辑：插入完整文档
    editor
        .apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "# 新文档\n\n内容\n".into(),
        })
        .expect("apply");
    assert!(editor.is_dirty());

    // 3. 另存为
    let doc = parse(&editor.to_string()).expect("parse");
    let mut ws = Workspace::new();
    ws.save_as(&path, &doc).expect("save_as");

    // 4. 重开
    let mut ws2 = Workspace::new();
    let reopened = ws2.open(&path).expect("reopen");

    // 5. 内容一致
    assert_eq!(reopened, doc);
}

#[test]
fn edit_undo_save_reopen_undone_state() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("undo_save.md");

    // 1. 初始文件
    std::fs::write(&path, "原始\n").expect("write");

    // 2. 打开 + 编辑
    let mut ws = Workspace::new();
    let doc = ws.open(&path).expect("open");
    let mut editor = Editor::new(&to_markdown(&doc));
    editor.mark_saved();

    editor
        .apply(Command::Insert {
            pos: Cursor::new(0, 1),
            text: "X".into(),
        })
        .expect("apply");
    assert_eq!(editor.to_string(), "原X始\n");

    // 3. 撤销
    editor.undo().expect("undo");
    assert_eq!(editor.to_string(), "原始\n");

    // 4. 保存（撤销后状态）
    let undone_doc = parse(&editor.to_string()).expect("parse");
    ws.save(&undone_doc).expect("save");

    // 5. 重开应是撤销后状态
    let mut ws2 = Workspace::new();
    let reopened = ws2.open(&path).expect("reopen");
    assert_eq!(reopened, undone_doc);

    let file_content = std::fs::read_to_string(&path).expect("read");
    assert_eq!(file_content, "原始\n");
}

#[test]
fn edit_redo_save_reopen_redone_state() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("redo_save.md");

    std::fs::write(&path, "abc\n").expect("write");

    let mut ws = Workspace::new();
    let doc = ws.open(&path).expect("open");
    let mut editor = Editor::new(&to_markdown(&doc));
    editor.mark_saved();

    // 编辑 + 撤销 + 重做
    editor
        .apply(Command::Insert {
            pos: Cursor::new(0, 3),
            text: "def".into(),
        })
        .expect("apply");
    editor.undo().expect("undo");
    editor.redo().expect("redo");
    assert_eq!(editor.to_string(), "abcdef\n");

    // 保存重开
    let redone_doc = parse(&editor.to_string()).expect("parse");
    ws.save(&redone_doc).expect("save");

    let mut ws2 = Workspace::new();
    let reopened = ws2.open(&path).expect("reopen");
    assert_eq!(reopened, redone_doc);
}

#[test]
fn multiple_edits_save_reopen() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("multi_edits.md");

    let mut editor = Editor::empty();
    editor.mark_saved();

    // 多步编辑
    for (i, ch) in ['a', 'b', 'c', 'd', 'e'].iter().enumerate() {
        editor
            .apply(Command::Insert {
                pos: Cursor::new(0, i),
                text: ch.to_string(),
            })
            .expect("apply");
    }
    assert_eq!(editor.to_string(), "abcde");

    let doc = parse(&editor.to_string()).expect("parse");
    let mut ws = Workspace::new();
    ws.save_as(&path, &doc).expect("save_as");

    let mut ws2 = Workspace::new();
    let reopened = ws2.open(&path).expect("reopen");
    assert_eq!(reopened, doc);
}

#[test]
fn delete_save_reopen() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("delete_save.md");

    std::fs::write(&path, "hello world\n").expect("write");

    let mut ws = Workspace::new();
    let doc = ws.open(&path).expect("open");
    let mut editor = Editor::new(&to_markdown(&doc));
    editor.mark_saved();

    // 删除 " world"
    editor
        .apply(Command::Delete {
            range: editor_engine::Selection::new(Cursor::new(0, 5), Cursor::new(0, 11)),
        })
        .expect("apply delete");
    assert_eq!(editor.to_string(), "hello\n");

    let deleted_doc = parse(&editor.to_string()).expect("parse");
    ws.save(&deleted_doc).expect("save");

    let mut ws2 = Workspace::new();
    let reopened = ws2.open(&path).expect("reopen");
    assert_eq!(reopened, deleted_doc);
}

#[test]
fn replace_save_reopen() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("replace_save.md");

    std::fs::write(&path, "hello world\n").expect("write");

    let mut ws = Workspace::new();
    let doc = ws.open(&path).expect("open");
    let mut editor = Editor::new(&to_markdown(&doc));
    editor.mark_saved();

    // 替换 "hello" 为 "hi"
    editor
        .apply(Command::Replace {
            range: editor_engine::Selection::new(Cursor::new(0, 0), Cursor::new(0, 5)),
            text: "hi".into(),
        })
        .expect("apply replace");
    assert_eq!(editor.to_string(), "hi world\n");

    let replaced_doc = parse(&editor.to_string()).expect("parse");
    ws.save(&replaced_doc).expect("save");

    let mut ws2 = Workspace::new();
    let reopened = ws2.open(&path).expect("reopen");
    assert_eq!(reopened, replaced_doc);
}

#[test]
fn save_mark_saved_clears_dirty() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("dirty_clear.md");

    std::fs::write(&path, "x\n").expect("write");

    let mut ws = Workspace::new();
    let doc = ws.open(&path).expect("open");
    let mut editor = Editor::new(&to_markdown(&doc));
    editor.mark_saved();
    assert!(!editor.is_dirty());

    editor
        .apply(Command::Insert {
            pos: Cursor::new(0, 1),
            text: "y".into(),
        })
        .expect("apply");
    assert!(editor.is_dirty());

    // 保存后 EditorState 层应 mark_saved（模拟）
    let doc = parse(&editor.to_string()).expect("parse");
    ws.save(&doc).expect("save");
    editor.mark_saved();
    assert!(!editor.is_dirty());
}
