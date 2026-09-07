//! 答案抽取与规范化、模型错误分类。逐函数移植自上游 server.rs 1384-1746，
//! 以及 Home.vue 的 stripMarkdownCodeBlock / getMostFrequentSuccessfulAnswer。

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

static OPTION_LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([A-Za-z])([\.、．\)])\s*(.+)$").expect("选项行正则"));
static LABEL_LOOSE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([A-Za-z])([\.、．\)])\s+(.+)$").expect("宽松前缀正则"));
static LABEL_TIGHT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([A-Za-z])([\.、．\)])(.+)$").expect("紧凑前缀正则"));
static ANSWER_JSON_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?s)\{\s*"(?:answer|anwser)"\s*:\s*"(.*?)"[\s\S]*?\}"#).expect("answer JSON 正则")
});
static ANSWER_TEXT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:答案|answer)[：:]\s*(.+?)(?:\n|$)").expect("答案文本正则"));
static CODE_FENCE_HEAD_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^```[\w]*\n?").expect("代码块头正则"));
static CODE_FENCE_TAIL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n?```$").expect("代码块尾正则"));
static WS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").expect("空白正则"));

/// 解析「A. 正文」形式的选项表：字母 → 正文
pub fn parse_option_letter_map(options: Option<&str>) -> HashMap<char, String> {
    let mut map = HashMap::new();
    let Some(options) = options.map(str::trim).filter(|s| !s.is_empty()) else {
        return map;
    };
    for raw in options.replace("\r\n", "\n").lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(caps) = OPTION_LINE_RE.captures(line) {
            let letter = caps
                .get(1)
                .and_then(|m| m.as_str().chars().next())
                .map(|c| c.to_ascii_uppercase());
            let sep = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let text = caps.get(3).map(|m| m.as_str().trim()).unwrap_or("");
            // 避免把正文「C、H、O、N…」误当成选项标号行
            if sep == "、"
                && text.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
                && text.chars().nth(1) == Some('、')
            {
                continue;
            }
            if let (Some(letter), true) = (letter, !text.is_empty()) {
                map.insert(letter, text.to_string());
            }
        }
    }
    map
}

/// 去掉行首选项字母前缀：`B. 传动角` → `传动角`
/// 有选项表时，仅当去前缀后能对应到某选项正文才剥离。
pub fn strip_leading_option_label(text: &str, map: &HashMap<char, String>) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let patterns: [(&Regex, bool); 2] = [(&LABEL_LOOSE_RE, false), (&LABEL_TIGHT_RE, true)];
    for (re, tight) in patterns {
        let Some(caps) = re.captures(trimmed) else {
            continue;
        };
        let Some(letter) = caps
            .get(1)
            .and_then(|m| m.as_str().chars().next())
            .map(|c| c.to_ascii_uppercase())
        else {
            continue;
        };
        let sep = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let rest = caps.get(3).map(|m| m.as_str().trim()).unwrap_or("");
        if rest.is_empty() {
            continue;
        }
        // 无空格且剩余全是字母：留给纯字母映射
        if tight && rest.chars().all(|c| c.is_ascii_alphabetic()) && rest.len() <= 8 {
            continue;
        }
        if map.is_empty() {
            if sep == "、" {
                continue;
            }
            return rest.to_string();
        }
        if map.get(&letter).map(|s| s.as_str()) == Some(rest) {
            return rest.to_string();
        }
        if map.values().any(|v| v == rest) {
            return rest.to_string();
        }
    }
    trimmed.to_string()
}

/// 将模型/题库答案规范为选项正文（去字母前缀，纯字母映射为正文并以 ### 连接）
pub fn normalize_answer_against_options(answer: &str, options: Option<&str>) -> String {
    let raw = answer.trim();
    if raw.is_empty() {
        return String::new();
    }
    let map = parse_option_letter_map(options);
    let parts: Vec<&str> = if raw.contains("###") {
        raw.split("###").collect()
    } else {
        vec![raw]
    };

    let mut normalized: Vec<String> = Vec::new();
    for part in parts {
        let mut p = strip_leading_option_label(part.trim(), &map);
        if p.is_empty() {
            continue;
        }
        let compact: String = p.chars().filter(|c| !c.is_whitespace()).collect();
        if !map.is_empty()
            && !compact.is_empty()
            && compact.chars().all(|c| c.is_ascii_alphabetic())
            && compact.len() <= 8
        {
            let letters: Vec<char> = compact.chars().map(|c| c.to_ascii_uppercase()).collect();
            let texts: Vec<String> = letters.iter().filter_map(|ch| map.get(ch).cloned()).collect();
            if texts.len() == letters.len() && !texts.is_empty() {
                p = texts.join("###");
            }
        }
        normalized.push(p);
    }

    if normalized.len() == 1 && normalized[0].contains("###") && !raw.contains("###") {
        return normalized[0].clone();
    }
    normalized.join("###")
}

/// 去掉 markdown 代码块围栏（```json ... ```）
pub fn strip_markdown_code_block(content: &str) -> String {
    let s = CODE_FENCE_HEAD_RE.replace(content, "");
    CODE_FENCE_TAIL_RE.replace(&s, "").to_string()
}

fn strip_fences_simple(text: &str) -> String {
    let mut cleaned = text.trim().to_string();
    if let Some(rest) = cleaned.strip_prefix("```json") {
        cleaned = rest.to_string();
    } else if let Some(rest) = cleaned.strip_prefix("```") {
        cleaned = rest.to_string();
    }
    if let Some(rest) = cleaned.strip_suffix("```") {
        cleaned = rest.to_string();
    }
    cleaned.trim().to_string()
}

/// 从文本末尾向前找最后一个括号平衡的 JSON 对象
pub fn extract_last_balanced_json(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut end: Option<usize> = None;
    let mut depth: i32 = 0;
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        let b = bytes[i];
        match end {
            None => {
                if b == b'}' {
                    end = Some(i);
                    depth = 1;
                }
            }
            Some(end_idx) => {
                if b == b'}' {
                    depth += 1;
                } else if b == b'{' {
                    depth -= 1;
                    if depth == 0 {
                        return text.get(i..=end_idx).map(|s| s.to_string());
                    }
                }
            }
        }
    }
    None
}

fn extract_field_from_value(v: &Value) -> Option<String> {
    v.get("answer")
        .and_then(|a| a.as_str())
        .or_else(|| v.get("anwser").and_then(|a| a.as_str()))
        .map(|s| s.to_string())
}

/// 从模型输出中提取 answer 字段（七级回退）
pub fn extract_answer_from_json(json_content: &str) -> String {
    // 1) 去除 markdown 代码块
    let cleaned = strip_fences_simple(json_content);

    // 2) 整体解析
    if let Ok(v) = serde_json::from_str::<Value>(&cleaned) {
        if let Some(ans) = extract_field_from_value(&v) {
            return ans;
        }
    }

    // 3) 末尾平衡 JSON 片段
    if let Some(json_str) = extract_last_balanced_json(&cleaned) {
        if let Ok(v) = serde_json::from_str::<Value>(&json_str) {
            if let Some(ans) = extract_field_from_value(&v) {
                return ans;
            }
        }
    }

    // 4) 正则捕获 answer 字段
    if let Some(caps) = ANSWER_JSON_RE.captures(&cleaned) {
        if let Some(m) = caps.get(1) {
            return m.as_str().to_string();
        }
    }

    // 5) 「答案:」/「answer:」后文本
    if let Some(caps) = ANSWER_TEXT_RE.captures(&cleaned) {
        if let Some(m) = caps.get(1) {
            let ans = m.as_str().trim();
            if !ans.is_empty() {
                return ans.to_string();
            }
        }
    }

    // 6) 简短纯文本直接作为答案
    let t = cleaned.trim();
    if !t.is_empty() && t.len() < 2000 && !t.starts_with('{') && !t.starts_with('[') {
        let lines = t.lines().filter(|l| !l.trim().is_empty()).count();
        if lines <= 3 {
            return t.to_string();
        }
    }

    // 7) 回退原文
    json_content.to_string()
}

pub fn is_timeout_like_model_failure(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("timeout")
        || lower.contains("超时")
        || lower.contains("no new tokens")
        || lower.contains("aborted")
        || lower.contains("abort")
        || lower.contains("cancelled")
        || lower.contains("canceled")
        || lower.contains("取消")
        || lower.contains("服务已停止")
}

/// 模型失败文本 → HTTP 状态：未选择模型 400，超时类 408，其它 500
pub fn classify_model_failure(text: &str) -> Option<(u16, String)> {
    let err = is_model_error(text)?;
    if err.contains("未选择模型") || text.contains("未选择模型") {
        Some((400, "错误: 未选择模型".to_string()))
    } else if is_timeout_like_model_failure(&err) || is_timeout_like_model_failure(text) {
        Some((408, err))
    } else {
        Some((500, err))
    }
}

/// 判断模型输出是否为错误文本，返回错误消息
pub fn is_model_error(text: &str) -> Option<String> {
    let cleaned = strip_fences_simple(text);
    if cleaned == "所有AI均查询失败" {
        return Some(cleaned);
    }
    if cleaned.starts_with("错误:")
        || cleaned.starts_with("错误：")
        || cleaned.starts_with("Error:")
        || cleaned.starts_with("API 错误")
    {
        return Some(cleaned);
    }
    if cleaned.contains("\"error\"") {
        let try_extract = |v: &Value| -> Option<String> {
            let err = v.get("error")?;
            Some(
                err.get("message")
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| err.to_string()),
            )
        };
        if let Ok(v) = serde_json::from_str::<Value>(&cleaned) {
            if let Some(msg) = try_extract(&v) {
                return Some(msg);
            }
        }
        if let Some(json_str) = extract_last_balanced_json(&cleaned) {
            if let Ok(v) = serde_json::from_str::<Value>(&json_str) {
                if let Some(msg) = try_extract(&v) {
                    return Some(msg);
                }
            }
        }
    }
    None
}

fn normalize_answer_for_comparison(content: &str) -> String {
    WS_RE
        .replace_all(&strip_markdown_code_block(content), " ")
        .trim()
        .to_string()
}

/// 多数投票：出现次数最多的答案；平局取最先出现的
pub fn get_most_frequent_answer(responses: &[String]) -> Option<String> {
    struct Stat {
        count: usize,
        first_index: usize,
        original: String,
    }
    let mut stats: Vec<(String, Stat)> = Vec::new();
    for (index, response) in responses.iter().enumerate() {
        let original = strip_markdown_code_block(response).trim().to_string();
        let key = normalize_answer_for_comparison(&original);
        if key.is_empty() {
            continue;
        }
        if let Some((_, s)) = stats.iter_mut().find(|(k, _)| *k == key) {
            s.count += 1;
        } else {
            stats.push((key, Stat { count: 1, first_index: index, original }));
        }
    }
    let mut selected: Option<&Stat> = None;
    for (_, stat) in &stats {
        let better = match selected {
            None => true,
            Some(cur) => stat.count > cur.count || (stat.count == cur.count && stat.first_index < cur.first_index),
        };
        if better {
            selected = Some(stat);
        }
    }
    selected.map(|s| s.original.clone()).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_letter_answers_to_option_text() {
        let opts = Some("A. 压力角\nB. 传动角\nC. 极力夹角");
        assert_eq!(normalize_answer_against_options("B", opts), "传动角");
        assert_eq!(normalize_answer_against_options("B. 传动角", opts), "传动角");
        assert_eq!(normalize_answer_against_options("AC", opts), "压力角###极力夹角");
        assert_eq!(normalize_answer_against_options("A. 压力角###C. 极力夹角", opts), "压力角###极力夹角");
        assert_eq!(normalize_answer_against_options("正确", None), "正确");
    }

    #[test]
    fn extracts_answer_with_fallbacks() {
        assert_eq!(extract_answer_from_json(r#"{"answer":"北京"}"#), "北京");
        assert_eq!(extract_answer_from_json("分析过程...\n{\"answer\": \"上海\"}"), "上海");
        assert_eq!(extract_answer_from_json("```json\n{\"anwser\":\"x\"}\n```"), "x");
        assert_eq!(extract_answer_from_json("答案: 42"), "42");
        assert_eq!(extract_answer_from_json("直接答案"), "直接答案");
    }

    #[test]
    fn classifies_failures() {
        assert_eq!(classify_model_failure("错误: 未选择模型").map(|c| c.0), Some(400));
        assert_eq!(classify_model_failure("错误: 模型响应超时（40 秒）").map(|c| c.0), Some(408));
        assert_eq!(classify_model_failure("错误: HTTP 500").map(|c| c.0), Some(500));
        assert!(classify_model_failure("{\"answer\":\"ok\"}").is_none());
        assert_eq!(
            is_model_error(r#"{"error":{"message":"bad key"}}"#),
            Some("bad key".to_string())
        );
    }

    #[test]
    fn majority_vote_prefers_count_then_order() {
        let v = vec!["A".to_string(), "B".to_string(), "```json\nB\n```".to_string()];
        assert_eq!(get_most_frequent_answer(&v), Some("B".to_string()));
        let tie = vec!["X".to_string(), "Y".to_string()];
        assert_eq!(get_most_frequent_answer(&tie), Some("X".to_string()));
        assert_eq!(get_most_frequent_answer(&[]), None);
    }
}
