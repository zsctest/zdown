//! zdown-app 二进制 crate（阶段 0）。
//!
//! 当前职责：初始化 tracing，启动一个显示 "zdown skeleton" 占位内容的 egui 窗口。
//! 多 crate 编排与编辑功能在后续阶段逐步加入。

mod editor_state;

use eframe::egui;

fn main() -> eframe::Result {
    // 初始化 tracing：优先读 RUST_LOG，缺失时退回 info 级别。
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("zdown 启动（阶段 0 骨架）");

    // CI smoke 模式：仅验证初始化与依赖加载，不启动 GUI 窗口。
    // 用法：ZDOWN_SMOKE=1 cargo run -p zdown-app
    if std::env::var_os("ZDOWN_SMOKE").is_some() {
        tracing::info!("ZDOWN_SMOKE 已设置，跳过 GUI 启动");
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("zdown"),
        ..Default::default()
    };

    eframe::run_ui_native("zdown", options, |ui, _frame| {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("zdown skeleton");
        });
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "zdown-app");
    }
}
