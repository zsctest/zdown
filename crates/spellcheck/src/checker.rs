//! check() 实现：词法分析 + token 过滤 + 拼写查询。

use crate::{SpellChecker, SpellError};

/// 检查全文，返回拼写错误列表。
///
/// # 算法
/// 1. 逐字节遍历，识别代码围栏块（```）和行内代码（`）并跳过
/// 2. 在非代码文本中，按非字母字符拆分提取单词 token
/// 3. 过滤：跳过长度 ≤ 1、纯数字、URL
/// 4. 对保留的 token 调用 spellbook check()
pub fn check(checker: &SpellChecker, text: &str) -> Vec<SpellError> {
    let mut errors = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_fenced_code = false;

    while i < len {
        // 检测行首 — 检测围栏代码块的开始/结束
        if (i == 0 || bytes[i - 1] == b'\n')
            && i + 2 < len
            && bytes[i] == b'`'
            && bytes[i + 1] == b'`'
            && bytes[i + 2] == b'`'
        {
            in_fenced_code = !in_fenced_code;
            // 跳过本行剩余
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        if in_fenced_code {
            i += 1;
            continue;
        }

        // 检测行内代码 `...`
        if bytes[i] == b'`' {
            let backtick_pos = i;
            i += 1;
            while i < len && bytes[i] != b'`' {
                i += 1;
            }
            if i < len {
                i += 1; // 跳过闭合 `
                continue;
            }
            // 未闭合反引号：跳过该反引号，作为普通字符继续
            i = backtick_pos + 1;
        }

        // 检测单词起始（字母或撇号）
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'\'' {
            let start = i;
            i += 1;
            while i < len && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'\'') {
                i += 1;
            }
            let word_bytes = &bytes[start..i];
            let word = std::str::from_utf8(word_bytes).unwrap_or("");

            // 过滤规则
            if should_check(word) && !checker.check_word(word) {
                errors.push(SpellError {
                    word: word.to_string(),
                    span: (start, i),
                });
            }
        } else {
            i += 1;
        }
    }

    errors
}

/// 判断一个 token 是否应该被拼写检查。
fn should_check(word: &str) -> bool {
    // 跳过空或单字符
    if word.len() <= 1 {
        return false;
    }
    // 跳过纯数字
    if word.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    // 跳过 URL 片段
    if word.contains("://") {
        return false;
    }
    // 跳过全大写缩写（如 HTML, CSS, API）
    if word.len() >= 2 && word.chars().all(|c| c.is_ascii_uppercase()) {
        return false;
    }
    // 跳过带数字的混合 token（如 "v2", "foo123"）
    if word.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn make_checker() -> SpellChecker {
        SpellChecker::new().expect("load dict")
    }

    #[test]
    fn check_hello_is_correct() {
        let c = make_checker();
        assert!(c.check_word("hello"));
    }

    #[test]
    fn check_misspelling_is_wrong() {
        let c = make_checker();
        assert!(!c.check_word("helo"));
    }

    #[test]
    fn check_all_correct_returns_empty() {
        let c = make_checker();
        let errors = c.check("hello world");
        assert!(errors.is_empty());
    }

    #[test]
    fn check_misspelled_returns_errors() {
        let c = make_checker();
        let errors = c.check("helo wrld");
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].word, "helo");
        assert_eq!(errors[1].word, "wrld");
    }

    #[test]
    fn skip_numbers() {
        let c = make_checker();
        let errors = c.check("123 456");
        assert!(errors.is_empty());
    }

    #[test]
    fn skip_single_char() {
        let c = make_checker();
        let errors = c.check("a b c");
        assert!(errors.is_empty());
    }

    #[test]
    fn skip_fenced_code_block() {
        let c = make_checker();
        let errors = c.check("```\nhelo\n```\nhelo");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].word, "helo");
    }

    #[test]
    fn skip_inline_code() {
        let c = make_checker();
        let errors = c.check("`helo` helo");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].word, "helo");
    }

    #[test]
    fn skip_all_caps_abbreviation() {
        let c = make_checker();
        let errors = c.check("HTML CSS API");
        assert!(errors.is_empty());
    }

    #[test]
    fn skip_mixed_digit_tokens() {
        let c = make_checker();
        let errors = c.check("v2 foo123 3d");
        assert!(errors.is_empty());
    }

    #[test]
    fn empty_text_returns_empty() {
        let c = make_checker();
        let errors = c.check("");
        assert!(errors.is_empty());
    }

    #[test]
    fn unclosed_inline_code_does_not_eat_remaining() {
        let c = make_checker();
        // "`helo" 未闭合反引号，"helo" 拼写错误应被标记；"world" 正确、不受影响
        let errors = c.check("`helo world");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].word, "helo");
    }

    #[test]
    fn all_fenced_code_returns_empty() {
        let c = make_checker();
        let errors = c.check("```\nhelo\nwrld\n```");
        assert!(errors.is_empty());
    }

    #[test]
    fn apostrophe_words_are_checked() {
        let c = make_checker();
        // "don't" 和 "it's" 都是合法英文缩写，词典应收录
        let errors = c.check("don't it's");
        assert!(errors.is_empty());
    }

    #[test]
    fn misspelled_apostrophe_word_is_caught() {
        let c = make_checker();
        let errors = c.check("dont't");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].word, "dont't");
    }
}
