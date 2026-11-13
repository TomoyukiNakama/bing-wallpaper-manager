//! Bing の「今日の画像」API へのアクセスと画像ダウンロード。

use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;

const BING: &str = "https://www.bing.com";

/// Bing が地域ごとの写真を配信しているマーケット（地域）。ダッシュボードに出す地域は設定で選ぶ。
/// ここに無い地域（ko-KR、en-AU など）は、Bing が世界共通の写真（ROW）を返すので載せない。
/// （地域コード, 日本語名, 英語名）
pub const MARKETS: &[(&str, &str, &str)] = &[
    ("ja-JP", "日本", "Japan"),
    ("en-US", "米国", "United States"),
    ("en-GB", "英国", "United Kingdom"),
    ("de-DE", "ドイツ", "Germany"),
    ("fr-FR", "フランス", "France"),
    ("zh-CN", "中国", "China"),
    ("en-CA", "カナダ（英語）", "Canada (English)"),
    ("fr-CA", "カナダ（フランス語）", "Canada (French)"),
    ("en-IN", "インド", "India"),
    ("es-ES", "スペイン", "Spain"),
    ("it-IT", "イタリア", "Italy"),
    ("pt-BR", "ブラジル", "Brazil"),
];

/// 設定で選んでいないときに表示する地域の数（MARKETS の先頭から）。
pub const DEFAULT_MARKET_COUNT: usize = 6;

/// 地域コードを MARKETS にあるもの（`&'static str`）に直す。対応していない地域は None。
pub fn known_market(code: &str) -> Option<&'static str> {
    MARKETS.iter().map(|(c, _, _)| *c).find(|c| c.eq_ignore_ascii_case(code))
}

/// 表示言語での地域名（翻訳キーは `region.ja_jp` のような形）。
pub fn market_name(code: &str) -> String {
    if MARKETS.iter().any(|(c, _, _)| *c == code) {
        crate::i18n::text(&format!("region.{}", code.to_lowercase().replace('-', "_")))
    } else {
        code.to_string()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BingImage {
    pub startdate: String,
    pub urlbase: String,
    pub copyright: String,
    #[serde(default)]
    pub title: String,
    /// この写真について Bing で調べるページの URL。
    #[serde(default)]
    pub copyrightlink: String,
}

#[derive(Deserialize)]
struct Archive {
    images: Vec<BingImage>,
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '_' })
        .collect()
}

impl BingImage {
    /// `OHR.Foo_JA-JP123456` のような画像 ID。
    pub fn id(&self) -> String {
        let id = self
            .urlbase
            .split("id=")
            .nth(1)
            .unwrap_or(&self.urlbase)
            .split('&')
            .next()
            .unwrap_or_default();
        sanitize(id)
    }

    /// 地域に依存しない保存用キー（例: `OHR.Foo`）。
    /// 同じ写真は地域や配信日が違っても同じキーになるので、保存は 1 回で済む。
    pub fn key(&self) -> String {
        let id = self.id();
        match id.rsplit_once('_') {
            Some((name, _)) if name.starts_with("OHR.") => name.to_string(),
            _ => format!("{}_{}", sanitize(&self.startdate), id),
        }
    }

    pub fn display_title(&self) -> String {
        display_title(&self.title, &self.copyright)
    }

}

fn parse_date(d: &str) -> Option<(i32, u32, u32)> {
    if d.len() != 8 || !d.is_ascii() {
        return None;
    }
    Some((d[0..4].parse().ok()?, d[4..6].parse().ok()?, d[6..8].parse().ok()?))
}

/// `20261108` を表示言語の日付にする（例: `11月8日（日）`、`Sun, Nov 8`）。`with_year` なら年も付ける。
pub fn date_text(d: &str, with_year: bool) -> String {
    let Some((y, m, day)) = parse_date(d) else {
        return d.to_string();
    };
    // Sakamoto の曜日計算（0 = 日曜）
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let yy = if m < 3 { y - 1 } else { y };
    let w = (yy + yy / 4 - yy / 100 + yy / 400 + T[(m - 1) as usize] + day as i32).rem_euclid(7);
    crate::i18n::format_date(y, m, day, w as usize, with_year)
}

pub fn display_title(title: &str, copyright: &str) -> String {
    if !title.is_empty() && title != "Info" {
        title.to_string()
    } else {
        // copyright は「説明 (© 撮影者)」形式なので括弧の前を使う
        copyright.split(" (©").next().unwrap_or(copyright).to_string()
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .user_agent("bing-wallpaper-manager/0.2")
        .build()
        .into()
}

/// `idx` 日前から `count` 日分（最大 8）の画像情報を新しい順に取得する。
pub fn fetch_archive(mkt: &str, idx: u8, count: u8) -> Result<Vec<BingImage>> {
    let url = format!(
        "{BING}/HPImageArchive.aspx?format=js&idx={idx}&n={}&mkt={mkt}",
        count.clamp(1, 8)
    );
    let archive: Archive = agent()
        .get(&url)
        .call()
        .with_context(|| tr!("error.bing_fetch", market = mkt))?
        .body_mut()
        .read_json()
        .with_context(|| tr!("error.bing_parse", market = mkt))?;
    if archive.images.is_empty() {
        bail!(tr!("error.bing_empty", market = mkt));
    }
    Ok(archive.images)
}

/// 公式 API でさかのぼれる範囲（今日から 15 日分）を新しい順に取得する。
/// API の `idx` は 7 までしか効かないので、0〜7 日前と 7〜14 日前の 2 回に分けて取る。
pub fn fetch_recent(mkt: &str) -> Result<Vec<BingImage>> {
    let mut images = fetch_archive(mkt, 0, 8)?;
    if let Ok(older) = fetch_archive(mkt, 7, 8) {
        for img in older {
            if !images.iter().any(|i| i.startdate == img.startdate) {
                images.push(img);
            }
        }
    }
    images.sort_by(|a, b| b.startdate.cmp(&a.startdate));
    Ok(images)
}

#[derive(Deserialize)]
struct Model {
    #[serde(rename = "MediaContents", default)]
    contents: Vec<MediaContent>,
}

#[derive(Deserialize)]
struct MediaContent {
    #[serde(rename = "ImageContent")]
    image: ImageContent,
}

#[derive(Deserialize)]
struct ImageContent {
    #[serde(rename = "Description", default)]
    description: String,
    #[serde(rename = "Image")]
    image: ModelImage,
}

#[derive(Deserialize)]
struct ModelImage {
    #[serde(rename = "Url", default)]
    url: String,
}

/// `/th?id=OHR.Foo_JA-JP123_1920x1080.webp` → `OHR.Foo`
fn key_from_image_url(url: &str) -> Option<String> {
    let (urlbase, _) = url.rsplit_once('_')?;
    let image = BingImage {
        startdate: String::new(),
        urlbase: urlbase.to_string(),
        copyright: String::new(),
        title: String::new(),
        copyrightlink: String::new(),
    };
    let key = image.key();
    key.starts_with("OHR.").then_some(key)
}

fn parse_descriptions(model: Model) -> Vec<(String, String)> {
    model
        .contents
        .into_iter()
        .filter(|c| !c.image.description.trim().is_empty())
        .filter_map(|c| Some((key_from_image_url(&c.image.image.url)?, c.image.description)))
        .collect()
}

/// 最近の写真（約 1 週間分）の解説文を取得する。（写真のキー, 解説文）を返す。
/// Bing のホームページが使っている API で、HPImageArchive には解説文が無いのでこちらから補う。
pub fn fetch_descriptions(mkt: &str) -> Result<Vec<(String, String)>> {
    let url = format!("{BING}/hp/api/model?mkt={mkt}");
    let model: Model = agent()
        .get(&url)
        .call()
        .with_context(|| tr!("error.bing_fetch", market = mkt))?
        .body_mut()
        .read_json()
        .with_context(|| tr!("error.bing_parse", market = mkt))?;
    Ok(parse_descriptions(model))
}

/// `url` を `dest` に保存する。既に存在する場合は何もしない。
pub fn download(url: &str, dest: &Path) -> Result<()> {
    if fs::metadata(dest).map(|m| m.is_file() && m.len() > 0).unwrap_or(false) {
        return Ok(());
    }
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir).with_context(|| tr!("error.create_dir", path = dir.display()))?;
    }
    let resp = agent()
        .get(url)
        .call()
        .with_context(|| tr!("error.download", url = url))?;
    let tmp = dest.with_extension("part");
    {
        let mut file = fs::File::create(&tmp)
            .with_context(|| tr!("error.write", path = tmp.display()))?;
        io::copy(&mut resp.into_body().into_reader(), &mut file)?;
    }
    fs::rename(&tmp, dest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(urlbase: &str) -> BingImage {
        BingImage {
            startdate: "20261108".into(),
            urlbase: urlbase.into(),
            copyright: "説明 (© 撮影者)".into(),
            title: "Info".into(),
            copyrightlink: String::new(),
        }
    }

    #[test]
    fn reads_descriptions_from_homepage_model() {
        let json = r#"{"MediaContents":[
            {"ImageContent":{"Description":"湖の解説","Image":{"Url":"/th?id=OHR.AbrahamCanada_JA-JP9609532400_1920x1080.webp"}}},
            {"ImageContent":{"Description":"","Image":{"Url":"/th?id=OHR.Empty_JA-JP1_1920x1080.webp"}}}
        ]}"#;
        let model: Model = serde_json::from_str(json).unwrap();
        assert_eq!(parse_descriptions(model), vec![("OHR.AbrahamCanada".to_string(), "湖の解説".to_string())]);
    }

    #[test]
    fn known_market_ignores_case() {
        assert_eq!(known_market("ES-es"), Some("es-ES"));
        assert_eq!(known_market("ko-KR"), None);
    }

    #[test]
    fn key_ignores_market() {
        let ja = image("/th?id=OHR.HudsonBear_JA-JP9155678647");
        let fr = image("/th?id=OHR.HudsonBear_FR-FR2767726453");
        assert_eq!(ja.key(), "OHR.HudsonBear");
        assert_eq!(ja.key(), fr.key());
        assert_eq!(ja.display_title(), "説明");
    }

    #[test]
    fn japanese_date() {
        assert_eq!(date_text("20261108", false), "11月8日（日）");
        assert_eq!(date_text("20260101", true), "2026年1月1日（木）");
    }
}
