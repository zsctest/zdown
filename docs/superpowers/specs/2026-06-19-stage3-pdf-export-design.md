# 阶段 3：PDF 导出设计规格

> 状态：草稿 | 2026-06-19

## 目标

为 zdown 实现 Markdown → PDF 导出功能。用纯 Rust 方案（`genpdf`），不经过 HTML 中转。

## 架构

```
crates/export_engine/
├── Cargo.toml           + genpdf
├── src/
│   ├── lib.rs           模块声明 + re-export
│   ├── error.rs         错误类型（已有，扩展）
│   ├── pdf.rs           入口：generate_pdf(doc, config) -> Result<Vec<u8>>
│   ├── renderer.rs      AST → genpdf 元素分发
│   ├── theme.rs         主题配置（字体/颜色/间距/纸张/页眉页脚）
│   └── font.rs          字体加载（内嵌后备 + 系统 fallback）
```

**核心流程：** `Document AST → renderer.rs → genpdf 元素 → genpdf::Document::render() → Vec<u8>`

## 对外接口

```rust
pub fn generate_pdf(doc: &Document, config: &PdfConfig) -> Result<Vec<u8>>;
```

`PdfConfig` 提供 3 个 preset：
- `PdfConfig::default()` — 内嵌 Noto Sans CJK SC，A4，浅色主题
- `PdfConfig::dark()` — 暗色背景主题
- `PdfConfig::minimal()` — 极简，系统字体，省墨

## 主题配置（theme.rs）

```rust
pub struct PdfConfig {
    pub paper: Paper,              // A4 | Letter | Custom
    pub margins: Margins,          // top/bottom/left/right (mm)
    pub header_footer: HeaderFooter, // {file} {date} {page}/{total} 模板
    pub theme: PdfTheme,
}

pub struct PdfTheme {
    pub body_font: FontConfig,     // 正文：Noto Sans CJK SC
    pub mono_font: FontConfig,     // 等宽：Noto Sans Mono CJK SC
    pub heading_font: FontConfig,  // 标题：同 body 字体
    pub font_size: FontSizes,      // 11/20-11/9/9pt 分级
    pub colors: ThemeColors,       // 文字/标题/代码背景/表格线/引用线
    pub spacing: ThemeSpacing,     // 行高1.4/段间距6pt/列表缩进20pt/单元格4pt
}

pub struct FontConfig {
    pub name: String,              // 字体名
    pub ttf_data: Option<Vec<u8>>, // 内嵌 TTF，None 从系统加载
}
```

## 渲染映射（renderer.rs）

**Block → PDF：**

| Block | PDF 实现 | 细节 |
|-------|----------|------|
| Heading | Paragraph + font_size(hN) | H1=20pt, H2=18pt, H3=16pt, H4=14pt, H5=12pt, H6=11pt |
| Paragraph | 逐 Inline → StyledString | emph=斜体, strong=粗体, code=等宽+灰底 |
| CodeBlock | FramedElement + 浅灰背景 | 等宽字体，不做语法高亮 |
| List | 递归缩进 + marker | 有序"1." / 无序"•"，缩进 20pt/级 |
| BlockQuote | 左边框线 + 缩进 | 左边 4pt 线，缩进 8pt |
| Table | genpdf Table widget | 表头加粗，列对齐，padding 4pt |
| ThematicBreak | 水平线 | |
| HtmlBlock | 忽略 | PDF 不渲染 HTML |

**Inline → StyledString：**  
Text→正文；Emph→斜体；Strong→粗体；Code→等宽+灰底；Link→蓝色+下划线；Image→占位 `[图片: alt]`；SoftBreak→空格；HardBreak→换行。

**分页：** genpdf 自动处理。文本过长自动断行，表格自动跨页。

## 字体加载（font.rs）

策略：`ttf_data` 有值→内存加载 → `FontData::new()`。无值→`font-kit` 系统查找。失败→回退到编译期内嵌 Noto Sans CJK SC 子集（约 2MB）。全失败则返回 `Error::FontLoad`，上层提示用户。

`FontSet { body, mono, heading, header_footer }` — 一次加载，renderer 各处复用。

## 错误处理

扩展 `export_engine::Error`：

```rust
pub enum Error {
    Io(std::io::Error),
    FontLoad(String),
    Render(String),     // 渲染逻辑内部错误
}
```

## 测试

- **单元测试**：`renderer.rs` 中每个 Block 类型渲染不 panic + 产出预期 genpdf Element
- **快照测试**：`generate_pdf()` 输出非空 Vec<u8>，用 `pdf` crate 读取验证结构
- **往返测试**：不适用（PDF 是单向导出）

## 不在范围内

- HTML 导出（独立规划）
- 图片渲染为实际图片（PDF 中仅占位文本，阶段 4 图床后扩展）
- CSS 自定义（PdfTheme 足够覆盖初始需求）
- 语法高亮（PDF 代码块仅等宽+背景，不做 syntect 颜色）

## 依赖新增

- `genpdf` — PDF 生成
- `font-kit` — 系统字体查找

## 完成标准

- `cargo test --workspace` 全通过
- `cargo clippy --workspace -- -D warnings` 无警告
- 手动：打开 zdown，编辑文档，导出为 PDF，PDF 中标题/段落/代码块/表格/列表正确渲染
