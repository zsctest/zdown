# zdown 任务清单

本文件只细化当前即将实施的阶段（阶段 0）。后续阶段在进入实施前再展开细化，避免长期任务清单失真。

每条任务标注：`[crate]` 范围、验收点。可在阶段 0 内独立并行。

---

## 阶段 0 — 工程骨架

### 0.1 workspace 初始化

- [x] **T0-01** 创建根 `Cargo.toml`，声明 workspace + `[workspace.dependencies]` 公共依赖版本
  - 范围：根
  - 验收：`cargo metadata` 成功，无成员时报错可接受
- [x] **T0-02** 创建 `crates/` 目录与 7 个成员 crate 子目录
  - 范围：根
  - 验收：目录结构存在

### 0.2 各 crate 骨架（可并行）

每项任务格式：创建 `crates/<name>/Cargo.toml` + `src/lib.rs`（或 `main.rs`）占位，导出空模块，含一个占位测试。

- [x] **T0-03** `document_model` 骨架
  - 验收：`cargo build -p document_model` 通过，`cargo test -p document_model` 通过 1 个占位测试
- [x] **T0-04** `editor_engine` 骨架
  - 验收：同上
- [x] **T0-05** `markdown_renderer` 骨架
  - 验收：同上
- [x] **T0-06** `export_engine` 骨架
  - 验收：同上
- [x] **T0-07** `workspace` 骨架
  - 验收：同上
- [x] **T0-08** `config` 骨架
  - 验收：同上
- [x] **T0-09** `zdown-app` 骨架（二进制 crate）
  - 验收：`cargo build -p zdown-app` 通过

### 0.3 egui 窗口跑通

- [x] **T0-10** `zdown-app` 集成 `eframe`，启动一个显示 "zdown" 标题的窗口，内容为占位文字 "zdown skeleton"
  - 范围：`zdown-app`
  - 验收：`cargo run -p zdown-app` 弹出窗口，无 panic
  - 备注：CI 无法验证 GUI，需本地手动确认，留截图
  - 完成：本地手动确认弹窗正常（标题 "zdown"，内容 "zdown skeleton"）；CI 通过 `ZDOWN_SMOKE=1 cargo run -p zdown-app` smoke 校验

### 0.4 质量基线

- [x] **T0-11** 添加 `clippy` 配置（`clippy.toml` 或 `Cargo.toml` 内 `[lints]`），开启 `-D warnings`
  - 验收：`cargo clippy --workspace -- -D warnings` 通过
  - 完成：在根 `Cargo.toml` 的 `[workspace.lints.clippy]` 中声明 `unwrap_used`/`expect_used`/`dbg_macro` = deny，其余 warn
- [x] **T0-12** 添加 `rustfmt` 配置（`rustfmt.toml`，统一版式）
  - 验收：`cargo fmt --check` 通过
  - 完成：`rustfmt.toml` 指定 `edition = "2024"`、`max_width = 100`
- [x] **T0-13** 各 crate 定义 `Error` 类型骨架（`thiserror` 派生，含一个变体占位）
  - 验收：`cargo build` 通过
  - 完成：六个库 crate 各自定义 `Error { NotImplemented }`，附 `error_display` 测试
- [x] **T0-14** `zdown-app` 初始化 `tracing-subscriber`，启动时打印一条 info 日志
  - 验收：运行时终端可见日志输出
  - 完成：`EnvFilter` 读 `RUST_LOG`，缺省退回 `info`；smoke 运行确认日志可见

### 0.5 CI

- [x] **T0-15** 添加 CI 配置（CNB 或 GitHub Actions）：
  - Windows：`fmt --check` + `clippy -D warnings` + `test` + `build` + `run` smoke
  - Linux / macOS：仅 `build`（无 GUI runner，不跑测试与 GUI）
  - 备注：阶段 0 不引入 `rfd`，Linux 无需装 gtk
  - 完成：选定 GitHub Actions；`.github/workflows/ci.yml` 三平台矩阵；Linux 安装 eframe 系统依赖；smoke 通过 `ZDOWN_SMOKE` 环境变量触发

### 0.6 文档与提交规范

- [x] **T0-16** 添加 `rust-toolchain.toml`，固定 Rust 版本（建议 stable）
  - 验收：`rustc --version` 与文件一致
  - 完成：`channel = "stable"`，components `rustfmt` + `clippy`，profile `minimal`
- [x] **T0-17** 更新 `.gitignore`，确保 `target/`、IDE 文件被忽略
  - 验收：`git status` 干净
  - 完成：含 `target/`、IDE、`.codegraph/`、`.cargo/config.toml`（本机配置不入库）

---

## 阶段 0 验收（汇总）

完成 T0-01 ~ T0-17 后，需同时满足：

1. Windows：`cargo build --workspace` 通过 ✅
2. Windows：`cargo run -p zdown-app` 弹出窗口 ✅（本地手动确认）
3. Windows：`cargo test --workspace` 全绿 ✅（13 个测试通过）
4. Windows：`cargo clippy --workspace -- -D warnings` 无警告 ✅
5. `cargo fmt --check` 通过 ✅
6. Linux / macOS CI：`cargo build --workspace` 通过（仅编译验证）⏳（待首次 push 后 CI 验证）
7. CI 在上述矩阵绿灯 ⏳（待首次 push 后 CI 验证）

> 阶段 0 本地验收项全部通过，跨平台 CI 待首次推送后在 GitHub Actions 上验证。
> 满足后，阶段 0 关闭，进入阶段 1（届时展开阶段 1 任务清单）。

---

## 后续阶段（待细化）

- 阶段 1 MVP：进入实施前展开
- 阶段 2 渲染与所见即所得：进入实施前展开
- 阶段 3 个性化与多文件：进入实施前展开
- 阶段 4 高级功能：按子里程碑（图床 / 导出 / mermaid / HTML / i18n / 终端 / AI / 插件）分别展开

不在本文件预先列出，避免任务清单过早固化。
