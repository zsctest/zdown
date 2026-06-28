//! zdown-app：egui 应用入口（阶段 2）。

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod editor_state;
mod file_tree;
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
use fluent_bundle::FluentArgs;
use i18n::I18n;
use menu::ConfirmDialog;
use search_state::SearchState;
use terminal_panel::TerminalPanel;
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
        viewport: egui::ViewportBuilder::default()
            .with_title("zdown")
            .with_inner_size([1200.0, 800.0]),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "zdown",
        options,
        Box::new(|cc| {
            setup_egui_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(ZdownApp::default()))
        }),
    )
}

/// 配置 egui 字体以支持 CJK 字符渲染。
/// Proportional 族：CJK 字体放在最前面，优先使用。
/// Monospace 族：CJK 字体放在最后作为后备，
/// 保持默认等宽字体渲染代码，只有默认字体缺失的字形才回退到 CJK。
fn setup_egui_cjk_fonts(ctx: &egui::Context) {
    if let Some(data) = find_system_cjk_font() {
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert("CJK".to_owned(), egui::FontData::from_owned(data).into());
        // Proportional: CJK 优先，保证 UI 中文正常渲染
        if let Some(proportional) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            proportional.insert(0, "CJK".to_owned());
        }
        // Monospace: CJK 作为后备，等宽字体优先保证代码对齐
        if let Some(monospace) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            monospace.push("CJK".to_owned());
        }
        ctx.set_fonts(fonts);
        tracing::info!("已加载系统 CJK 字体用于界面渲染");
    } else {
        tracing::warn!("未找到系统 CJK 字体，非 ASCII 字符可能显示为方块");
    }
}

/// 使用 font-kit 在系统字体目录中查找支持 CJK 的字体。
fn find_system_cjk_font() -> Option<Vec<u8>> {
    use font_kit::family_name::FamilyName;
    use font_kit::properties::Properties;
    use font_kit::source::SystemSource;

    let source = SystemSource::new();

    // 按优先级排列的 CJK 字体名列表（分平台）
    let candidates: &[&str] = if cfg!(target_os = "windows") {
        &["Microsoft YaHei", "SimHei", "SimSun", "FangSong", "KaiTi"]
    } else if cfg!(target_os = "macos") {
        &[
            "PingFang SC",
            "PingFang TC",
            "Hiragino Sans GB",
            "Heiti SC",
            "STHeiti",
        ]
    } else {
        &[
            "Noto Sans CJK SC",
            "Noto Sans SC",
            "Source Han Sans SC",
            "WenQuanYi Micro Hei",
            "WenQuanYi Zen Hei",
        ]
    };

    for name in candidates {
        let handle = match source
            .select_best_match(&[FamilyName::Title(name.to_string())], &Properties::new())
        {
            Ok(h) => h,
            Err(_) => continue,
        };

        let data = match handle {
            font_kit::handle::Handle::Path { path, .. } => std::fs::read(path).ok(),
            font_kit::handle::Handle::Memory { bytes, .. } => Some((*bytes).clone()),
        };

        if let Some(data) = data {
            if !data.is_empty() {
                tracing::info!("找到系统 CJK 字体: {name}");
                return Some(data);
            }
        }
    }

    None
}

struct ZdownApp {
    state: EditorState,
    confirm: ConfirmDialog,
    view_mode: ViewMode,
    /// 缓存上次窗口标题，避免每帧 send_viewport_cmd。
    last_title: String,
    /// 缓存 SourceHighlighter 避免每帧重建。
    highlighter: Option<markdown_renderer::SourceHighlighter>,
    /// 渲染缓存（LRU 10 条）。
    render_cache: markdown_renderer::RenderCache,
    /// 图片加载缓存（避免逐帧重复下载）。
    image_cache: markdown_renderer::ImageCache,
    /// 大纲面板折叠状态。
    fold_state: outline_view::OutlineFoldState,
    /// 大纲面板拖拽排序状态。
    outline_drag: outline_view::OutlineDragState,
    /// 大纲面板搜索过滤状态。
    outline_filter: outline_view::OutlineFilterState,
    /// 应用配置（持久化用户设置）。
    app_config: config::AppConfig,
    /// 设置对话框状态。
    settings_dialog: settings_dialog::SettingsDialog,
    /// 查找替换状态。
    search: SearchState,
    /// 当前主题模式。
    theme: ThemeMode,
    /// 国际化管理器。
    i18n: I18n,
    /// 终端面板。
    terminal: TerminalPanel,
    /// 文件树面板。
    file_tree: file_tree::FileTreeState,
    /// 左侧面板竖分割比例（大纲占比，0.0~1.0）。
    side_split_ratio: f32,
}

impl Default for ZdownApp {
    fn default() -> Self {
        let app_config = config::AppConfig::load().unwrap_or_default();
        let theme = app_config.theme.clone();
        let lang = app_config
            .lang
            .parse::<i18n::Lang>()
            .unwrap_or(i18n::Lang::ZhCN);
        Self {
            state: EditorState::default(),
            confirm: ConfirmDialog::default(),
            view_mode: ViewMode::default(),
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
            image_cache: markdown_renderer::ImageCache::new(),
            fold_state: outline_view::OutlineFoldState::default(),
            outline_drag: outline_view::OutlineDragState::default(),
            outline_filter: outline_view::OutlineFilterState::default(),
            app_config,
            settings_dialog: settings_dialog::SettingsDialog::default(),
            search: SearchState::default(),
            theme,
            i18n: I18n::with_lang(lang),
            terminal: TerminalPanel::default(),
            file_tree: file_tree::FileTreeState::default(),
            side_split_ratio: 0.5,
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
            &self.i18n,
            &mut self.terminal,
            &mut self.file_tree,
        );

        // 拖拽文件夹到窗口：打开文件夹
        ctx.input(|i| {
            for file in &i.raw.dropped_files {
                if let Some(path) = &file.path {
                    if path.is_dir() {
                        self.file_tree.open_folder(path);
                    }
                }
            }
        });

        menu::handle_shortcuts(
            &ctx,
            &mut self.state,
            &mut self.confirm,
            &mut self.view_mode,
            &mut self.theme,
            &self.app_config,
        );

        // 主题切换时重建 highlighter + 保存配置
        // 注：此检查放在 show_menu 和 handle_shortcuts 之后，
        // 确保无论通过菜单还是快捷键切换主题都能正确更新高亮。
        if self.theme != theme_before {
            let syntax_name = match self.theme {
                ThemeMode::Dark => "base16-ocean.dark",
                ThemeMode::Light => "InspiredGitHub",
            };
            // 优先使用 set_theme 动态切换，避免重建 SyntaxSet
            let switched = self
                .highlighter
                .as_mut()
                .map(|h| h.set_theme(syntax_name))
                .transpose();
            match switched {
                Ok(Some(())) => {
                    tracing::debug!("语法主题已切换: {syntax_name}");
                }
                Ok(None) | Err(_) => {
                    tracing::warn!("语法主题动态切换失败，重建高亮器: {syntax_name}");
                    self.highlighter =
                        markdown_renderer::SourceHighlighter::with_theme(syntax_name)
                            .or_else(|_| {
                                tracing::warn!("语法主题加载失败，使用默认");
                                markdown_renderer::SourceHighlighter::new()
                            })
                            .ok();
                }
            }

            self.app_config.theme = self.theme.clone();
            if let Err(e) = self.app_config.save() {
                tracing::error!("配置保存失败: {e}");
            }
        }

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
                    self.state.needs_scroll_cursor = true;
                }
            }
        }

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

        // Ctrl+` 切换终端
        if mods.ctrl
            && !mods.shift
            && !mods.alt
            && ctx.input(|i| i.key_pressed(egui::Key::Backtick))
        {
            self.terminal.toggle(&ctx);
        }

        // Ctrl+I 浏览插入图片
        if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::I)) {
            menu::trigger_browse_image(&mut self.state, &self.app_config.image_hosting, &self.i18n);
        }

        menu::show_confirm_dialog(&ctx, &mut self.state, &mut self.confirm, &self.i18n);

        settings_dialog::show_settings_dialog(
            &ctx,
            &mut self.app_config,
            &mut self.settings_dialog,
            &mut self.i18n,
        );

        // 标签栏（多标签页时显示）
        if self.state.tab_count() > 1 {
            let active_before = self.state.active_tab_index();
            tab_bar::show_tab_bar(ui, &mut self.state, &mut self.confirm, &self.i18n);
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

        // 大纲侧边栏 + 文件树 + 中央视图区域
        egui::SidePanel::left("outline_panel")
            .resizable(true)
            .default_width(200.0)
            .min_width(60.0)
            .show_inside(ui, |ui| {
                let available = ui.available_height();
                let handle_height = 6.0;
                let min_top = 100.0;
                let min_bottom = 40.0;
                let split = self.side_split_ratio.clamp(0.1, 0.9);

                let max_h = available - handle_height;
                // 按比例分割，但确保两个面板都有合理的最小空间。
                // (max_h - min_bottom).max(min_top) 保证即使在空间紧张时
                // 大纲面板也不会被压缩到 min_top 以下。
                let top_h = (max_h * split)
                    .max(min_top)
                    .min((max_h - min_bottom).max(min_top.min(max_h)));

                // 上半部：大纲面板
                let top_rect = ui.available_rect_before_wrap();
                let top_rect =
                    egui::Rect::from_min_size(top_rect.min, egui::vec2(top_rect.width(), top_h));
                ui.allocate_ui_at_rect(top_rect, |ui| {
                    ui.set_min_height(min_top);
                    outline_view::show_outline_panel(
                        ui,
                        &mut self.state,
                        &mut self.fold_state,
                        &mut self.outline_drag,
                        &mut self.outline_filter,
                        &self.i18n,
                    );
                });

                // 拖拽手柄
                let handle_rect = egui::Rect::from_min_size(
                    egui::pos2(top_rect.min.x, top_rect.max.y),
                    egui::vec2(top_rect.width(), handle_height),
                );
                let handle_id = ui.make_persistent_id("side_split_handle");
                let handle_resp = ui.interact(handle_rect, handle_id, egui::Sense::drag());

                if handle_resp.dragged() {
                    if let Some(pos) = ui.ctx().pointer_latest_pos() {
                        let panel_top = ui.available_rect_before_wrap().min.y;
                        let panel_h = available;
                        let new_ratio = (pos.y - panel_top) / panel_h;
                        self.side_split_ratio = new_ratio.clamp(0.1, 0.9);
                    }
                }

                // 视觉手柄
                if ui.is_rect_visible(handle_rect) {
                    let painter = ui.painter();
                    let color = if handle_resp.hovered() || handle_resp.dragged() {
                        egui::Color32::GRAY
                    } else {
                        egui::Color32::from_gray(80)
                    };
                    let center_y = handle_rect.center().y;
                    let left = handle_rect.min.x + 8.0;
                    let right = handle_rect.max.x - 8.0;
                    painter.line_segment(
                        [egui::pos2(left, center_y), egui::pos2(right, center_y)],
                        egui::Stroke::new(2.0, color),
                    );
                }

                // 设置游标样式
                if handle_resp.hovered() || handle_resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }

                // 下半部：文件树面板
                let bottom_h = available - top_h - handle_height;
                let bottom_rect = egui::Rect::from_min_size(
                    egui::pos2(handle_rect.min.x, handle_rect.max.y),
                    egui::vec2(handle_rect.width(), bottom_h),
                );
                ui.allocate_ui_at_rect(bottom_rect, |ui| {
                    ui.set_min_height(min_bottom);
                    file_tree::show_file_tree_panel(
                        ui,
                        &mut self.file_tree,
                        &mut self.state,
                        &self.i18n,
                        &mut self.image_cache,
                    );
                });
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            // ===== 搜索栏（Ctrl+F 激活） =====
            if self.search.visible {
                egui::Frame::group(ui.style())
                    .inner_margin(egui::Margin::symmetric(4, 2))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // 查找标签
                            ui.label(egui::RichText::new(self.i18n.t("search-find")).size(13.0));

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
                                    self.state.needs_scroll_cursor = true;
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
                                    self.state.needs_scroll_cursor = true;
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
                                    self.state.needs_scroll_cursor = true;
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
                            ui.label(egui::RichText::new(self.i18n.t("search-replace")).size(13.0));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.search.replace)
                                    .desired_width(200.0)
                                    .font(egui::TextStyle::Monospace),
                            );

                            if ui
                                .add(
                                    egui::Button::new(self.i18n.t("search-replace-btn"))
                                        .min_size(egui::vec2(48.0, 16.0)),
                                )
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
                                .add(
                                    egui::Button::new(self.i18n.t("search-replace-all"))
                                        .min_size(egui::vec2(48.0, 16.0)),
                                )
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
                                let mut args = FluentArgs::new();
                                args.set("count", count as i64);
                                self.state.status_message =
                                    self.i18n.tr("status-replaced-count", Some(&args));
                            }
                        });
                    });
            }
            // ===== 搜索栏结束 =====

            // 根据视图模式渲染
            match self.view_mode {
                ViewMode::Source => {
                    source_view::show_source_view(
                        ui,
                        &mut self.state,
                        highlighter,
                        &self.search,
                        &self.app_config.image_hosting,
                    );
                }
                ViewMode::Preview => {
                    preview_view::show_preview_view(
                        ui,
                        &mut self.state,
                        &mut self.render_cache,
                        &mut self.image_cache,
                        &self.app_config.image_hosting,
                    );
                }
                ViewMode::Hybrid => {
                    hybrid_view::show_hybrid_view(
                        ui,
                        &mut self.state,
                        highlighter,
                        &mut self.render_cache,
                        &mut self.image_cache,
                        &self.app_config.image_hosting,
                    );
                }
            }
        });

        // ===== 终端面板 (Ctrl+`) =====
        if self.terminal.visible {
            egui::TopBottomPanel::bottom("terminal_panel")
                .resizable(true)
                .default_height(self.terminal.height)
                .min_height(60.0)
                .show_inside(ui, |ui| {
                    self.terminal.show(ui);
                });
        }

        if self.state.should_exit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // 更新窗口标题（只在变化时发送，避免每帧触发窗口管理器）
        let title = format!(
            "{} [{}]",
            self.state.title(),
            self.i18n.t(self.view_mode.label())
        );
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }
}
