//! 有志の Bing 壁紙アーカイブ（Bing-Wallpaper-Archive, https://bing.npanuhin.me）。
//!
//! Bing 公式の API でさかのぼれるのは約 2 週間なので、それより古い写真はここから取得する。
//! Microsoft の公式サービスではないため、使えないときは公式 API の範囲だけで動く。
//! 収録範囲は米国が 2010 年から、ほかの地域は 2024 年 4 月から（2026 年 11 月時点）。

use std::time::Duration;

use anyhow::{Context as _, Result};
use serde::Deserialize;

use crate::bing;
use crate::library::PhotoSource;

pub const SOURCE_NAME: &str = "Bing-Wallpaper-Archive";
const BASE: &str = "https://bing.npanuhin.me";

#[derive(Deserialize)]
struct Entry {
    title: Option<String>,
    caption: Option<String>,
    copyright: Option<String>,
    date: String,
    description: Option<String>,
    bing_url: Option<String>,
    url: Option<String>,
}

/// アーカイブの 1 枚。同じ写真を配信した地域をまとめてある。
#[derive(Debug, Clone)]
pub struct ArchivePhoto {
    pub source: PhotoSource,
    pub markets: Vec<&'static str>,
}

/// `ja-JP` → `JP/ja`
fn archive_path(market: &str) -> Option<String> {
    let (lang, country) = market.split_once('-')?;
    Some(format!("{country}/{lang}"))
}

/// `https://bing.com/th?id=OHR.Foo_JA-JP123_UHD.jpg` → `/th?id=OHR.Foo_JA-JP123`
fn urlbase_from(bing_url: &str) -> Option<String> {
    let start = bing_url.find("/th?id=")?;
    let (base, _) = bing_url[start..].rsplit_once('_')?;
    Some(base.to_string())
}

fn to_source(entry: Entry, market: &str) -> Option<PhotoSource> {
    let startdate = entry.date.replace('-', "");
    let urlbase = entry.bing_url.as_deref().and_then(urlbase_from).unwrap_or_default();
    if urlbase.is_empty() && entry.url.is_none() {
        return None;
    }
    // アーカイブの title は撮影地、caption は Bing の見出しにあたる
    let place = entry.title.unwrap_or_default();
    let copyright = match entry.copyright {
        Some(c) if !place.is_empty() => format!("{place} ({c})"),
        Some(c) => c,
        None => place.clone(),
    };
    let key = if urlbase.is_empty() {
        // Bing の URL が無い古い写真は、日付と地域で区別する
        format!("{startdate}_{market}")
    } else {
        bing::BingImage {
            startdate: startdate.clone(),
            urlbase: urlbase.clone(),
            copyright: String::new(),
            title: String::new(),
            copyrightlink: String::new(),
        }
        .key()
    };
    Some(PhotoSource {
        key,
        startdate,
        title: entry.caption.clone().unwrap_or_default(),
        copyright: copyright.clone(),
        localized: vec![(market.to_string(), entry.caption.clone().unwrap_or_default(), copyright.clone())],
        descriptions: entry
            .description
            .filter(|d| !d.is_empty())
            .map(|d| vec![(market.to_string(), d)])
            .unwrap_or_default(),
        urlbase,
        mirror_url: entry.url,
        links: Vec::new(),
    })
}

fn fetch_market(market: &str) -> Result<Vec<PhotoSource>> {
    let path = archive_path(market).context(tr!("error.unsupported_region"))?;
    let url = format!("{BASE}/{path}.json");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .user_agent("bing-wallpaper-manager/0.3")
        .build()
        .into();
    let entries: Vec<Entry> = agent
        .get(&url)
        .call()
        .with_context(|| tr!("error.archive_fetch", source = SOURCE_NAME, market = market))?
        .body_mut()
        .with_config()
        .limit(32 * 1024 * 1024)
        .read_json()
        .with_context(|| tr!("error.archive_parse", source = SOURCE_NAME, market = market))?;
    Ok(entries.into_iter().filter_map(|e| to_source(e, market)).collect())
}

/// 全地域の一覧を取得し、同じ写真をまとめて新しい順に返す。
/// 取得できなかった地域はエラーとして返し、取得できた分だけで続ける。
pub fn fetch_all(markets: &[&'static str]) -> (Vec<ArchivePhoto>, Vec<anyhow::Error>) {
    let handles: Vec<_> = markets
        .iter()
        .map(|&code| std::thread::spawn(move || (code, fetch_market(code))))
        .collect();

    let mut photos: Vec<ArchivePhoto> = Vec::new();
    let mut errors = Vec::new();
    let mut index = std::collections::HashMap::new();
    for handle in handles {
        let Ok((market, result)) = handle.join() else { continue };
        match result {
            Ok(sources) => {
                for source in sources {
                    match index.get(&source.key) {
                        Some(&i) => {
                            let photo: &mut ArchivePhoto = &mut photos[i];
                            if !photo.markets.contains(&market) {
                                photo.markets.push(market);
                            }
                            // 地域ごとの見出しと説明をまとめて持つ
                            photo.source.localized.extend(source.localized);
                            photo.source.descriptions.extend(source.descriptions);
                        }
                        None => {
                            index.insert(source.key.clone(), photos.len());
                            photos.push(ArchivePhoto { source, markets: vec![market] });
                        }
                    }
                }
            }
            Err(err) => errors.push(err),
        }
    }
    photos.sort_by(|a, b| b.source.startdate.cmp(&a.source.startdate));
    (photos, errors)
}

/// 「古い写真を読み込む」1 回分を選ぶ。`archive` は新しい順。
///
/// いちばん古い保管済みの写真（`oldest`）より前を `days` 日分と、それより新しい側の抜けを `gap_limit` 枚まで。
/// 抜けだけを先に埋めると一覧の途中にしか写真が増えず、止まったように見えるので、必ず古い方へも進める。
pub fn older_batch(
    archive: &[ArchivePhoto],
    skip: &std::collections::HashSet<String>,
    oldest: Option<&str>,
    days: usize,
    gap_limit: usize,
) -> Vec<ArchivePhoto> {
    let missing = || archive.iter().filter(|p| !skip.contains(&p.source.key));
    let is_older = |p: &&ArchivePhoto| oldest.is_none_or(|o| p.source.startdate.as_str() < o);

    let mut dates: Vec<&str> = Vec::new();
    let mut batch: Vec<ArchivePhoto> = missing()
        .filter(is_older)
        .take_while(|p| {
            let date = p.source.startdate.as_str();
            if dates.last() != Some(&date) {
                dates.push(date);
            }
            dates.len() <= days
        })
        .cloned()
        .collect();
    batch.extend(missing().filter(|p| !is_older(p)).take(gap_limit).cloned());
    batch
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo(key: &str, date: &str) -> ArchivePhoto {
        ArchivePhoto {
            source: PhotoSource {
                key: key.into(),
                startdate: date.into(),
                title: String::new(),
                copyright: String::new(),
                localized: Vec::new(),
                descriptions: Vec::new(),
                urlbase: String::new(),
                mirror_url: None,
                links: Vec::new(),
            },
            markets: vec!["en-US"],
        }
    }

    #[test]
    fn older_batch_moves_past_the_oldest_photo_even_with_gaps() {
        let archive = vec![
            photo("gap1", "20241105"),
            photo("have", "20241104"),
            photo("gap2", "20241103"),
            photo("gap3", "20241102"),
            photo("old1", "20240804"),
            photo("old2", "20240803"),
            photo("old2b", "20240803"),
            photo("old3", "20240802"),
        ];
        let skip = ["have".to_string()].into_iter().collect();
        let keys = |batch: Vec<ArchivePhoto>| batch.into_iter().map(|p| p.source.key).collect::<Vec<_>>();

        // 古い方へ 2 日分進み、抜けは 2 枚まで
        let batch = older_batch(&archive, &skip, Some("20240805"), 2, 2);
        assert_eq!(keys(batch), ["old1", "old2", "old2b", "gap1", "gap2"]);

        // 何も保管していなければ、新しい方から
        let batch = older_batch(&archive, &skip, None, 2, 2);
        assert_eq!(keys(batch), ["gap1", "gap2"]);

        // 古い写真が尽きても、抜けが残っていれば読み込む
        let batch = older_batch(&archive, &skip, Some("20240101"), 2, 3);
        assert_eq!(keys(batch), ["gap1", "gap2", "gap3"]);
    }

    #[test]
    fn converts_entry_with_bing_url() {
        let entry = Entry {
            title: Some("カクシャール・ツー山にかかる虹, キルギス".into()),
            caption: Some("虹を見つける日".into()),
            copyright: Some("© Emad aljumah/Getty Images".into()),
            date: "2024-04-03".into(),
            description: Some("虹は…".into()),
            bing_url: Some("https://bing.com/th?id=OHR.KyrgyzstanRainbow_JA-JP6458656191_UHD.jpg".into()),
            url: Some("https://bing.npanuhin.me/JP/ja/2024-04-03.jpg".into()),
        };
        let src = to_source(entry, "ja-JP").unwrap();
        assert_eq!(src.key, "OHR.KyrgyzstanRainbow");
        assert_eq!(src.startdate, "20240403");
        assert_eq!(src.urlbase, "/th?id=OHR.KyrgyzstanRainbow_JA-JP6458656191");
        assert_eq!(src.title, "虹を見つける日");
        assert_eq!(src.copyright, "カクシャール・ツー山にかかる虹, キルギス (© Emad aljumah/Getty Images)");
    }

    #[test]
    fn converts_entry_without_bing_url() {
        let entry = Entry {
            title: Some("Snow".into()),
            caption: None,
            copyright: None,
            date: "2010-01-01".into(),
            description: None,
            bing_url: None,
            url: Some("https://bing.npanuhin.me/US/en/2010-01-01.jpg".into()),
        };
        let src = to_source(entry, "en-US").unwrap();
        assert_eq!(src.key, "20100101_en-US");
        assert!(src.urlbase.is_empty());
        assert_eq!(archive_path("en-US").unwrap(), "US/en");
    }
}
