//! Bing の「今日の画像」API へのアクセスと画像ダウンロード。

use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;

const BING: &str = "https://www.bing.com";

/// ダッシュボードに表示するマーケット（地域）。
pub const MARKETS: &[(&str, &str)] = &[
    ("ja-JP", "日本"),
    ("en-US", "米国"),
    ("en-GB", "英国"),
    ("de-DE", "ドイツ"),
    ("fr-FR", "フランス"),
    ("zh-CN", "中国"),
];

pub fn market_name(code: &str) -> &str {
    MARKETS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| *n)
        .unwrap_or(code)
}

#[derive(Debug, Clone, Deserialize)]
pub struct BingImage {
    pub startdate: String,
    pub urlbase: String,
    pub copyright: String,
    #[serde(default)]
    pub title: String,
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

    pub fn url(&self, resolution: &str) -> String {
        format!("{BING}{}_{resolution}.jpg", self.urlbase)
    }
}

fn parse_date(d: &str) -> Option<(i32, u32, u32)> {
    if d.len() != 8 || !d.is_ascii() {
        return None;
    }
    Some((d[0..4].parse().ok()?, d[4..6].parse().ok()?, d[6..8].parse().ok()?))
}

/// `20261108` → `11月8日（日）`（`with_year` なら先頭に `2026年`）
pub fn date_ja(d: &str, with_year: bool) -> String {
    let Some((y, m, day)) = parse_date(d) else {
        return d.to_string();
    };
    // Sakamoto の曜日計算（0 = 日曜）
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let yy = if m < 3 { y - 1 } else { y };
    let w = (yy + yy / 4 - yy / 100 + yy / 400 + T[(m - 1) as usize] + day as i32).rem_euclid(7);
    let wd = ["日", "月", "火", "水", "木", "金", "土"][w as usize];
    if with_year {
        format!("{y}年{m}月{day}日（{wd}）")
    } else {
        format!("{m}月{day}日（{wd}）")
    }
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

/// 直近 `count` 日分（最大 8）の画像情報を新しい順に取得する。
pub fn fetch_archive(mkt: &str, count: u8) -> Result<Vec<BingImage>> {
    let url = format!(
        "{BING}/HPImageArchive.aspx?format=js&idx=0&n={}&mkt={mkt}",
        count.clamp(1, 8)
    );
    let archive: Archive = agent()
        .get(&url)
        .call()
        .with_context(|| format!("画像一覧の取得に失敗しました ({mkt})"))?
        .body_mut()
        .read_json()
        .with_context(|| format!("画像一覧の解析に失敗しました ({mkt})"))?;
    if archive.images.is_empty() {
        bail!("Bing から画像が返されませんでした ({mkt})");
    }
    Ok(archive.images)
}

/// `url` を `dest` に保存する。既に存在する場合は何もしない。
pub fn download(url: &str, dest: &Path) -> Result<()> {
    if fs::metadata(dest).map(|m| m.is_file() && m.len() > 0).unwrap_or(false) {
        return Ok(());
    }
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir).with_context(|| format!("{} を作成できません", dir.display()))?;
    }
    let resp = agent()
        .get(url)
        .call()
        .with_context(|| format!("ダウンロードに失敗しました: {url}"))?;
    let tmp = dest.with_extension("part");
    {
        let mut file = fs::File::create(&tmp)
            .with_context(|| format!("{} に書き込めません", tmp.display()))?;
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
        }
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
        assert_eq!(date_ja("20261108", false), "11月8日（日）");
        assert_eq!(date_ja("20260101", true), "2026年1月1日（木）");
    }
}
