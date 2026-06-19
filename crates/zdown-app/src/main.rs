//! zdown-app：egui 应用入口（阶段 2）。

mod editor_state;
mod hybrid_view;
mod image_hosting;
mod input;
mod menu;
mod outline_view;
mod preview_view;
mod search;
mod search_state;
mod settings_dialog;
mod source_view;
mod tab_bar;
mod view_mode;

use config::ThemeMode;
use editor_engine::Cursor;
use editor_state::EditorState;
use eframe::egui;
use menu::ConfirmDialog;
use search_state::SearchState;
use view_mode::ViewMode;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("zdown 启动（阶段 2）");

    if std::env::var_os("ZDOWN_SMOKE").is_some() {
        tracing::info!("ZDOWN_SMOKE 已设置，跳过 GUI 启动");
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("zdown"),
        ..Default::default()
    };

    eframe::run_native(
        "zdown",
        options,
        Box::new(|_cc| Ok(Box::new(ZdownApp::default()))),
    )
}

struct ZdownApp {
    state: EditorState,
    confirm: ConfirmDialog,
    view_mode: ViewMode,
    settings_dialog: SettingsDialog,
    /// 缓存上次窗口标题，避免每帧 send_viewport_cmd。
    last_title: String,
    /// 缓存 SourceHighlighter 避免每帧重建。
    highlighter: Option<markdown_renderer::SourceHighlighter>,
    /// 渲染缓存（LRU 10 条）。
    render_cache: markdown_renderer::RenderCache,
    /// 大纲面板折叠状态。
    fold_state: outline_view::OutlineFoldState,
    /// 应用配置（持久化用户设置）。
    app_config: config::AppConfig,
    /// 设置对话框状态。
    settings_dialog: settings_dialog::SettingsDialog,
    /// 查找替换状态。
    search: SearchState,
    /// 当前主题模式。
    theme: ThemeMode,
}

impl Default for ZdownApp {
    fn default() -> Self {
        let app_config = config::AppConfig::load().unwrap_or_default();
        let theme = app_config.theme.clone();
        Self {
            state: EditorState::default(),
            confirm: ConfirmDialog::default(),
            view_mode: ViewMode::default(),
            settings_dialog: SettingsDialog::default(),
            last_title: String::new(),
            highlighter: {
                let syntax_name = match theme {
                    ThemeMode::Dark => "base16-ocean.dark",
                    ThemeMode::Light => "InspiredGitHub",
                };
                markdown_renderer::SourceHighlighter::with_theme(syntax_name)
                    .or_else(|_| {
                        tracing::warn!("语法主题加载失败: {syntax_name}，使用默认");
                        markdown_renderer::SourceHighlighter::new()
                    })
                    .ok()
            },
            render_cache: markdown_renderer::RenderCache::new(),
            fold_state: outline_view::OutlineFoldState::default(),
            app_config,
            settings_dialog: settings_dialog::SettingsDialog::default(),
            search: SearchState::default(),
            theme,
        }
    }
}

impl eframe::App for ZdownApp {
    #[allow(deprecated)]
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // 应用当前主题到 egui
        match self.theme {
            ThemeMode::Dark => ctx.set_visuals(egui::Visuals::dark()),
            ThemeMode::Light => ctx.set_visuals(egui::Visuals::light()),
        }

        let theme_before = self.theme.clone();
        menu::show_menu(
            ui,
            &mut self.state,
            &mut self.confirm,
            &mut self.view_mode,
            &mut self.settings_dialog,
            &self.app_config,
            &mut self.theme,
            &self.app_config.image_hosting,
        );

        // 主题切换时重建 highlighter + 保存配置
        if self.theme != theme_before {
            let syntax_name = match self.theme {
                ThemeMode::Dark => "base16-ocean.dark",
                ThemeMode::Light => "InspiredGitHub",
            };
            self.highlighter = markdown_renderer::SourceHighlighter::with_theme(syntax_name)
                .or_else(|_| {
                    tracing::warn!("语法主题加载失败: {syntax_name}，使用默认");
                    markdown_renderer::SourceHighlighter::new()
                })
                .ok();

            self.app_config.theme = self.theme.clone();
            if let Err(e) = self.app_config.save() {
                tracing::error!("配置保存失败: {e}");
            }
        }
        menu::handle_shortcuts(&ctx, &mut self.state, &mut self.confirm);

        // 搜索快捷键：Esc 关闭、Enter 导航（需在编辑器输入处理之前）
        if self.search.visible {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.search.close();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some(m) = self.search.next_match() {
                    let _ = self
                        .state
                        .editor_mut()
                        .set_cursor(Cursor::new(m.line, m.col_start));
                }
            }
        }

        // 视图模式快捷键 Ctrl+1/2/3
        let mods = ctx.input(|i| i.modifiers);

        // Ctrl+F 切换搜索栏
        if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::F)) {
            self.search.visible = !self.search.visible;
            if self.search.visible {
                self.search.focus_search = true;
                let src = self.state.editor().to_string();
                self.search.search(&src);
            } else {
                self.search.close();
            }
        }
        if mods.ctrl && !mods.shift {
            if ctx.input(|i| i.key_pressed(egui::Key::Num1)) {
                self.view_mode = ViewMode::Source;
                tracing::info!("切换到源码模式");
            } else if ctx.input(|i| i.key_pressed(egui::Key::Num2)) {
                self.view_mode = ViewMode::Preview;
                tracing::info!("切换到预览模式");
            } else if ctx.input(|i| i.key_pressed(egui::Key::Num3)) {
                self.view_mode = ViewMode::Hybrid;
                tracing::info!("切换到 Hybrid 模式");
            }
        }

        // Ctrl+I 浏览插入图片
        if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::I)) {
            menu::trigger_browse_image(&mut self.state, &self.app_config.image_hosting);
        }

        menu::show_confirm_dialog(&ctx, &mut self.state, &mut self.confirm);

        settings_dialog::show_settings_dialog(
            &ctx,
            &mut self.app_config,
            &mut self.settings_dialog,
        );

        // 标签栏（多标签页时显示）
        if self.state.tab_count() > 1 {
            let active_before = self.state.active_tab_index();
            tab_bar::show_tab_bar(ui, &mut self.state, &mut self.confirm);
            // 标签页切换时关闭搜索
            if self.state.active_tab_index() != active_before {
                self.search.close();
            }
        }

        // 状态栏（导出结果等）
        if !self.state.status_message.is_empty() {
            egui::TopBottomPanel::bottom("status_bar").show_inside(ui, |ui| {
                ui.label(
                    egui::RichText::new(&self.state.status_message)
                        .size(12.0)
                        .weak(),
                );
            });
            self.state.status_message.clear();
        }

        let highlighter = self.highlighter.as_ref();

        // 大纲侧边栏 + 中央视图区域
        egui::SidePanel::left("outline_panel")
            .resizable(true)
            .default_width(200.0)
            .min_width(60.0)
            .show_inside(ui, |ui| {
                outline_view::show_outline_panel(ui, &mut self.state, &mut self.fold_state);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            // ===== 搜索栏（Ctrl+F 激活） =====
            if self.search.visible {
                egui::Frame::group(ui.style())
                    .inner_margin(egui::Margin::symmetric(4, 2))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // 查找标签
                            ui.label(egui::RichText::new("查找:").size(13.0));

                            // 查找输入框
                            let search_id = egui::Id::new("search_query_input");
                            let search_resp = ui.add(
                                egui::TextEdit::singleline(&mut self.search.query)
                                    .id(search_id)
                                    .desired_width(200.0)
                                    .font(egui::TextStyle::Monospace),
                            );

                            // 焦点请求
                            let ctx_for_focus = ui.ctx().clone();
                            if self.search.focus_search {
                                ctx_for_focus.memory_mut(|m| m.request_focus(search_id));
                                self.search.focus_search = false;
                            }

                            // 查询变化时重新搜索
                            if search_resp.changed() {
                                let src = self.state.editor().to_string();
                                self.search.search(&src);
                                // 光标跳到当前匹配
                                if let Some(m) = self.search.current_match_pos() {
                                    let _ = self
                                        .state
                                        .editor_mut()
                                        .set_cursor(Cursor::new(m.line, m.col_start));
                                }
                            }

                            // 匹配计数
                            let count_str = match self.search.current_match {
                                Some(idx) => {
                                    format!("{}/{}", idx + 1, self.search.matches.len())
                                }
                                None => "0/0".to_string(),
                            };
                            ui.label(egui::RichText::new(count_str).size(12.0).weak());

                            ui.separator();

                            // 区分大小写按钮
                            let case_text = if self.search.case_sensitive {
                                egui::RichText::new("Aa").size(12.0).strong()
                            } else {
                                egui::RichText::new("Aa").size(12.0).weak()
                            };
                            if ui
                                .add(egui::Button::new(case_text).min_size(egui::vec2(24.0, 16.0)))
                                .clicked()
                            {
                                self.search.case_sensitive = !self.search.case_sensitive;
                                let src = self.state.editor().to_string();
                                self.search.search(&src);
                            }

                            // 全词匹配按钮
                            let word_text = if self.search.whole_word {
                                egui::RichText::new("ab|").size(12.0).strong()
                            } else {
                                egui::RichText::new("ab|").size(12.0).weak()
                            };
                            if ui
                                .add(egui::Button::new(word_text).min_size(egui::vec2(24.0, 16.0)))
                                .clicked()
                            {
                                self.search.whole_word = !self.search.whole_word;
                                let src = self.state.editor().to_string();
                                self.search.search(&src);
                            }

                            // 上/下一个匹配按钮
                            if ui
                                .add(egui::Button::new("\u{2190}").min_size(egui::vec2(20.0, 16.0)))
                                .clicked()
                            {
                                if let Some(m) = self.search.prev_match() {
                                    let _ = self
                                        .state
                                        .editor_mut()
                                        .set_cursor(Cursor::new(m.line, m.col_start));
                                }
                            }
                            if ui
                                .add(egui::Button::new("\u{2192}").min_size(egui::vec2(20.0, 16.0)))
                                .clicked()
                            {
                                if let Some(m) = self.search.next_match() {
                                    let _ = self
                                        .state
                                        .editor_mut()
                                        .set_cursor(Cursor::new(m.line, m.col_start));
                                }
                            }

                            // 关闭按钮
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("\u{2715}").color(egui::Color32::RED),
                                    )
                                    .min_size(egui::vec2(20.0, 16.0)),
                                )
                                .clicked()
                            {
                                self.search.close();
                            }
                        });

                        // 替换行
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("替换:").size(13.0));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.search.replace)
                                    .desired_width(200.0)
                                    .font(egui::TextStyle::Monospace),
                            );

                            if ui
                                .add(egui::Button::new("替换").min_size(egui::vec2(48.0, 16.0)))
                                .clicked()
                            {
                                if let Some(m) = self.search.current_match_pos().cloned() {
                                    let range = editor_engine::Selection::new(
                                        Cursor::new(m.line, m.col_start),
                                        Cursor::new(m.line, m.col_end),
                                    );
                                    let replace_text = self.search.replace.clone();
                                    let _ = self.state.editor_mut().apply(
                                        editor_engine::Command::Replace {
                                            range,
                                            text: replace_text,
                                        },
                                    );
                                    let src = self.state.editor().to_string();
                                    self.search.search(&src);
                                    if let Some(next) = self.search.current_match_pos().cloned() {
                                        let _ = self
                                            .state
                                            .editor_mut()
                                            .set_cursor(Cursor::new(next.line, next.col_start));
                                    }
                                }
                            }

                            if ui
                                .add(egui::Button::new("全部").min_size(egui::vec2(48.0, 16.0)))
                                .clicked()
                            {
                                let count = self.search.matches.len();
                                let mut sorted_matches = self.search.matches.clone();
                                sorted_matches.sort_by(|a, b| {
                                    b.line.cmp(&a.line).then(b.col_start.cmp(&a.col_start))
                                });
                                let replace_text = self.search.replace.clone();
                                for m in &sorted_matches {
                                    let range = editor_engine::Selection::new(
                                        Cursor::new(m.line, m.col_start),
                                        Cursor::new(m.line, m.col_end),
                                    );
                                    let _ = self.state.editor_mut().apply(
                                        editor_engine::Command::Replace {
                                            range,
                                            text: replace_text.clone(),
                                        },
                                    );
                                }
                                self.search.close();
                                self.state.status_message = format!("已替换 {count} 处");
                            }
                        });
                    });
            }
            // ===== 搜索栏结束 =====

            // 根据视图模式渲染
            match self.view_mode {
                ViewMode::Source => {
                    source_view::show_source_view(ui, &mut self.state, highlighter, &self.search, &self.app_config.image_hosting);
                }
                ViewMode::Preview => {
                    preview_view::show_preview_view(ui, &mut self.state, &mut self.render_cache, &self.app_config.image_hosting);
                }
                ViewMode::Hybrid => {
                    hybrid_view::show_hybrid_view(
                        ui,
                        &mut self.state,
                        highlighter,
                        &mut self.render_cache,
                        &self.app_config.image_hosting,
                    );
                }
            }
        });

        if self.state.should_exit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // 更新窗口标题（只在变化时发送，避免每帧触发窗口管理器）
        let title = format!("{} [{}]", self.state.title(), self.view_mode.label());
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }
}
