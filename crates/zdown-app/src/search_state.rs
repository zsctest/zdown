//! 搜索栏 UI 状态。

use crate::search::{Match, SearchEngine, SearchOptions};

/// 搜索栏 UI 状态，由 ZdownApp 持有。
#[derive(Default)]
pub struct SearchState {
    /// 搜索栏是否可见。
    pub visible: bool,
    /// 搜索文本输入。
    pub query: String,
    /// 替换文本输入。
    pub replace: String,
    /// 区分大小写。
    pub case_sensitive: bool,
    /// 全词匹配。
    pub whole_word: bool,
    /// 当前所有匹配位置。
    pub matches: Vec<Match>,
    /// 当前高亮匹配索引。
    pub current_match: Option<usize>,
    /// 下一帧需请求搜索框焦点。
    pub focus_search: bool,
}

impl SearchState {
    /// 用当前查询和选项搜索文本，更新匹配列表。
    pub fn search(&mut self, text: &str) {
        let opts = SearchOptions {
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
        };
        self.matches = SearchEngine::find_all(text, &self.query, &opts);
        if self.matches.is_empty() {
            self.current_match = None;
        } else {
            // 尝试保持当前匹配索引在有效范围
            if let Some(idx) = self.current_match {
                if idx >= self.matches.len() {
                    self.current_match = Some(self.matches.len().saturating_sub(1));
                }
            } else {
                self.current_match = Some(0);
            }
        }
    }

    /// 跳到下一个匹配。返回新匹配位置（用于移动光标）。
    pub fn next_match(&mut self) -> Option<Match> {
        if self.matches.is_empty() {
            return None;
        }
        let next = match self.current_match {
            Some(idx) if idx + 1 < self.matches.len() => idx + 1,
            _ => 0, // 循环回到第一个
        };
        self.current_match = Some(next);
        Some(self.matches[next].clone())
    }

    /// 跳到上一个匹配。
    pub fn prev_match(&mut self) -> Option<Match> {
        if self.matches.is_empty() {
            return None;
        }
        let prev = match self.current_match {
            Some(idx) if idx > 0 => idx - 1,
            _ => self.matches.len().saturating_sub(1), // 循环到最后一个
        };
        self.current_match = Some(prev);
        Some(self.matches[prev].clone())
    }

    /// 关闭搜索栏并清除状态。
    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.replace.clear();
        self.matches.clear();
        self.current_match = None;
        self.focus_search = false;
    }

    /// 当前匹配（如果存在）。
    pub fn current_match_pos(&self) -> Option<&Match> {
        self.current_match.and_then(|idx| self.matches.get(idx))
    }
}
