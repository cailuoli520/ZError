//! 题目匹配评分：URL 归一化 + Levenshtein 字符相似度 + jieba 关键词覆盖率。
//! 逐函数移植自上游 database.rs，算法保持一致。

use jieba_rs::Jieba;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use strsim::normalized_levenshtein;

pub static JIEBA: Lazy<Jieba> = Lazy::new(Jieba::new);
static URL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"https?://[^\s]+").expect("URL 正则"));

/// 提取字符串中所有 URL（http/https），返回排序后的列表
pub fn extract_urls(text: &str) -> Vec<String> {
    let mut urls: Vec<String> = URL_RE
        .find_iter(text)
        .map(|m| {
            // 去掉末尾可能粘连的中文标点
            m.as_str()
                .trim_end_matches(|c: char| {
                    matches!(
                        c,
                        '，' | '。' | '！' | '？' | '、' | '；' | '：' | '\u{300c}'
                            ..='\u{300f}' | '（' | '）' | '【' | '】'
                    )
                })
                .to_string()
        })
        .collect();
    urls.sort();
    urls
}

/// 文本是否包含 URL
pub fn contains_url(text: &str) -> bool {
    URL_RE.is_match(text)
}

/// 将字符串中所有 URL 替换为统一占位符，用于相似度比较
fn normalize_urls(text: &str) -> String {
    URL_RE.replace_all(text, "__URL__").to_string()
}

pub const QUERY_STOPWORDS: &[&str] = &[
    "的", "地", "得", "了", "着", "吗", "呢", "啊", "呀", "吧", "么", "嘛", "在", "是", "和", "与",
    "及", "或", "并", "且", "将", "把", "被", "由", "对", "于", "中", "上", "下", "请问", "哪里",
    "哪儿", "哪个", "哪种", "哪项", "哪些", "什么", "怎么", "怎样", "如何", "为何", "为什么", "多少",
    "几", "一下", "以下", "下列", "题目", "选项", "答案", "内容", "说法", "图片", "图中", "名字",
    "名称", "城市", "国家", "地区", "地方",
];

const QUESTION_AND_OPTIONS_MATCH_KEYWORDS: &[&str] = &["以下", "下列", "下面", "下叙"];

/// 题干含「以下/下列/下面/下叙」时，非精确命中必须同时匹配选项
pub fn should_require_option_match(title: &str) -> bool {
    QUESTION_AND_OPTIONS_MATCH_KEYWORDS
        .iter()
        .any(|keyword| title.contains(keyword))
}

pub fn normalize_optional_query_text(text: Option<&str>) -> Option<String> {
    text.map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| text.to_string())
}

fn is_punctuation_or_space(c: char) -> bool {
    c.is_whitespace()
        || c.is_ascii_punctuation()
        || matches!(
            c,
            '，' | '。' | '！' | '？' | '、' | '；' | '：' | '（' | '）' | '【' | '】' | '《' | '》'
                | '“' | '”' | '‘' | '’' | '—' | '…' | '·'
        )
}

fn is_meaningful_query_token(token: &str) -> bool {
    let trimmed = token.trim();
    if trimmed.is_empty() || QUERY_STOPWORDS.contains(&trimmed) {
        return false;
    }
    if trimmed.chars().all(is_punctuation_or_space) {
        return false;
    }
    let char_count = trimmed.chars().count();
    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    if trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
        return char_count > 1;
    }
    char_count > 1
}

fn extract_query_keywords(text: &str) -> HashSet<String> {
    JIEBA
        .cut_for_search(text, false)
        .into_iter()
        .map(|token| token.trim().to_lowercase())
        .filter(|token| is_meaningful_query_token(token))
        .collect()
}

fn keyword_coverage(query_keywords: &HashSet<String>, candidate_keywords: &HashSet<String>) -> f64 {
    if query_keywords.is_empty() {
        return 1.0;
    }
    let matched = query_keywords
        .iter()
        .filter(|token| candidate_keywords.contains(*token))
        .count();
    matched as f64 / query_keywords.len() as f64
}

pub fn min_keyword_coverage(query_keywords_len: usize, char_similarity: f64) -> f64 {
    if query_keywords_len <= 2 {
        return 1.0;
    }
    // 高字符相似但关键词数量不多时，要求所有关键词都命中，避免核心词不同的题目误匹配
    if char_similarity >= 0.88 && query_keywords_len <= 8 {
        return 1.0;
    }
    if query_keywords_len <= 4 {
        return 1.0;
    }
    if query_keywords_len <= 8 {
        return 0.9;
    }
    0.75
}

/// 综合相似度：None 表示不匹配；1.0 表示规范化后完全相同
pub fn compute_query_match_score(query: &str, candidate: &str) -> Option<f64> {
    let normalized_query = normalize_urls(query).trim().to_lowercase();
    let normalized_candidate = normalize_urls(candidate).trim().to_lowercase();

    if normalized_query.is_empty() || normalized_candidate.is_empty() {
        return None;
    }
    if normalized_query == normalized_candidate {
        return Some(1.0);
    }

    let char_similarity = normalized_levenshtein(&normalized_query, &normalized_candidate);
    if char_similarity < 0.72 {
        return None;
    }

    let query_keywords = extract_query_keywords(&normalized_query);
    if query_keywords.is_empty() {
        return Some(char_similarity);
    }

    let candidate_keywords = extract_query_keywords(&normalized_candidate);
    let coverage = keyword_coverage(&query_keywords, &candidate_keywords);
    let min_coverage = min_keyword_coverage(query_keywords.len(), char_similarity);
    if coverage + f64::EPSILON < min_coverage {
        return None;
    }

    Some(char_similarity * 0.7 + coverage * 0.3)
}

pub fn is_exact_match_score(score: f64) -> bool {
    (score - 1.0).abs() <= f64::EPSILON
}

/// 搜索分词（cut_for_search），去空白与空 token
pub fn segment_text(text: &str) -> Vec<String> {
    JIEBA
        .cut_for_search(text, false)
        .into_iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_same_template_with_different_entity() {
        assert!(compute_query_match_score("韩国的首都在哪里", "美国的首都在哪里").is_none());
    }

    #[test]
    fn keeps_same_question_with_small_wording_changes() {
        assert!(compute_query_match_score("韩国的首都在哪里", "韩国首都是哪里").is_some());
    }

    #[test]
    fn keeps_exact_match() {
        assert_eq!(
            compute_query_match_score("韩国的首都在哪里", "韩国的首都在哪里"),
            Some(1.0)
        );
    }

    #[test]
    fn exact_match_score_is_detected() {
        assert!(is_exact_match_score(1.0));
        assert!(!is_exact_match_score(0.999));
    }

    #[test]
    fn urls_are_extracted_and_sorted() {
        let urls = extract_urls("看图 https://b.com/x.png， 以及 https://a.com/y.jpg。");
        assert_eq!(urls, vec!["https://a.com/y.jpg", "https://b.com/x.png"]);
        assert!(contains_url("http://x"));
        assert!(!contains_url("无链接"));
    }
}
