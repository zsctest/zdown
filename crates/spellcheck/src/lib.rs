//! 英文拼写检查。基于 spellbook + 嵌入 en_US Hunspell 词典。

mod checker;

/// 拼写错误信息。
#[derive(Debug, Clone, PartialEq)]
pub struct SpellError {
    /// 错误单词。
    pub word: String,
    /// 在原文中的字节偏移 (start_byte, end_byte)。
    pub span: (usize, usize),
}

/// 拼写检查器。
pub struct SpellChecker {
    dict: spellbook::Dictionary,
}

impl SpellChecker {
    /// 从嵌入的 en_US 词典构建。
    /// 词典文件通过 include_str! 编译时嵌入。
    pub fn new() -> Result<Self, SpellcheckError> {
        let aff = include_str!("dict/en_US.aff");
        let dic = include_str!("dict/en_US.dic");
        let dict = spellbook::Dictionary::new(aff, dic)
            .map_err(|e| SpellcheckError::Parse(e.to_string()))?;
        Ok(Self { dict })
    }

    /// 检查单个单词。
    pub fn check_word(&self, word: &str) -> bool {
        self.dict.check(word)
    }

    /// 检查整段文本，返回所有拼写错误。
    /// 调用 checker::check() 执行完整的词法分析与过滤逻辑。
    pub fn check(&self, text: &str) -> Vec<SpellError> {
        checker::check(self, text)
    }
}

/// 拼写检查错误类型。
#[derive(Debug, Clone, thiserror::Error)]
pub enum SpellcheckError {
    /// 词典解析失败。
    #[error("词典解析失败: {0}")]
    Parse(String),
}
