# PDF 增强：字体嵌入 / 语法高亮 / 页眉页脚

日期: 2026-06-19
状态: 已批准

---

## 目标

在阶段 3 PDF 导出基础上增强三项能力：

1. **嵌入字体文件** — `crates/export_engine/fonts/` 放入子集 TTF，编译期嵌入 PDF
2. **代码块语法高亮** — 复用 syntect（workspace 已有依赖）
3. **页眉页脚页码** — 自定义 `PageDecorator` 实现 `{page}/{total}` 等模板

---

## 架构

### 文件变更清单

```
crates/export_engine/
├── fonts/
│   └── NotoSansCJKsc-Regular-subset.ttf   # [新增] 子集字体
├── src/
│   ├── font.rs            # [修改] 扩展 include_bytes! 分支
│   ├── theme.rs           # [修改] PdfConfig 增加 syntax_theme 字段
│   ├── renderer/
│   │   ├── mod.rs         # [修改] 入口 render_document
│   │   ├── block.rs       # [修改] render_code_block 调用高亮
│   │   ├── highlight.rs   # [新增] syntect → genpdf 适配层
│   ├── decorator.rs       # [新增] ZdownPageDecorator
│   ├── pdf.rs             # [修改] 使用 ZdownPageDecorator
│   └── lib.rs             # [修改] 暴露新模块
```

### 模块职责

| 模块 | 职责 |
|------|------|
| `font.rs` | 字体加载三层回退；`embed-fonts` feature 下 `include_bytes!` TTF |
| `theme.rs` | `PdfConfig` 集中所有配置 |
| `renderer/highlight.rs` | syntect tokenize + 样式映射 → genpdf Paragraph |
| `renderer/mod.rs` | `render_document` → 分派 `render_block` |
| `renderer/block.rs` | 各 block 渲染函数 |
| `decorator.rs` | `ZdownPageDecorator` 实现 `PageDecorator` trait |
| `pdf.rs` | `generate_pdf` 入口，组装 doc + decorator + renderer |

---

## 详细设计

### 1. 字体文件嵌入

**现状**：`font.rs` 已有 `embed-fonts` feature + `include_bytes!` 骨架，`fonts/` 目录为空（仅 `.gitkeep`）。

**改动**：
- 将子集 TTF 放入 `fonts/` 目录（Noto Sans CJK SC Regular subset）
- 扩展 `get_fallback_ttf()` 覆盖 mono 变体
- 字体自动通过 `FontData::new(data, None)` 嵌入 PDF

**配置**（无新增字段，复用现有 `FontConfig.ttf_data` 优先级）：
1. 用户提供 `ttf_data: Option<Vec<u8>>` —— 内存加载
2. font-kit 系统查找 —— 回退
3. `embed-fonts` feature 编译期内嵌 —— 最终后备

### 2. 代码块语法高亮

**现状**：`render_code_block()` 将每行作为纯文本 Paragraph 渲染。

**改动**：
- 新增 `renderer/highlight.rs`，封装 syntect 调用
- 函数签名：`fn highlight_code(code: &str, lang: &Option<String>, syntax_theme: &str) -> Vec<Vec<(genpdf::style::Style, String)>>`
- 在 `render_code_block()` 中调用高亮，逐 token 输出到 Paragraph

**样式映射**（syntect → genpdf）：
```
syntect::Style.foreground     → genpdf::style::Style.color(Rgb)
syntect::Style.font_style     → genpdf bold() / italic()
```

**配置新增**：`PdfConfig.theme.syntax_theme: String`（默认 `"InspiredGitHub"`，适合白底 PDF）

### 3. 页眉页脚页码

**现状**：`SimplePageDecorator` 仅支持页眉回调，不支持页脚。

**改动**：
- 新增 `decorator.rs`，实现 `ZdownPageDecorator: PageDecorator`
- 页眉：在内容区顶部上方打印，用 `add_offset` 向下移动内容原点
- 页脚：预留底部空间，用 `set_height` 缩小内容区高度；在页面底部固定位置打印
- 占位符替换：`{page}` / `{total}` / `{file}` / `{date}`

**`{total}` 处理（两趟渲染）**：
- genpdf 0.2 单趟渲染，`decorate_page` 调用时总页数未知，且 `Document::render()` 消费 self 无法重入
- 解决方案：需要 `{total}` 时，**第一趟**用临时 `Vec<u8>` 渲染（`{total}` → `"?"` 占位），从 Renderer 获取 `page_count`；**第二趟**用正确 `{total}` 值创建新 Document 正式渲染到输出
- 代价：字体已缓存（`FontData` 持有 bytes），`render_document` 是纯 CPU 操作——第二趟开销很小
- 不需要 `{total}` 时（模板中无此占位符），直接单趟渲染

**配置**（复用现有 `HeaderFooter`，无新增字段）：
```rust
pub struct HeaderFooter {
    pub left: String,    // 如 "{file}"
    pub center: String,  // 如 ""
    pub right: String,   // 如 "{page}/{total}"
}
```

---

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| genpdf 0.2 单趟渲染，`{total}` 不可用 | 两趟渲染——第一趟获取 page_count，第二趟正式输出 |
| syntect 主题色不适合 PDF 白底 | 默认使用 InspiredGitHub（亮色主题） |
| 子集 TTF 缺少某些 CJK 字符 | 字体加载保留系统查找和用户提供回退 |
| Areas text_section 页面底部的精确定位 | 已在设计阶段通过固定 Y 坐标计算解决 |

---

## 测试计划

- `font.rs`: 内嵌 TTF 加载测试（已有）
- `highlight.rs`: syntect 高亮 → Paragraph 输出不为空
- `decorator.rs`: HeaderFooter 占位符替换单元测试
- `pdf.rs`: 端到端 PDF 生成包含页眉页脚文本（检查 PDF 字节量/文本存在性）
