//! 保存した写真の検索（言葉・色）。

use crate::bing;
use crate::colors::{ColorBucket, MATCH_THRESHOLD};
use crate::library::StoredImage;

/// 検索条件。言葉はスペース区切りのすべてを含むもの、色はその色を一定以上含むもの。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Query {
    pub text: String,
    pub color: Option<ColorBucket>,
}

impl Query {
    pub fn is_active(&self) -> bool {
        !self.text.trim().is_empty() || self.color.is_some()
    }

    fn terms(&self) -> Vec<String> {
        self.text.split_whitespace().map(normalize).collect()
    }
}

/// 大文字小文字と全角英数字の違いを無視して比べるための正規化。
fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            // 全角英数字・記号 → 半角
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            '\u{3000}' => ' ',
            _ => c,
        })
        .flat_map(char::to_lowercase)
        .collect()
}

/// 検索の対象になる文字列（全言語の見出し・説明・解説文、地域名、日付）。
fn haystack(image: &StoredImage) -> String {
    let mut parts: Vec<String> = vec![image.title.clone(), image.copyright.clone()];
    for (title, copyright) in image.localized.values() {
        parts.push(title.clone());
        parts.push(copyright.clone());
    }
    parts.extend(image.descriptions.values().cloned());
    for market in &image.markets {
        parts.push(market.clone());
        parts.push(bing::market_name(market));
    }
    let d = &image.startdate;
    parts.push(d.clone());
    if d.len() == 8 {
        parts.push(format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8]));
        parts.push(format!("{}/{}/{}", &d[0..4], &d[4..6], &d[6..8]));
    }
    parts.push(bing::date_text(d, true));
    normalize(&parts.join("\n"))
}

/// 色の割合（%）。まだ計算していなければ None。
pub fn color_share(image: &StoredImage, color: ColorBucket) -> Option<u8> {
    image.colors.as_ref().and_then(|c| c.get(color.index()).copied())
}

/// 条件に合う写真を返す。色を指定したときはその色が多い順、それ以外は新しい順。
pub fn filter<'a>(images: &'a [StoredImage], query: &Query) -> Vec<&'a StoredImage> {
    let terms = query.terms();
    let mut hits: Vec<&StoredImage> = images
        .iter()
        .filter(|image| {
            if let Some(color) = query.color {
                if color_share(image, color).is_none_or(|share| share < MATCH_THRESHOLD) {
                    return false;
                }
            }
            if terms.is_empty() {
                return true;
            }
            let text = haystack(image);
            terms.iter().all(|t| text.contains(t.as_str()))
        })
        .collect();
    if let Some(color) = query.color {
        hits.sort_by_key(|image| std::cmp::Reverse(color_share(image, color).unwrap_or(0)));
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn image(title: &str, description: &str, colors: Vec<u8>) -> StoredImage {
        let mut descriptions = BTreeMap::new();
        descriptions.insert("ja-JP".to_string(), description.to_string());
        serde_json::from_value::<StoredImage>(serde_json::json!({
            "key": title, "startdate": "20261108", "title": title, "copyright": "",
            "urlbase": "", "markets": ["ja-JP"], "descriptions": descriptions, "colors": colors,
        }))
        .unwrap()
    }

    #[test]
    fn matches_words_and_colors() {
        let images = vec![
            image("ペンギン", "南極の氷の上", vec![0, 0, 0, 0, 0, 30, 0, 0, 50, 0]),
            image("紅葉", "Ｋｙｏｔｏ の秋", vec![60, 20, 0, 0, 0, 0, 0, 0, 0, 0]),
        ];
        let q = |text: &str, color| Query { text: text.into(), color };
        assert_eq!(filter(&images, &q("南極", None)).len(), 1);
        // 全角英字・大文字小文字の違いを無視する
        assert_eq!(filter(&images, &q("kyoto", None))[0].title, "紅葉");
        assert_eq!(filter(&images, &q("2026-11-08", None)).len(), 2);
        assert_eq!(filter(&images, &q("", Some(ColorBucket::Red)))[0].title, "紅葉");
        assert!(filter(&images, &q("ペンギン", Some(ColorBucket::Red))).is_empty());
    }
}
