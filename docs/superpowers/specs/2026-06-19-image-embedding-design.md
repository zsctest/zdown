# PDF Image Embedding — Design Spec

**Date**: 2026-06-19
**Branch**: feature/pdf-enhancement
**Status**: approved

## 1. Overview

Enable `export_engine` to render Markdown `![alt](url)` images as actual embedded images in PDF output. Currently images are rendered as placeholder text `[图片: alt]`.

genpdf 0.2 natively supports images via `elements::Image` behind the `images` feature flag.

## 2. Image Sources

All three URL types are supported:

| Source | Example | Method |
|--------|---------|--------|
| Local file (relative) | `./images/photo.png` | `image::open(working_dir/url)` |
| Local file (absolute) | `C:\pic.jpg` | `image::open(url)` |
| Data URI | `data:image/png;base64,...` | base64 decode → `image::load_from_memory()` |
| Remote URL | `https://example.com/x.png` | `ureq::get(url)` → `image::load_from_memory()` |

## 3. Architecture

### 3.1 New module: `image_loader.rs`

```
crates/export_engine/src/image_loader.rs
```

**Public API**:

```rust
pub fn load_image(url: &str, working_dir: &Path) -> Result<DynamicImage>
```

**Internal dispatch**:

- `data:` prefix → `load_from_data_uri()`
- `http://` / `https://` prefix → `load_from_remote()`
- otherwise → `load_from_local()`

**Alpha channel handling**: If loaded image has alpha, flatten against white background before returning (genpdf/printpdf does not support transparency).

**HTTP**: Uses `ureq` (synchronous, no async runtime). Timeout: 10 seconds.

### 3.2 Renderer changes (`renderer.rs`)

**Approach: Paragraph splitting** (方案 A).

Images are inline elements in the AST but block-level elements in genpdf. When a paragraph contains `Inline::Image`, the paragraph is split at image boundaries:

```
"Hello ![](a.png) World"
  → [InlineSegment::Text("Hello "), InlineSegment::Image(a.png), InlineSegment::Text(" World")]
```

**New helper type**:

```rust
enum InlineSegment {
    Text(Vec<Inline>),
    Image { alt: String, url: String, title: Option<String> },
}
```

**Function signature changes**:

`render_heading` and `render_paragraph` change from returning `Paragraph` to pushing into `&mut LinearLayout` — consistent with `render_code_block`, `render_list`, `render_blockquote`:

```rust
// Old
fn render_heading(h: &Heading, theme: &PdfTheme) -> Paragraph
fn render_paragraph(para: &AstParagraph, theme: &PdfTheme) -> Paragraph

// New — accept layout to push into, plus working_dir for image resolution
fn render_heading(h: &Heading, theme: &PdfTheme, working_dir: Option<&Path>, layout: &mut LinearLayout)
fn render_paragraph(para: &AstParagraph, theme: &PdfTheme, working_dir: Option<&Path>, layout: &mut LinearLayout)
```

**List items with images**: `render_list` also handles inline content. Each list item's inlines are similarly split by `Inline::Image` boundaries. The existing `render_list` function already pushes into `&mut LinearLayout` directly — it will use the same `InlineSegment` split helper for item content.

**`working_dir` threading**: `working_dir` flows from `generate_pdf` → `render_document` → `render_block` → each render function. It is passed as a plain `Option<&Path>` parameter, not stored on the layout.

### 3.3 Image rendering logic

For each `InlineSegment::Image`:

1. Call `image_loader::load_image(url, working_dir)`
2. On success: create `elements::Image`, apply center alignment and auto-fit scale, push to layout
3. On failure: push `[图片: alt]` as fallback text

### 3.4 Auto-fit scaling

```
if image_width_mm <= page_content_width_mm:
    scale = 1.0
else:
    scale = page_content_width_mm / image_width_mm
```

`page_content_width` = paper_width - left_margin - right_margin. For A4: `210 - 25.4*2 = 159.2mm`.

### 3.5 Configuration

`PdfConfig` gains an optional `working_dir` field:

```rust
pub working_dir: Option<PathBuf>,
```

Default: `None` (images with relative paths cannot be loaded).

## 4. Error Handling

| Scenario | Behavior |
|----------|----------|
| File not found | `[图片: alt]` placeholder |
| Network timeout (10s) | `[图片: alt]` placeholder |
| Unsupported format | `[图片: alt]` placeholder |
| Alpha channel | Flatten to white bg, proceed |
| Image wider than page | Auto-scale down to fit |
| Valid image | Embedded with center alignment |

New error variant in `Error` enum:
```rust
#[error("图片加载失败: {0}")]
ImageLoad(String),
```

## 5. Dependencies

### Cargo.toml changes for `export_engine`:

```toml
# genpdf: enable images feature
genpdf = { workspace = true, features = ["images"] }

# new dependencies
image = "0.25"
ureq = { version = "3", default-features = false, features = ["tls"] }
base64 = "0.22"
```

## 6. Testing

### Unit tests (`image_loader.rs`):
- `load_from_data_uri_png` — valid data URI → DynamicImage
- `load_from_data_uri_invalid_base64` — returns Err
- `load_from_local_nonexistent` — returns Err
- `alpha_image_flattened` — image with alpha → no alpha after load

### Unit tests (`renderer.rs`):
- `split_inlines_no_image` — all text in one Text segment
- `split_inlines_single_image` — one Image segment
- `split_inlines_mixed` — Text, Image, Text segments
- `split_inlines_consecutive_images` — consecutive Image segments
- `auto_fit_scale_small_image` — returns (1.0, 1.0) for image < page width
- `auto_fit_scale_large_image` — returns scale < 1.0

### Integration tests (`pdf.rs`):
- `generate_pdf_with_local_image` — generates PDF with embedded image (non-empty output)
- `generate_pdf_with_missing_image` — falls back to placeholder, does not error
- `generate_pdf_with_data_uri` — data URI image embedded

### Test images:
- Create a small (10x10) PNG/JPG test image programmatically in tests
- Use a valid data URI string
- No external file dependencies for tests

## 7. Files Changed

| File | Change |
|------|--------|
| `crates/export_engine/Cargo.toml` | Enable `genpdf/images`, add `image`, `ureq`, `base64` |
| `crates/export_engine/src/lib.rs` | Register `image_loader` module |
| `crates/export_engine/src/error.rs` | Add `ImageLoad` variant |
| `crates/export_engine/src/image_loader.rs` | **New** — image loading logic |
| `crates/export_engine/src/renderer.rs` | Paragraph splitting, image embedding, push-mode refactor |
| `crates/export_engine/src/theme.rs` | Add `working_dir` to `PdfConfig` |
| `crates/export_engine/src/pdf.rs` | Pass `working_dir` through to renderer |
