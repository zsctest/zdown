# 自定义图床 设计规格

> 2026-06-20

## 目标

支持用户通过拖拽、粘贴、浏览三种方式将图片插入 Markdown 文档，
图片可按三种存储策略（本地文件、base64 内联、云端图床 SM.MS）持久化。

## 架构

```
zdown-app/
├── image_hosting.rs          ← 新增：ImageStorage trait + 3 种实现
├── source_view.rs            ← 修改：处理拖拽/粘贴/浏览事件 → 插入图片
├── settings_dialog.rs        ← 修改：增加"图片"标签页
config/
├── lib.rs                    ← 修改：AppConfig 增加 ImageConfig 和 ImgHostingConfig
```

- `export_engine::image_loader::load_image()` 已负责"读取"方向（本地/base64/远端→`DynamicImage`），本功能新增的 `image_hosting` 负责"写入"方向（内存→本地文件/base64字符串/云端URL）。
- `image_hosting` 模块放入 `zdown-app` crate（依赖 egui rfd 和 ureq），不放入 `export_engine`（export_engine 只管导出，不管 UI 交互）。

## ImageStorage trait

```rust
/// 图片存储后端：负责将图片数据持久化并返回 Markdown 中使用的 URL。
pub trait ImageStorage {
    /// 存储图片，返回 Markdown 图片 URL。
    /// 
    /// * `data` — 图片字节数据
    /// * `filename` — 原始文件名（用于生成存储文件名）
    /// * `format` — 图片格式（png/jpeg/gif/webp/svg）
    fn store(&self, data: &[u8], filename: &str, format: ImageFormat) -> Result<String>;
}

/// 支持的图片格式。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    WebP,
    Svg,
    /// 无法识别的格式，存储为 .bin
    Unknown,
}
```

### 实现 1：LocalStorage

- **职责**：将图片复制到文档所在目录的 `images/` 子目录（目录名可配置）
- **store 行为**：
  1. 确定目标目录：当前 Markdown 文件的父目录 + `images/`
  2. 若 Markdown 文件未保存（无路径），则使用系统临时目录
  3. 若文件名为纯数字/无扩展名，自动补扩展名
  4. 若目标文件已存在，添加 `_N` 后缀去重
  5. 写入文件
  6. 返回相对路径 URL：`images/xxx.png`
- **错误处理**：IO 错误返回 `Err("图片保存失败: {io_err}")`

### 实现 2：Base64Storage

- **职责**：将图片编码为 data URI
- **store 行为**：
  1. 将 `data` 用 base64 编码
  2. 根据格式确定 MIME type
  3. 返回 `data:image/png;base64,<encoded>`
- **错误处理**：无失败路径（纯内存操作）

### 实现 3：SmMsStorage

- **职责**：上传图片到 sm.ms 免费图床
- **依赖**：`ureq`（HTTP 客户端，已在 export_engine 中使用）
- **store 行为**：
  1. POST multipart/form-data 到 `https://sm.ms/api/v2/upload`
  2. 若配置了 API token，添加 `Authorization: <token>` 头
  3. 解析 JSON 响应：`success` → 提取 `data.url`；`image_repeated` → 使用已有 URL
  4. 返回远程 URL
- **错误处理**：网络错误 / API 错误 / 限速 → 返回 `Err("上传失败: {reason}")`
- **token 配置**：用户可到 https://sm.ms 注册获取 API token，填入设置

## 图片插入交互

### 拖拽插入

- **触发**：egui `ctx.input(|i| i.raw.dropped_files)` 检测
- **流程**：
  1. 遍历 dropped files，筛选 MIME 类型为 `image/*` 的文件
  2. 读取文件字节
  3. 根据文件名推断 `ImageFormat`
  4. 调用 `ImageStorage::store()` → 获得 URL
  5. 生成 `![filename](url)` 文本
  6. 使用 `Command::Insert { text: "![...](...)" }` 插入编辑器光标处
- **多文件**：每个图片一行，空格分隔

### 粘贴插入

- **触发**：`ctx.input(|i| i.raw.events)` 中检测 `Event::Paste` 
- **流程**：
  1. 尝试从剪贴板读取图片数据（egui 0.34 支持 `clipboard_image`）
  2. 解码图片 → 获得字节 + 格式
  3. 调用 `ImageStorage::store()` → 获得 URL
  4. 生成 `![image](url)` 插入光标处
- **降级**：若无图片数据（纯文本粘贴），走正常文本粘贴流程

### 浏览插入（Ctrl+I）

- **触发**：Ctrl+I 快捷键
- **流程**：
  1. 弹出文件对话框，过滤图片扩展名（png/jpg/jpeg/gif/webp/svg/bmp）
  2. 读取选中文件
  3. 调用 `ImageStorage::store()` → 获得 URL
  4. 插入 `![filename](url)` 到光标处

## 配置（AppConfig 扩展）

```toml
[image_hosting]
default_strategy = "local"    # "local" | "base64" | "smms"
local_dir = "images"           # 本地图片子目录名

[image_hosting.smms]
api_token = ""                 # SM.MS API token（可选，无 token 也可上传）
```

对应 Rust 类型：

```rust
/// 图片存储策略。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum ImageStrategy {
    #[default]
    Local,
    Base64,
    SmMs,
}

/// 图片托管配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageHostingConfig {
    pub default_strategy: ImageStrategy,
    pub local_dir: String,          // 默认 "images"
    pub smms: SmMsConfig,
}

impl Default for ImageHostingConfig {
    fn default() -> Self {
        Self {
            default_strategy: ImageStrategy::Local,
            local_dir: "images".into(),
            smms: SmMsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SmMsConfig {
    pub api_token: String,
}
```

`AppConfig` 增加字段：
```rust
pub struct AppConfig {
    pub custom_css: Option<String>,
    pub theme: ThemeMode,
    pub image_hosting: ImageHostingConfig,   // 新增
}
```

向后兼容：`#[serde(default)]` 确保旧配置文件缺少 `image_hosting` 字段时使用默认值。

## 设置对话框扩展

现有 `SettingsDialog` 增加"图片"标签页，包含：

1. **默认存储策略** — 下拉选择（本地 / Base64 内联 / SM.MS）
2. **本地图片目录** — 文本输入框，默认 `images`
3. **SM.MS API Token** — 文本输入框 + "获取 Token"链接（打开浏览器）

标签页布局：在现有 CSS 标签页旁边增加"图片"标签按钮。

## 错误处理

- 拖拽文件读取失败 → 状态栏显示"图片读取失败: {reason}"
- base64 编码 → 不会失败
- 网络上传失败 → 状态栏显示"上传 SM.MS 失败: {reason}"
- 本地写入失败 → 状态栏显示"图片保存失败: {reason}"
- 不支持的图片格式 → 跳过，状态栏提示

所有错误不阻塞编辑器操作，通过 `status_message` 告知用户。

## 验收标准

1. 拖拽 PNG/JPEG 文件到编辑器 → 图片按默认策略存储，`![alt](url)` 出现在光标位置
2. 粘贴剪贴板图片 → 同上
3. Ctrl+I 浏览选择图片 → 同上
4. 切换存储策略为 base64 → 插入图片生成 data URI
5. 切换存储策略为 SM.MS → 插入图片上传到云端
6. 配置持久化：修改设置 → 重启 → 设置保留
7. 降级：无网络时 SM.MS 上传失败显示错误信息，不 panic
8. 多文件拖拽 → 每个图片一行
