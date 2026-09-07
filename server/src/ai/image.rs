//! 图片抓取（带缓存与 SSRF 防护）与视觉预处理（白底、放大）。

use std::net::IpAddr;
use std::path::Path;
use std::time::Duration;

use base64::Engine;
use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};
use once_cell::sync::Lazy;
use regex::Regex;
use sha2::{Digest, Sha256};

const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

/// 与前端 questionImage.ts 默认算法一致的 URL 正则
static IMAGE_URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://[A-Za-z0-9\-._~:/#@!$&*+,;%]+(?:\?[A-Za-z0-9\-._~:/#@!$&*+,;%]*=[A-Za-z0-9\-._~:/#@!$&*+,;=%]*)?")
        .expect("图片 URL 正则")
});

fn normalize_question_url(raw: &str) -> String {
    raw.trim().trim_end_matches(|c: char| matches!(c, '.' | ',' | ';' | '!' | '?')).to_string()
}

/// 文本中的 URL 片段（起止字节位置与规范化 URL）
#[derive(Debug, Clone, PartialEq)]
pub struct UrlMatch {
    pub start: usize,
    pub end: usize,
    pub url: String,
}

pub fn find_image_url_matches(text: &str) -> Vec<UrlMatch> {
    IMAGE_URL_RE
        .find_iter(text)
        .filter_map(|m| {
            let url = normalize_question_url(m.as_str());
            if url.is_empty() {
                return None;
            }
            // 规范化可能去掉了末尾标点，end 按规范化后的长度回退
            let end = m.start() + url.len().min(m.as_str().len());
            Some(UrlMatch { start: m.start(), end, url })
        })
        .collect()
}

#[cfg(test)]
pub fn extract_image_urls(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for m in find_image_url_matches(text) {
        if !out.contains(&m.url) {
            out.push(m.url);
        }
    }
    out
}

fn is_public_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || o[0] == 0
                || (o[0] == 100 && (64..=127).contains(&o[1]))
                || (o[0] == 192 && o[1] == 0 && o[2] == 0))
        }
        IpAddr::V6(v6) => {
            let seg = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || (seg[0] & 0xfe00) == 0xfc00
                || (seg[0] & 0xffc0) == 0xfe80
                || v6.to_ipv4_mapped().map(|m| !is_public_ip(&IpAddr::V4(m))).unwrap_or(false))
        }
    }
}

/// SSRF 防护：主机必须解析到公网地址
async fn ensure_public_host(url: &url::Url) -> Result<(), String> {
    let host = url.host_str().ok_or("图片 URL 缺少主机名")?;
    if host.eq_ignore_ascii_case("localhost") {
        return Err("拒绝访问本机地址".into());
    }
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
        return if is_public_ip(&ip) { Ok(()) } else { Err("拒绝访问内网地址".into()) };
    }
    let port = url.port_or_known_default().unwrap_or(80);
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("解析主机失败: {e}"))?;
    let mut any = false;
    for addr in addrs {
        any = true;
        if !is_public_ip(&addr.ip()) {
            return Err("拒绝访问内网地址".into());
        }
    }
    if !any {
        return Err("主机无法解析".into());
    }
    Ok(())
}

fn detect_image_type(bytes: &[u8]) -> &'static str {
    match image::guess_format(bytes) {
        Ok(ImageFormat::Png) => "image/png",
        Ok(ImageFormat::Jpeg) => "image/jpeg",
        Ok(ImageFormat::Gif) => "image/gif",
        Ok(ImageFormat::WebP) => "image/webp",
        Ok(ImageFormat::Bmp) => "image/bmp",
        _ => "image/png",
    }
}

fn cache_path(cache_dir: &Path, url: &str) -> std::path::PathBuf {
    let digest = Sha256::digest(url.as_bytes());
    cache_dir.join(format!("{}.b64", hex::encode(digest)))
}

/// 抓取图片并转为 data URL（三种请求头策略依次尝试；磁盘缓存）
pub async fn fetch_image_as_data_url(http: &reqwest::Client, cache_dir: &Path, url: &str) -> Result<String, String> {
    let url = url.trim();
    if url.starts_with("data:image/") {
        return Ok(url.to_string());
    }
    let parsed = url::Url::parse(url).map_err(|e| format!("图片 URL 无效: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("仅支持 http/https 图片".into());
    }

    let _ = std::fs::create_dir_all(cache_dir);
    let cache_file = cache_path(cache_dir, url);
    if let Ok(cached) = std::fs::read_to_string(&cache_file) {
        if !cached.is_empty() {
            return Ok(cached);
        }
    }

    ensure_public_host(&parsed).await?;

    let origin = format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or(""));
    let strategies: [(&str, Vec<(&str, String)>); 3] = [
        (
            "完整浏览器伪装",
            vec![
                ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36".into()),
                ("Accept", "image/avif,image/webp,image/apng,image/*,*/*;q=0.8".into()),
                ("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8".into()),
                ("Referer", format!("{origin}/")),
                ("Sec-Fetch-Dest", "image".into()),
                ("Sec-Fetch-Mode", "no-cors".into()),
                ("Sec-Fetch-Site", "same-origin".into()),
            ],
        ),
        (
            "简化请求头",
            vec![("User-Agent", "Mozilla/5.0".into()), ("Accept", "image/*,*/*".into())],
        ),
        (
            "移动端伪装",
            vec![
                ("User-Agent", "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".into()),
                ("Accept", "image/*,*/*;q=0.8".into()),
                ("Referer", format!("{origin}/")),
            ],
        ),
    ];

    let mut last_error = String::from("所有请求策略都失败了");
    for (name, headers) in strategies.iter() {
        let mut req = http.get(url).timeout(FETCH_TIMEOUT);
        for (k, v) in headers {
            req = req.header(*k, v.as_str());
        }
        match req.send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    last_error = format!("HTTP {} ({name})", resp.status());
                    continue;
                }
                if let Some(len) = resp.content_length() {
                    if len as usize > MAX_IMAGE_BYTES {
                        return Err("图片超过 8MB 上限".into());
                    }
                }
                match resp.bytes().await {
                    Ok(bytes) => {
                        if bytes.len() > MAX_IMAGE_BYTES {
                            return Err("图片超过 8MB 上限".into());
                        }
                        if bytes.is_empty() {
                            last_error = format!("响应为空 ({name})");
                            continue;
                        }
                        let mime = detect_image_type(&bytes);
                        let data_url = format!("data:{};base64,{}", mime, base64::engine::general_purpose::STANDARD.encode(&bytes));
                        let _ = std::fs::write(&cache_file, &data_url);
                        return Ok(data_url);
                    }
                    Err(e) => {
                        last_error = format!("读取图片失败 ({name}): {e}");
                    }
                }
            }
            Err(e) => {
                last_error = format!("网络请求失败 ({name}): {e}");
            }
        }
    }
    Err(last_error)
}

fn decode_data_url(data_url: &str) -> Option<DynamicImage> {
    let rest = data_url.strip_prefix("data:")?;
    let (_, b64) = rest.split_once(";base64,")?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).ok()?;
    image::load_from_memory(&bytes).ok()
}

fn encode_png_data_url(img: &RgbaImage) -> Result<String, String> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png).map_err(|e| format!("图片编码失败: {e}"))?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(buf.into_inner())
    ))
}

/// 视觉预处理：透明合成白底；短边 < min_side 时等比放大。非图片或解码失败时原样返回。
pub fn preprocess_for_vision(data_url: &str, min_side: u32) -> String {
    if !data_url.starts_with("data:image/") {
        return data_url.to_string();
    }
    let Some(img) = decode_data_url(data_url) else {
        return data_url.to_string();
    };
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return data_url.to_string();
    }
    let has_alpha = img.color().has_alpha();
    let needs_upscale = w < min_side || h < min_side;
    if !has_alpha && !needs_upscale {
        return data_url.to_string();
    }

    let mut rgba = img.to_rgba8();
    if has_alpha {
        for p in rgba.pixels_mut() {
            let a = p[3] as u32;
            if a < 255 {
                let blend = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
                *p = Rgba([blend(p[0]), blend(p[1]), blend(p[2]), 255]);
            }
        }
    }
    if needs_upscale {
        let scale = f64::max(min_side as f64 / w as f64, min_side as f64 / h as f64).max(1.0);
        let tw = ((w as f64 * scale).ceil() as u32).max(min_side);
        let th = ((h as f64 * scale).ceil() as u32).max(min_side);
        rgba = image::imageops::resize(&rgba, tw, th, image::imageops::FilterType::Triangle);
    }
    encode_png_data_url(&rgba).unwrap_or_else(|_| data_url.to_string())
}

/// 从供应商错误消息中提取需要的最小边长（上游 code 20015 / height(x) or width(y)）
pub fn extract_vision_image_size_error(message: &str) -> Option<u32> {
    static RE_HIT: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)(code"?\s*:\s*20015|height\(\d+\)\s*or\s*width\(\d+\))"#).expect("re"));
    static RE_SIZE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)height\((\d+)\)\s*or\s*width\((\d+)\)").expect("re"));
    if !RE_HIT.is_match(message) {
        return None;
    }
    let Some(c) = RE_SIZE.captures(message) else {
        return Some(32);
    };
    let h: u32 = c.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(32);
    let w: u32 = c.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(32);
    Some((h.max(w) + 16).clamp(32, 96))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_urls() {
        let urls = extract_image_urls("看图 https://a.com/x.png。再看 https://a.com/x.png 和 http://b.com/y.jpg?x=1");
        assert_eq!(urls, vec!["https://a.com/x.png", "http://b.com/y.jpg?x=1"]);
    }

    #[test]
    fn public_ip_check() {
        assert!(!is_public_ip(&"127.0.0.1".parse().unwrap()));
        assert!(!is_public_ip(&"10.1.2.3".parse().unwrap()));
        assert!(!is_public_ip(&"169.254.1.1".parse().unwrap()));
        assert!(!is_public_ip(&"::1".parse().unwrap()));
        assert!(!is_public_ip(&"fd00::1".parse().unwrap()));
        assert!(is_public_ip(&"8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn preprocess_upscales_and_whitens() {
        // 2x2 透明 PNG
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 0]));
        let url = encode_png_data_url(&img).unwrap();
        let out = preprocess_for_vision(&url, 32);
        let decoded = decode_data_url(&out).unwrap();
        assert!(decoded.width() >= 32 && decoded.height() >= 32);
        let px = decoded.to_rgba8().get_pixel(0, 0).0;
        assert_eq!(px, [255, 255, 255, 255]);
        assert_eq!(preprocess_for_vision("not-an-image", 32), "not-an-image");
    }

    #[test]
    fn size_error_detection() {
        assert_eq!(extract_vision_image_size_error("\"code\": 20015"), Some(32));
        assert_eq!(extract_vision_image_size_error("height(40) or width(50) too small"), Some(66));
        assert_eq!(extract_vision_image_size_error("other"), None);
    }
}
