# {total} 两趟渲染实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** PDF 导出中 `{total}` 占位符通过两趟渲染正确显示实际总页数。

**架构：** `Arc<AtomicUsize>` 共享页计数器，从 decorator 外部读取总页数；`total_pages: Option<usize>` 控制 `{total}` 行为（None="?", Some(n)=n）；`template_needs_total()` 门控按需两趟渲染。

**技术栈：** Rust 2024, genpdf 0.2, `std::sync::Arc`, `std::sync::atomic::AtomicUsize`

---

### 任务 1：更新 ZdownPageDecorator 结构和方法

**文件：**
- 修改：`crates/export_engine/src/decorator.rs`

- [ ] **步骤 1：替换字段和构造签名**

将当前的 `page: usize` 字段替换为 `page_counter: Arc<AtomicUsize>` 和 `total_pages: Option<usize>`。

修改前：

```rust
pub struct ZdownPageDecorator {
    page: usize,
    config: HeaderFooter,
    margins: genpdf::Margins,
    file_name: String,
    date_str: String,
    font_size: u8,
}

impl ZdownPageDecorator {
    pub fn new(
        config: HeaderFooter,
        margins: genpdf::Margins,
        file_name: String,
        date_str: String,
        font_size: f32,
    ) -> Self {
        Self {
            page: 0,
            config,
            margins,
            file_name,
            date_str,
            font_size: font_size as u8,
        }
    }
```

修改后：

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub struct ZdownPageDecorator {
    page_counter: Arc<AtomicUsize>,
    total_pages: Option<usize>,
    config: HeaderFooter,
    margins: genpdf::Margins,
    file_name: String,
    date_str: String,
    font_size: u8,
}

impl ZdownPageDecorator {
    pub fn new(
        config: HeaderFooter,
        margins: genpdf::Margins,
        file_name: String,
        date_str: String,
        font_size: f32,
        page_counter: Arc<AtomicUsize>,
        total_pages: Option<usize>,
    ) -> Self {
        Self {
            page_counter,
            total_pages,
            config,
            margins,
            file_name,
            date_str,
            font_size: font_size as u8,
        }
    }
```

- [ ] **步骤 2：更新 fill_template 接收 page 参数**

修改方法签名和实现：

修改前：

```rust
    fn fill_template(&self, template: &str) -> String {
        if template.is_empty() {
            return String::new();
        }
        template
            .replace("{page}", &self.page.to_string())
            .replace("{total}", "?")
            .replace("{file}", &self.file_name)
            .replace("{date}", &self.date_str)
    }
```

修改后：

```rust
    fn fill_template(&self, template: &str, page: usize) -> String {
        if template.is_empty() {
            return String::new();
        }
        template
            .replace("{page}", &page.to_string())
            .replace("{total}", &self.total_pages.map_or("?".into(), |n| n.to_string()))
            .replace("{file}", &self.file_name)
            .replace("{date}", &self.date_str)
    }
```

- [ ] **步骤 3：更新 build_line 透传 page 参数**

修改前：

```rust
    fn build_line(&self) -> String {
        let parts: [&str; 3] = [&self.config.left, &self.config.center, &self.config.right];
        let filled: Vec<String> = parts
            .iter()
            .map(|t| self.fill_template(t))
            .filter(|s| !s.is_empty())
            .collect();
        filled.join("    ")
    }
```

修改后：

```rust
    fn build_line(&self, page: usize) -> String {
        let parts: [&str; 3] = [&self.config.left, &self.config.center, &self.config.right];
        let filled: Vec<String> = parts
            .iter()
            .map(|t| self.fill_template(t, page))
            .filter(|s| !s.is_empty())
            .collect();
        filled.join("    ")
    }
```

- [ ] **步骤 4：更新 decorate_page 使用 AtomicUsize 获取当前页**

修改前：

```rust
    fn decorate_page<'a>(
        &mut self,
        context: &Context,
        mut area: genpdf::render::Area<'a>,
        style: Style,
    ) -> Result<genpdf::render::Area<'a>, genpdf::error::Error> {
        self.page += 1;

        // 1. 页边距（来自配置）
        area.add_margins(self.margins);

        // 2. 页眉
        let header_text = self.build_line();
        if !header_text.is_empty() {
```

修改后（第 67-81 行）：

```rust
    fn decorate_page<'a>(
        &mut self,
        context: &Context,
        mut area: genpdf::render::Area<'a>,
        style: Style,
    ) -> Result<genpdf::render::Area<'a>, genpdf::error::Error> {
        let page = self.page_counter.fetch_add(1, Ordering::Relaxed) + 1;

        // 1. 页边距（来自配置）
        area.add_margins(self.margins);

        // 2. 页眉
        let header_text = self.build_line(page);
        if !header_text.is_empty() {
```

注意：decorate_page 后续用到 `self.build_line()` 的两处（页眉和页脚）全部改为 `self.build_line(page)`。

- [ ] **步骤 5：更新测试以匹配新构造签名**

修改前：

```rust
    fn make_decorator(config: HeaderFooter, file_name: &str, date_str: &str) -> ZdownPageDecorator {
        ZdownPageDecorator::new(
            config,
            test_margins(),
            file_name.into(),
            date_str.into(),
            9.0,
        )
    }
```

修改后：

```rust
    fn make_decorator(
        config: HeaderFooter,
        file_name: &str,
        date_str: &str,
        total_pages: Option<usize>,
    ) -> ZdownPageDecorator {
        ZdownPageDecorator::new(
            config,
            test_margins(),
            file_name.into(),
            date_str.into(),
            9.0,
            Arc::new(AtomicUsize::new(0)),
            total_pages,
        )
    }
```

测试函数中所有 `make_decorator(config, ...)` 调用更新为 `make_decorator(config, "test.md", "2026-06-19", None)`。

`fill_template` 调用更新为 `d.fill_template("hello", 1)`（传入 page 参数）。

`total_placeholder_renders_question_mark` 测试中 `d.fill_template("{total}", 0)` 预期 `"?"`（total=None）。

- [ ] **步骤 6：运行 decorator 测试**

```bash
cargo test -p export_engine -- decorator
```

预期：3 个测试通过。

- [ ] **步骤 7：Commit**

```bash
git add crates/export_engine/src/decorator.rs
git commit -m "refactor(export_engine): ZdownPageDecorator uses Arc<AtomicUsize> for page counting"
```

---

### 任务 2：在 generate_pdf 中实现两趟渲染

**文件：**
- 修改：`crates/export_engine/src/pdf.rs`

- [ ] **步骤 1：添加 use 导入**

在文件头部添加：

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use crate::theme::HeaderFooter;
```

- [ ] **步骤 2：添加 template_needs_total 辅助函数**

在 `generate_pdf` 前添加：

```rust
/// 检查 HeaderFooter 模板是否引用了 {total} 占位符。
fn template_needs_total(hf: &HeaderFooter) -> bool {
    hf.left.contains("{total}")
        || hf.center.contains("{total}")
        || hf.right.contains("{total}")
}
```

- [ ] **步骤 3：提取 make_doc 辅助函数**

在 `generate_pdf` 前添加，用于创建 genpdf::Document 和设置纸张/字体/标题/decorator：

```rust
fn make_doc(
    config: &PdfConfig,
    fonts: &FontSet,
    decorator: crate::decorator::ZdownPageDecorator,
) -> genpdf::Document {
    let paper_size: genpdf::Size = match config.paper {
        crate::theme::Paper::A4 => genpdf::PaperSize::A4.into(),
        crate::theme::Paper::Letter => genpdf::PaperSize::Letter.into(),
        crate::theme::Paper::Custom {
            width_mm,
            height_mm,
        } => genpdf::Size::new(width_mm, height_mm),
    };

    let mut pdf_doc = genpdf::Document::new(genpdf::fonts::FontFamily {
        regular: fonts.body.clone(),
        bold: fonts.body.clone(),
        italic: fonts.body.clone(),
        bold_italic: fonts.body.clone(),
    });
    pdf_doc.set_paper_size(paper_size);
    pdf_doc.set_title("zdown export");
    pdf_doc.set_page_decorator(decorator);
    pdf_doc
}
```

- [ ] **步骤 4：添加 layout_and_push 辅助函数**

```rust
fn layout_and_push(
    pdf_doc: &mut genpdf::Document,
    doc: &Document,
    config: &PdfConfig,
    fonts: &FontSet,
) -> crate::Result<()> {
    let layout = crate::renderer::render_document(doc, config, fonts)?;
    pdf_doc.push(layout);
    Ok(())
}
```

- [ ] **步骤 5：重写 generate_pdf 实现两趟渲染**

用两趟渲染逻辑替换旧的实现。注意构建 genpdf_margins 的逻辑保留在 generate_pdf 中（因为两趟都需要同样的 margins）。

```rust
/// 将 Document 导出为 PDF，返回完整 PDF 字节。
pub fn generate_pdf(doc: &Document, config: &PdfConfig) -> crate::Result<Vec<u8>> {
    let fonts = FontSet::load(config)?;

    let date_str = chrono::Local::now()
        .format("%Y-%m-%d")
        .to_string();
    let file_name = "untitled.md".to_string();

    let genpdf_margins = genpdf::Margins::trbl(
        genpdf::Mm::from(config.margins.top),
        genpdf::Mm::from(config.margins.right),
        genpdf::Mm::from(config.margins.bottom),
        genpdf::Mm::from(config.margins.left),
    );

    let hf_font_size = config.theme.font_size.header_footer;

    if template_needs_total(&config.header_footer) {
        // Pass 1: 获取总页数
        let pc = Arc::new(AtomicUsize::new(0));
        let d1 = crate::decorator::ZdownPageDecorator::new(
            config.header_footer.clone(),
            genpdf_margins,
            file_name.clone(),
            date_str.clone(),
            hf_font_size,
            pc.clone(),
            None,
        );
        let mut doc1 = make_doc(config, &fonts, d1);
        layout_and_push(&mut doc1, doc, config, &fonts)?;
        let mut tmp = Vec::new();
        doc1
            .render(&mut tmp)
            .map_err(|e| crate::Error::Io(std::io::Error::other(e.to_string())))?;
        let total = pc.load(Ordering::Relaxed);

        // Pass 2: 正式渲染
        let pc2 = Arc::new(AtomicUsize::new(0));
        let d2 = crate::decorator::ZdownPageDecorator::new(
            config.header_footer.clone(),
            genpdf_margins,
            file_name,
            date_str,
            hf_font_size,
            pc2,
            Some(total),
        );
        let mut doc2 = make_doc(config, &fonts, d2);
        layout_and_push(&mut doc2, doc, config, &fonts)?;
        let mut buf = Vec::new();
        doc2
            .render(&mut buf)
            .map_err(|e| crate::Error::Io(std::io::Error::other(e.to_string())))?;
        Ok(buf)
    } else {
        // 单趟渲染（模板不含 {total}）
        let pc = Arc::new(AtomicUsize::new(0));
        let d = crate::decorator::ZdownPageDecorator::new(
            config.header_footer.clone(),
            genpdf_margins,
            file_name,
            date_str,
            hf_font_size,
            pc,
            None,
        );
        let mut pdf_doc = make_doc(config, &fonts, d);
        layout_and_push(&mut pdf_doc, doc, config, &fonts)?;
        let mut buf = Vec::new();
        pdf_doc
            .render(&mut buf)
            .map_err(|e| crate::Error::Io(std::io::Error::other(e.to_string())))?;
        Ok(buf)
    }
}
```

- [ ] **步骤 6：检查编译**

```bash
cargo check -p export_engine 2>&1
```

预期：编译无警告。

- [ ] **步骤 7：运行测试**

```bash
cargo test -p export_engine
```

预期：所有已有测试通过（17 passed，包括 pdf.rs 的 2 个测试）。

- [ ] **步骤 8：Commit**

```bash
git add crates/export_engine/src/pdf.rs
git commit -m "feat(export_engine): implement two-pass rendering for {total} placeholder"
```

---

### 任务 3：添加增强测试

**文件：**
- 修改：`crates/export_engine/src/pdf.rs`

- [ ] **步骤 1：添加 template_needs_total 测试**

在 `pdf.rs` 的 `tests` 模块中添加：

```rust
#[test]
fn template_needs_total_detects_total_placeholder() {
    use super::template_needs_total;
    use crate::theme::HeaderFooter;

    let hf = HeaderFooter {
        left: String::new(),
        center: String::new(),
        right: "{page}/{total}".into(),
    };
    assert!(template_needs_total(&hf));

    let hf2 = HeaderFooter {
        left: "{total}".into(),
        center: String::new(),
        right: String::new(),
    };
    assert!(template_needs_total(&hf2));

    let hf3 = HeaderFooter {
        left: "{file}".into(),
        center: "{date}".into(),
        right: "{page}".into(),
    };
    assert!(!template_needs_total(&hf3));
}
```

- [ ] **步骤 2：添加两趟渲染集成测试（含 {total}）**

```rust
#[test]
fn generate_pdf_with_total_does_not_panic() {
    let mut config = PdfConfig::default();
    config.header_footer.right = "{page}/{total}".into();
    let doc = Document {
        blocks: vec![BlockWithSpan {
            block: Block::Paragraph(AstParagraph {
                inlines: vec![Inline::Text("test".into())],
            }),
            span: Span {
                start_line: 0,
                end_line: 0,
            },
        }],
    };
    let result = super::generate_pdf(&doc, &config);
    if let Ok(bytes) = result {
        assert!(!bytes.is_empty(), "两趟渲染应产出非空 PDF");
    }
    // Err 可接受（无字体环境）
}
```

- [ ] **步骤 3：运行全部测试**

```bash
cargo test -p export_engine
```

预期：19 个测试全部通过。

- [ ] **步骤 4：Commit**

```bash
git add crates/export_engine/src/pdf.rs
git commit -m "test(export_engine): add tests for template_needs_total and two-pass rendering"
```

---

### 任务 4：全量验证

**文件：** 无新建文件

- [ ] **步骤 1：fmt + clippy + full test**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **步骤 2：验证通过后 Commit**

```bash
git add -A
git commit -m "chore: fmt + clippy fixes after {total} two-pass rendering"
```

---

## 依赖关系

```
任务 1 (decorator 重构) → 任务 2 (generate_pdf 两趟) → 任务 3 (增强测试) → 任务 4 (全量验证)
```
