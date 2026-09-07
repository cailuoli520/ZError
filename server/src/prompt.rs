//! Prompt 构造：题型识别、答题 prompt + few-shot、同题判重、总结、URL 图片题。
//! 文案逐字移植自上游 server.rs / answerFewShot.ts / Home.vue / urlQuestion.ts。

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;

use crate::ai::ChatMessage;
use crate::answer::strip_markdown_code_block;
use crate::db::QuestionMatch;

// ---------------------------------------------------------------------------
// 题型
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionKind {
    Single,
    Multiple,
    Judgement,
    Completion,
}

impl QuestionKind {
    pub fn chinese_name(&self) -> &'static str {
        match self {
            QuestionKind::Single => "单选",
            QuestionKind::Multiple => "多选",
            QuestionKind::Judgement => "判断",
            QuestionKind::Completion => "填空",
        }
    }

    fn prompt_hint(&self) -> &'static str {
        match self {
            QuestionKind::Single => "这是单选题，只有一个正确答案。answer 只能写正确选项的完整文字（与【选项】原文一致），禁止写 A/B/C/D，也禁止写成「B. 传动角」这种带字母前缀的形式，应直接写「传动角」。",
            QuestionKind::Multiple => "这是多选题，可能有多个正确答案。answer 只写各正确选项的完整文字，用 ### 连接（顺序不限）。禁止写 ABD，禁止写「A. xxx###C. yyy」这种带字母前缀的形式。",
            QuestionKind::Judgement => "这是判断题。answer 只能是「正确」或「错误」二字之一。禁止写 A/B，禁止写「A. 正确」。",
            QuestionKind::Completion => "这是填空题。answer 只填空白处应填的内容本身；若有多个空，用 ### 连接。不要加第1空/①/A. 等序号前缀。",
        }
    }

    fn few_shot_block(&self) -> &'static str {
        match self {
            QuestionKind::Single => "【作答示例】\n题目：凸轮机构中从动件运动规律取决于（ ）。\n选项：A. 压力角  B. 传动角  C. 极力夹角\n正确输出：{\"answer\": \"传动角\"}\n错误输出：{\"answer\": \"B. 传动角\"} 或 {\"answer\": \"B\"}\n",
            QuestionKind::Multiple => "【作答示例】\n题目：下列属于输入设备的有（ ）。\n选项：A. 键盘  B. 显示器  C. 鼠标  D. 打印机\n正确输出：{\"answer\": \"键盘###鼠标\"}\n错误输出：{\"answer\": \"AC\"} 或 {\"answer\": \"A. 键盘###C. 鼠标\"}\n",
            QuestionKind::Judgement => "【作答示例】\n题目：地球绕太阳公转一周约为 365 天。\n正确输出：{\"answer\": \"正确\"}\n错误输出：{\"answer\": \"A\"} 或 {\"answer\": \"A. 正确\"}\n",
            QuestionKind::Completion => "【作答示例】\n题目：中国的首都是____，最大的城市是____。\n正确输出：{\"answer\": \"北京###上海\"}\n错误输出：{\"answer\": \"第1空：北京###第2空：上海\"}\n",
        }
    }
}

pub fn detect_question_kind(query_type: &str) -> Option<QuestionKind> {
    let trimmed = query_type.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.to_lowercase();
    if normalized.contains("single") || trimmed.contains("单选") || trimmed.contains("单项选择") {
        Some(QuestionKind::Single)
    } else if normalized.contains("multiple") || trimmed.contains("多选") || trimmed.contains("多项选择") {
        Some(QuestionKind::Multiple)
    } else if normalized.contains("judgement") || normalized.contains("judgment") || trimmed.contains("判断") {
        Some(QuestionKind::Judgement)
    } else if normalized.contains("completion") || trimmed.contains("填空") {
        Some(QuestionKind::Completion)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// 答题 prompt
// ---------------------------------------------------------------------------

pub fn build_model_query_prompt(title: &str, options: Option<&str>, query_type: Option<&str>) -> String {
    let mut q = String::from("你是一个专业的答题助手。请按以下要求作答：\n");
    q.push_str("1. 先在内部完成审题与推理（可简要），再给出最终答案。\n");
    q.push_str("2. 最后一行输出**唯一**一个 JSON 对象，不要用 markdown 代码块包裹，JSON 前后不要附加说明。\n");
    q.push_str("3. JSON 格式严格为：{\"answer\": \"最终答案\"}\n");
    q.push_str("4. answer 只写答案正文本身：选择题必须与【选项】中去掉「A.」「B.」后的文字完全一致；判断题只写「正确」或「错误」；填空题只写填空内容。\n");
    q.push_str("5. 严禁把选项字母写进 answer：不要写 A/B/C/D，不要写「B. 传动角」「D. meeting」，应直接写「传动角」「meeting」。不要把分析过程写进 answer。\n\n");

    if let Some(raw_type) = query_type.map(str::trim).filter(|v| !v.is_empty()) {
        if let Some(kind) = detect_question_kind(raw_type) {
            q.push_str(&format!("【题目类型：{}题】\n", kind.chinese_name()));
            q.push_str(&format!("提示：{}\n", kind.prompt_hint()));
            q.push_str(kind.few_shot_block());
            q.push('\n');
        } else {
            q.push_str(&format!("【题目类型字段：{}】\n", raw_type));
        }
    }

    q.push_str(&format!("【题目】\n{}\n", title));
    if let Some(options) = options.map(str::trim).filter(|v| !v.is_empty()) {
        q.push_str(&format!("【选项】\n{}\n", options));
    }
    q.push_str("\n请作答，并在最后输出答案 JSON：");
    q
}

// ---------------------------------------------------------------------------
// few-shot 多轮消息（answerFewShot.ts）
// ---------------------------------------------------------------------------

const SYSTEM_RULES: &str = "你是专业答题助手。最终只输出一行 JSON：{\"answer\":\"答案正文\"}。\n选择题 answer 必须是选项正文（与选项原文去掉 A./B. 后一致），禁止写 A/B/C/D，禁止写「B. 传动角」这种带字母前缀的形式。\n判断题 answer 只能是「正确」或「错误」。\n多选题多个答案用 ### 连接；填空题多空也用 ### 连接。";

const SINGLE_SHOTS: &[(&str, &str)] = &[
    (
        "【题目类型：单选题】\n【题目】\n凸轮机构中，从动件运动规律取决于（ ）。\n【选项】\nA. 压力角\nB. 传动角\nC. 极力夹角\n\n请作答，并在最后输出答案 JSON：",
        "{\"answer\":\"传动角\"}",
    ),
    (
        "【题目类型：单选题】\n【题目】\nThe gerund of the verb \"meet\" is ____.\n【选项】\nA. meet\nB. meeted\nC. to meet\nD. meeting\n\n请作答，并在最后输出答案 JSON：",
        "{\"answer\":\"meeting\"}",
    ),
];
const MULTIPLE_SHOTS: &[(&str, &str)] = &[(
    "【题目类型：多选题】\n【题目】\n下列属于输入设备的有（ ）。\n【选项】\nA. 键盘\nB. 显示器\nC. 鼠标\nD. 打印机\n\n请作答，并在最后输出答案 JSON：",
    "{\"answer\":\"键盘###鼠标\"}",
)];
const JUDGEMENT_SHOTS: &[(&str, &str)] = &[
    (
        "【题目类型：判断题】\n【题目】\n地球绕太阳公转一周约为 365 天。\n\n请作答，并在最后输出答案 JSON：",
        "{\"answer\":\"正确\"}",
    ),
    (
        "【题目类型：判断题】\n【题目】\n纯净水的 pH 值一定等于 7。\n\n请作答，并在最后输出答案 JSON：",
        "{\"answer\":\"错误\"}",
    ),
];
const COMPLETION_SHOTS: &[(&str, &str)] = &[(
    "【题目类型：填空题】\n【题目】\n中国的首都是____，最大的城市是____。\n\n请作答，并在最后输出答案 JSON：",
    "{\"answer\":\"北京###上海\"}",
)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FewShotKind {
    Single,
    Multiple,
    Judgement,
    Completion,
    General,
}

static RE_MULTI: Lazy<Regex> = Lazy::new(|| Regex::new(r"题目类型：\s*多选|(?i)multiple").expect("re"));
static RE_JUDGE: Lazy<Regex> = Lazy::new(|| Regex::new(r"题目类型：\s*判断|(?i)judg(?:e)?ment").expect("re"));
static RE_COMPLETION: Lazy<Regex> = Lazy::new(|| Regex::new(r"题目类型：\s*填空|(?i)completion").expect("re"));
static RE_SINGLE: Lazy<Regex> = Lazy::new(|| Regex::new(r"题目类型：\s*单选|(?i)single").expect("re"));

fn detect_few_shot_kind(query: &str) -> FewShotKind {
    if RE_MULTI.is_match(query) {
        FewShotKind::Multiple
    } else if RE_JUDGE.is_match(query) {
        FewShotKind::Judgement
    } else if RE_COMPLETION.is_match(query) {
        FewShotKind::Completion
    } else if RE_SINGLE.is_match(query) {
        FewShotKind::Single
    } else {
        FewShotKind::General
    }
}

/// 是否为普通答题 prompt（排除同题判断 / URL 题等）
fn should_attach_few_shot(query: &str) -> bool {
    let q = query.trim();
    if q.is_empty() || q.starts_with("__SAME_QUESTION_CHECK__:") || q.starts_with("__URL_QUESTION__:") {
        return false;
    }
    q.contains("【题目】") || q.contains("专业的答题助手") || q.contains("\"answer\"")
}

/// system 规则 → 例题 user/assistant 对 → 当前题目 user
pub fn build_answer_messages(query: &str) -> Vec<ChatMessage> {
    if !should_attach_few_shot(query) {
        return vec![ChatMessage::user(query)];
    }
    let shots: Vec<(&str, &str)> = match detect_few_shot_kind(query) {
        FewShotKind::Single => SINGLE_SHOTS.to_vec(),
        FewShotKind::Multiple => MULTIPLE_SHOTS.to_vec(),
        FewShotKind::Judgement => JUDGEMENT_SHOTS.to_vec(),
        FewShotKind::Completion => COMPLETION_SHOTS.to_vec(),
        FewShotKind::General => vec![SINGLE_SHOTS[0], JUDGEMENT_SHOTS[0]],
    };
    let mut messages = vec![ChatMessage::system(SYSTEM_RULES)];
    for (user, assistant) in shots {
        messages.push(ChatMessage::user(user));
        messages.push(ChatMessage::assistant(assistant));
    }
    messages.push(ChatMessage::user(query));
    messages
}

// ---------------------------------------------------------------------------
// 同题判重（Home.vue buildSameQuestionCheckPrompt / parseSameQuestionCheckResult）
// ---------------------------------------------------------------------------

pub const SAME_QUESTION_CANDIDATE_LIMIT: usize = 5;

pub fn build_same_question_check_prompt(title: &str, options: Option<&str>, candidates: &[QuestionMatch]) -> String {
    let candidate_lines = candidates
        .iter()
        .enumerate()
        .map(|(index, c)| {
            let options_text = c
                .options
                .as_deref()
                .filter(|o| !o.trim().is_empty())
                .map(|o| format!("\n选项：{o}"))
                .unwrap_or_default();
            format!("候选{}（id={}）：\n题干：{}{}", index + 1, c.id, c.question, options_text)
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let options_block = options
        .filter(|o| !o.trim().is_empty())
        .map(|o| format!("\n【新题选项】\n{o}\n"))
        .unwrap_or_default();
    [
        "你是题目判重助手。请判断「新题目」是否与下列候选中的某一道是同一道题（允许措辞略有差异，但题意与考点必须一致）。",
        "只输出一个 JSON 对象，不要代码块，不要其他文字。",
        "若是同一题：{\"same\":true,\"matched_id\":<候选id>}",
        "若都不是：{\"same\":false}",
        "",
        "【新题目】",
        title,
        &options_block,
        "【候选题目】",
        if candidate_lines.is_empty() { "（无候选）" } else { &candidate_lines },
    ]
    .join("\n")
}

/// 解析判重结果：same=true 且 matched_id 有效时返回 id
pub fn parse_same_question_result(content: &str) -> Option<i64> {
    let text = strip_markdown_code_block(content).trim().to_string();
    if text.is_empty() || crate::answer::is_model_error(&text).is_some() {
        return None;
    }
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end < start {
        return None;
    }
    let value: Value = serde_json::from_str(&text[start..=end]).ok()?;
    if value.get("same").and_then(|v| v.as_bool()) != Some(true) {
        return None;
    }
    let matched = value.get("matched_id")?;
    matched
        .as_i64()
        .or_else(|| matched.as_u64().map(|v| v as i64))
        .or_else(|| matched.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
}

// ---------------------------------------------------------------------------
// 总结 prompt（Home.vue）
// ---------------------------------------------------------------------------

pub fn build_summary_prompt(query: &str, combined: &str) -> String {
    format!(
        "你是一个总结专家。下面是用户的问题以及AI模型的回答。请根据回答内容，整理并总结出一个最准确、最全面的最终答案。\n\n用户原始问题：\n{query}\n\n模型回答内容：\n{combined}\n\n请直接给出最终总结答案："
    )
}

// ---------------------------------------------------------------------------
// URL 图片题（urlQuestion.ts + Home.vue analyzeUrlQuestion）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrlQuestionMode {
    Open,
    Single,
    Multiple,
    Judgement,
}

impl UrlQuestionMode {
    pub fn is_choice(&self) -> bool {
        matches!(self, UrlQuestionMode::Single | UrlQuestionMode::Multiple)
    }
}

static LETTER_LABEL: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*([A-Za-z])[\.、．]\s*(.*)$").expect("re"));
static NUMBER_LABEL: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*(\d+)[\.、．]\s*(.*)$").expect("re"));
static LINE_LABEL_PREFIX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z\d]+[\.、．]\s*").expect("re"));
static ANSWER_LINE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)ANSWER:\s*(.+)").expect("re"));
static SPLIT_SEP_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[,，、;\s]+").expect("re"));

/// 有序选项表：编号("1","2",…) → 正文
pub type UrlOptionMap = Vec<(String, String)>;

fn map_get<'a>(map: &'a UrlOptionMap, key: &str) -> Option<&'a String> {
    map.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// 从选项文本中解析带标签的选项块（支持块内换行）
fn parse_labeled_option_blocks(options_str: &str) -> Vec<String> {
    let text = options_str.replace("\r\n", "\n");
    let mut letter_blocks: Vec<String> = Vec::new();
    let mut number_blocks: Vec<String> = Vec::new();
    let mut letter_current: Option<Vec<String>> = None;
    let mut number_current: Option<Vec<String>> = None;
    let mut letter_count = 0;
    let mut number_count = 0;

    let flush = |cur: &mut Option<Vec<String>>, out: &mut Vec<String>| {
        if let Some(lines) = cur.take() {
            let t = lines.join("\n").trim().to_string();
            if !t.is_empty() {
                out.push(t);
            }
        }
    };

    for raw in text.split('\n') {
        let line = raw.trim();
        if let Some(m) = LETTER_LABEL.captures(line) {
            flush(&mut letter_current, &mut letter_blocks);
            letter_count += 1;
            letter_current = Some(vec![m.get(2).map(|x| x.as_str()).unwrap_or("").to_string()]);
            // 字母标签优先，打断数字块
            flush(&mut number_current, &mut number_blocks);
            continue;
        }
        if letter_current.is_none() {
            if let Some(m) = NUMBER_LABEL.captures(line) {
                flush(&mut number_current, &mut number_blocks);
                number_count += 1;
                number_current = Some(vec![m.get(2).map(|x| x.as_str()).unwrap_or("").to_string()]);
                continue;
            }
        }
        if let Some(cur) = letter_current.as_mut() {
            if !line.is_empty() {
                cur.push(line.to_string());
            }
            continue;
        }
        if let Some(cur) = number_current.as_mut() {
            if !line.is_empty() {
                cur.push(line.to_string());
            }
        }
    }
    flush(&mut letter_current, &mut letter_blocks);
    flush(&mut number_current, &mut number_blocks);

    if letter_count >= 2 && letter_blocks.len() >= 2 {
        return letter_blocks;
    }
    if number_count >= 2 && number_blocks.len() >= 2 {
        return number_blocks;
    }
    Vec::new()
}

fn parse_line_fallback_options(options_str: &str) -> Vec<String> {
    options_str
        .replace("\r\n", "\n")
        .split('\n')
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|line| {
            let stripped = LINE_LABEL_PREFIX.replace(line, "").trim().to_string();
            if stripped.is_empty() {
                line.to_string()
            } else {
                stripped
            }
        })
        .collect()
}

pub fn parse_url_options(options_str: &str, prefer_labeled_only: bool, allow_line_fallback: bool) -> UrlOptionMap {
    let mut map = UrlOptionMap::new();
    if options_str.trim().is_empty() {
        return map;
    }
    let labeled = parse_labeled_option_blocks(options_str);
    if labeled.len() >= 2 {
        for (i, t) in labeled.into_iter().enumerate() {
            map.push(((i + 1).to_string(), t));
        }
        return map;
    }
    if prefer_labeled_only || !allow_line_fallback {
        return map;
    }
    for (i, t) in parse_line_fallback_options(options_str).into_iter().enumerate() {
        map.push(((i + 1).to_string(), t));
    }
    map
}

pub fn classify_url_question_mode(question_type: Option<&str>, options_str: Option<&str>) -> UrlQuestionMode {
    let type_text = question_type.unwrap_or("").trim();
    let normalized = type_text.to_lowercase();
    if normalized.contains("completion")
        || type_text.contains("填空")
        || type_text.contains("简答")
        || type_text.contains("解答")
        || type_text.contains("主观")
    {
        return UrlQuestionMode::Open;
    }
    if normalized.contains("multiple") || type_text.contains("多选") || type_text.contains("多项选择") {
        return UrlQuestionMode::Multiple;
    }
    if normalized.contains("judgement") || normalized.contains("judgment") || type_text.contains("判断") {
        return UrlQuestionMode::Judgement;
    }
    if normalized.contains("single") || type_text.contains("单选") || type_text.contains("单项选择") {
        return UrlQuestionMode::Single;
    }
    // type 空或未知：仅当能解析出 ≥2 个带标签选项时视为单选
    if parse_url_options(options_str.unwrap_or(""), true, false).len() >= 2 {
        UrlQuestionMode::Single
    } else {
        UrlQuestionMode::Open
    }
}

pub fn build_url_option_map_for_mode(mode: UrlQuestionMode, options_str: Option<&str>) -> UrlOptionMap {
    if mode == UrlQuestionMode::Open {
        return UrlOptionMap::new();
    }
    parse_url_options(options_str.unwrap_or(""), false, mode.is_choice())
}

/// 组装发给视觉模型的完整文本（title + 选项 + 指令），与 Home.vue analyzeUrlQuestion 一致
pub fn build_url_question_text(
    title: &str,
    options: Option<&str>,
    question_type: Option<&str>,
    mode: UrlQuestionMode,
    option_map: &UrlOptionMap,
) -> String {
    let options_str = options.unwrap_or("");
    let type_text = question_type.unwrap_or("").trim();
    let type_hint = if type_text.is_empty() {
        String::new()
    } else {
        format!("\n【题目类型：{type_text}】\n")
    };
    let has_options = !option_map.is_empty();

    let options_text = if mode.is_choice() && has_options {
        format!(
            "\n\n选项：\n{}",
            option_map
                .iter()
                .map(|(k, v)| format!("{k}. {v}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    } else if mode == UrlQuestionMode::Open && !options_str.trim().is_empty() {
        format!("\n\n【补充材料】\n{}\n", options_str.trim())
    } else {
        String::new()
    };

    let answer_rule = if mode == UrlQuestionMode::Multiple && has_options {
        "- 这是多选题：必须列出全部正确选项编号，多个编号用空格分隔，如 ANSWER: 1 3\n- 也可以写成 ANSWER: A C（字母对应下方选项顺序）\n- 不要只选一个；禁止输出空答案或仅空格"
    } else if mode == UrlQuestionMode::Judgement {
        "- 判断题：写 正确 或 错误\n- 格式如 ANSWER: 正确"
    } else if mode == UrlQuestionMode::Single && has_options {
        "- 单选题：只写一个正确选项编号，如 ANSWER: 2；不要根据原始文本的空行号作答\n- 也可以写成 ANSWER: B（字母对应下方选项顺序）\n- 禁止输出空答案或仅空格"
    } else if mode.is_choice() && !has_options {
        "- 选择题但未能解析出选项列表：直接写出完整答案内容，如 ANSWER: ……"
    } else {
        "- 填空/简答/解答题：直接写完整答案内容，如 ANSWER: 42\n- 不要编造 A/B/C 选项编号；不要把补充材料当成选项列表"
    };

    let instruction = format!(
        "\n\n请仔细分析上述题目，给出详细的解题过程和答案。{type_hint}\n在回答末尾，严格按照以下格式单独一行给出答案：\nANSWER: <答案>\n\n其中规则：\n{answer_rule}"
    );
    format!("{title}{options_text}{instruction}")
}

/// 取最后一次 ANSWER: 后的原文
pub fn extract_answer_raw(response: &str) -> String {
    ANSWER_LINE_RE
        .captures_iter(response)
        .last()
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_default()
}

fn strip_url_label(text: &str) -> String {
    let t = text.trim();
    if let Some(c) = LABEL_LOOSE.captures(t) {
        if let Some(rest) = c.get(2).map(|m| m.as_str().trim()).filter(|r| !r.is_empty()) {
            return rest.to_string();
        }
    }
    if let Some(c) = LABEL_TIGHT.captures(t) {
        if let Some(rest) = c.get(2).map(|m| m.as_str().trim()).filter(|r| !r.is_empty()) {
            if !rest.chars().all(|ch| ch.is_ascii_alphabetic()) {
                return rest.to_string();
            }
        }
    }
    t.to_string()
}
static LABEL_LOOSE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([A-Za-z])[\.、．\)]\s+(.+)$").expect("re"));
static LABEL_TIGHT: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([A-Za-z])[\.、．\)](.+)$").expect("re"));

fn letter_to_num(ch: char) -> String {
    ((ch.to_ascii_uppercase() as u8 - b'A') as u32 + 1).to_string()
}

/// 按模式解析 ANSWER：open/judgement 返回原文；single/multiple 把编号映射为选项正文
pub fn resolve_url_answer(response: &str, option_map: &UrlOptionMap, mode: UrlQuestionMode) -> String {
    let raw = extract_answer_raw(response);
    if raw.is_empty() {
        return String::new();
    }
    if mode == UrlQuestionMode::Open {
        return raw;
    }
    if mode == UrlQuestionMode::Judgement || option_map.is_empty() {
        let stripped = strip_url_label(&raw);
        if mode == UrlQuestionMode::Judgement && !option_map.is_empty() {
            let mut chars = stripped.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                if c.is_ascii_alphabetic() {
                    return map_get(option_map, &letter_to_num(c)).cloned().unwrap_or(stripped);
                }
            }
        }
        return stripped;
    }

    // 整段已是「B. 选项正文」时，直接去前缀并校验
    let whole_stripped = strip_url_label(&raw);
    if whole_stripped != raw.trim() && option_map.iter().any(|(_, v)| v.trim() == whole_stripped) {
        return whole_stripped;
    }

    let expand = |token: &str| -> Vec<String> {
        let t = strip_url_label(token.trim());
        if t.is_empty() {
            return vec![];
        }
        let all_digit = t.chars().all(|c| c.is_ascii_digit());
        let all_alpha = t.chars().all(|c| c.is_ascii_alphabetic());
        let len = t.chars().count();
        if all_digit && len == 1 {
            return vec![t];
        }
        if all_alpha && len == 1 {
            return vec![letter_to_num(t.chars().next().unwrap_or('A'))];
        }
        if all_alpha && len >= 2 {
            return t.chars().map(letter_to_num).collect();
        }
        if all_digit && len >= 2 {
            let digits: Vec<String> = t.chars().map(|c| c.to_string()).collect();
            if digits.iter().all(|d| map_get(option_map, d).is_some()) {
                return digits;
            }
            return vec![t];
        }
        vec![t]
    };

    let replaced: String = raw.chars().map(|c| if matches!(c, '和' | '与' | '及') { ' ' } else { c }).collect();
    let mut nums: Vec<String> = Vec::new();
    for tok in SPLIT_SEP_RE.split(&replaced) {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        for n in expand(tok) {
            if !nums.contains(&n) {
                nums.push(n);
            }
        }
    }
    if nums.is_empty() {
        return whole_stripped;
    }
    let parts: Vec<String> = nums
        .iter()
        .filter_map(|n| map_get(option_map, n))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();
    if parts.is_empty() {
        return whole_stripped;
    }
    parts.join("###")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_kind_and_builds_prompt() {
        assert_eq!(detect_question_kind("single"), Some(QuestionKind::Single));
        assert_eq!(detect_question_kind("多项选择题"), Some(QuestionKind::Multiple));
        assert_eq!(detect_question_kind(""), None);
        let p = build_model_query_prompt("题干", Some("A. x\nB. y"), Some("judgement"));
        assert!(p.contains("【题目类型：判断题】"));
        assert!(p.contains("【选项】\nA. x\nB. y"));
        assert!(p.ends_with("请作答，并在最后输出答案 JSON："));
    }

    #[test]
    fn few_shot_messages() {
        let q = build_model_query_prompt("t", None, Some("多选"));
        let msgs = build_answer_messages(&q);
        assert_eq!(msgs[0].role, "system");
        assert_eq!(msgs.len(), 1 + 2 + 1);
        assert_eq!(build_answer_messages("__URL_QUESTION__:x").len(), 1);
        let general = build_answer_messages(&build_model_query_prompt("t", None, None));
        assert_eq!(general.len(), 1 + 4 + 1);
    }

    #[test]
    fn same_question_roundtrip() {
        let c = vec![QuestionMatch {
            id: 7,
            question: "Q".into(),
            options: Some("A. 1".into()),
            answer: "1".into(),
            is_ai: true,
            is_pending_correction: false,
            score: 0.9,
        }];
        let p = build_same_question_check_prompt("新题", None, &c);
        assert!(p.contains("候选1（id=7）"));
        assert_eq!(parse_same_question_result("```json\n{\"same\":true,\"matched_id\":7}\n```"), Some(7));
        assert_eq!(parse_same_question_result("{\"same\":true,\"matched_id\":\"8\"}"), Some(8));
        assert_eq!(parse_same_question_result("{\"same\":false}"), None);
        assert_eq!(parse_same_question_result("错误: x"), None);
    }

    #[test]
    fn url_question_flow() {
        let opts = "A. 苹果\nB. 香蕉\nC. 橘子";
        let mode = classify_url_question_mode(None, Some(opts));
        assert_eq!(mode, UrlQuestionMode::Single);
        let map = build_url_option_map_for_mode(mode, Some(opts));
        assert_eq!(map.len(), 3);
        assert_eq!(resolve_url_answer("分析...\nANSWER: B", &map, mode), "香蕉");
        let multi = UrlQuestionMode::Multiple;
        assert_eq!(resolve_url_answer("ANSWER: 1 3", &map, multi), "苹果###橘子");
        assert_eq!(resolve_url_answer("ANSWER: A和C", &map, multi), "苹果###橘子");
        assert_eq!(resolve_url_answer("ANSWER: 13", &map, multi), "苹果###橘子");
        assert_eq!(resolve_url_answer("ANSWER: 正确", &UrlOptionMap::new(), UrlQuestionMode::Judgement), "正确");
        assert_eq!(resolve_url_answer("没有答案行", &map, mode), "");
        let text = build_url_question_text("看图 http://x/a.png", Some(opts), Some("single"), mode, &map);
        assert!(text.contains("选项：\n1. 苹果"));
        assert!(text.contains("ANSWER: <答案>"));
    }
}
