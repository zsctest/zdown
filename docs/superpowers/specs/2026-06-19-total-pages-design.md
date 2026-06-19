# {total} 两趟渲染设计规格

日期: 2026-06-19
状态: 已批准

---

## 目标

PDF 导出中 `{total}` 占位符当前渲染为 `"?"`。通过两趟渲染技术获取总页数，使 `{total}` 正确显示实际页数。

---

## 架构

### 核心思路

genpdf `Document::render()` 消费 self，无法重入同一个 Document。采用两趟渲染：
1. **Pass 1**：渲染到临时 `Vec<u8>`（丢弃），通过 `Arc<AtomicUsize>` 共享计数器获取总页数
2. **Pass 2**：用正确的 `{total}` 值创建新 Document，正式渲染到输出

**性能优化**：如果模板不含 `{total}`，直接单趟渲染，零开销。

### 文件变更清单

```
crates/export_engine/src/
├── decorator.rs       # [修改] page 字段改为 Arc<AtomicUsize> + total_pages: Option<usize>
├── pdf.rs             # [修改] generate_pdf 增加两趟渲染分支
```

---

## 详细设计

### ZdownPageDecorator 变更

```rust
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

pub struct ZdownPageDecorator {
    page_counter: Arc<AtomicUsize>,   // 共享页计数，外部可读取总页数
    total_pages: Option<usize>,       // None = Pass 1 ({total}→"?"), Some(n) = Pass 2
    config: HeaderFooter,
    margins: genpdf::Margins,
    file_name: String,
    date_str: String,
    font_size: u8,
}
```

**构造签名变更**：

```rust
pub fn new(
    config: HeaderFooter,
    margins: genpdf::Margins,
    file_name: String,
    date_str: String,
    font_size: f32,
    page_counter: Arc<AtomicUsize>,
    total_pages: Option<usize>,
) -> Self
```

**decorate_page 变更**：

```rust
fn decorate_page<'a>(...) -> ... {
    let page = self.page_counter.fetch_add(1, Ordering::Relaxed) + 1;
    // page 用于 fill_template 中的 {page} 替换
    // 不再使用 self.page 字段
}
```

**fill_template 变更**：

```rust
fn fill_template(&self, template: &str, page: usize) -> String {
    template
        .replace("{page}", &page.to_string())
        .replace("{total}", &self.total_pages.map_or("?".into(), |n| n.to_string()))
        .replace("{file}", &self.file_name)
        .replace("{date}", &self.date_str)
}
```

`fill_template` 现在接收 `page` 参数（来自 AtomicUsize 的最新值），而非读取 `self.page`。

**build_line 变更**：透传 `page` 参数。

### generate_pdf 变更

提取辅助函数 `build_render_document`（字体/纸张/Document 构造），两趟共享。

```rust
pub fn generate_pdf(doc: &Document, config: &PdfConfig) -> Result<Vec<u8>> {
    let fonts = FontSet::load(config)?;
    let needs_total = template_needs_total(&config.header_footer);

    if needs_total {
        // Pass 1: 获取总页数
        let pc = Arc::new(AtomicUsize::new(0));
        let d1 = ZdownPageDecorator::new(..., pc.clone(), None);
        let mut doc1 = make_doc(config, &fonts, d1)?;
        layout_and_push(&mut doc1, doc, config, &fonts)?;
        let mut tmp = Vec::new();
        doc1.render(&mut tmp)?;
        let total = pc.load(Ordering::Relaxed);

        // Pass 2: 正式渲染
        let pc2 = Arc::new(AtomicUsize::new(0));
        let d2 = ZdownPageDecorator::new(..., pc2.clone(), Some(total));
        let mut doc2 = make_doc(config, &fonts, d2)?;
        layout_and_push(&mut doc2, doc, config, &fonts)?;
        let mut buf = Vec::new();
        doc2.render(&mut buf)?;
        Ok(buf)
    } else {
        // 单趟
        let pc = Arc::new(AtomicUsize::new(0));
        let d = ZdownPageDecorator::new(..., pc, None);
        let mut pdf_doc = make_doc(config, &fonts, d)?;
        layout_and_push(&mut pdf_doc, doc, config, &fonts)?;
        let mut buf = Vec::new();
        pdf_doc.render(&mut buf)?;
        Ok(buf)
    }
}
```

### 模板检测

```rust
fn template_needs_total(hf: &HeaderFooter) -> bool {
    hf.left.contains("{total}")
        || hf.center.contains("{total}")
        || hf.right.contains("{total}")
}
```

---

## 错误处理

两趟渲染均可能失败（字体失效、IO 错误）。Pass 1 失败时错误直接返回，不进入 Pass 2。

---

## 测试计划

1. **单趟渲染（无 {total}）**：输出正确，单趟执行
2. **两趟渲染（含 {total}）**：`{total}` 替换为正确页数
3. **fill_template 单元测试**：验证 `total=Some(5)` 时 `{total}` → `"5"`，`total=None` 时 → `"?"`
4. **空文档两趟渲染**：0 个 block，PDF 最小有效输出
5. **template_needs_total 测试**：各种模板组合正确判断

---

## 不在范围内

- 超过 999 页的场景（`AtomicUsize` 计数足够）
- 并发 PDF 生成（`Arc` 支持但当前单线程使用）
- mermaid 图表渲染
- 图片嵌入

---

## 依赖

- `std::sync::Arc`
- `std::sync::atomic::AtomicUsize`
- 无新增外部依赖
