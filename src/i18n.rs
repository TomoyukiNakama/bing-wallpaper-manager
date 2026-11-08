//! 表示言語。
//!
//! 文言は `locales/<言語>.yml` に置き、コードからは `tr!("キー")` や
//! `tr!("キー", count = n)` で参照する（rust-i18n。ビルド時にアプリへ埋め込まれる）。
//! 訳が無いキーは英語で表示する。新しい言語は `locales/` にファイルを足し、
//! [`Language`] に追加する。

/// `tr!("past.count", count = n)`: 今の表示言語の文言（`String`）。
#[macro_export]
macro_rules! tr {
    ($($all:tt)*) => {
        ::rust_i18n::t!($($all)*).into_owned()
    };
}

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

/// 設定画面で選ぶ言語。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    /// OS の言語に合わせる
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "ja")]
    Ja,
    #[serde(rename = "en")]
    En,
    #[serde(rename = "zh-CN")]
    ZhHans,
    #[serde(rename = "zh-TW")]
    ZhHant,
    #[serde(rename = "ko")]
    Ko,
    #[serde(rename = "de")]
    De,
    #[serde(rename = "fr")]
    Fr,
    #[serde(rename = "es")]
    Es,
    #[serde(rename = "it")]
    It,
    #[serde(rename = "pt-BR")]
    Pt,
    #[serde(rename = "ru")]
    Ru,
}

impl Language {
    /// 設定画面に並べる順（自動 → アジア → ヨーロッパ）。
    pub const ALL: [Language; 12] = [
        Language::Auto,
        Language::Ja,
        Language::En,
        Language::ZhHans,
        Language::ZhHant,
        Language::Ko,
        Language::De,
        Language::Fr,
        Language::Es,
        Language::It,
        Language::Pt,
        Language::Ru,
    ];

    /// `locales/` のファイル名（rust-i18n のロケール名）。
    pub fn code(self) -> &'static str {
        match self {
            Language::Auto | Language::En => "en",
            Language::Ja => "ja",
            Language::ZhHans => "zh-CN",
            Language::ZhHant => "zh-TW",
            Language::Ko => "ko",
            Language::De => "de",
            Language::Fr => "fr",
            Language::Es => "es",
            Language::It => "it",
            Language::Pt => "pt-BR",
            Language::Ru => "ru",
        }
    }

    /// 設定画面での表示名。言語名はその言語自身の表記で出す。
    pub fn label(self) -> String {
        match self {
            Language::Auto => tr!("language.auto"),
            Language::Ja => "日本語".to_string(),
            Language::En => "English".to_string(),
            Language::ZhHans => "简体中文".to_string(),
            Language::ZhHant => "繁體中文".to_string(),
            Language::Ko => "한국어".to_string(),
            Language::De => "Deutsch".to_string(),
            Language::Fr => "Français".to_string(),
            Language::Es => "Español".to_string(),
            Language::It => "Italiano".to_string(),
            Language::Pt => "Português (Brasil)".to_string(),
            Language::Ru => "Русский".to_string(),
        }
    }

    /// 写真の見出しと説明を優先して取る Bing の地域。無い言語は英語圏。
    pub fn content_markets(self) -> &'static [&'static str] {
        match self {
            Language::Ja => &["ja-JP"],
            Language::De => &["de-DE"],
            Language::Fr => &["fr-FR"],
            Language::ZhHans => &["zh-CN"],
            _ => &["en-US", "en-GB"],
        }
    }

    /// `LANG` などの値（`ja_JP.UTF-8`、`zh_TW` など）から言語を決める。
    fn from_os_locale(value: &str) -> Language {
        let value = value.to_ascii_lowercase().replace('-', "_");
        let lang = value.split(['_', '.', '@']).next().unwrap_or("");
        match lang {
            "ja" => Language::Ja,
            "zh" if ["_tw", "_hk", "_mo", "hant"].iter().any(|r| value.contains(r)) => Language::ZhHant,
            "zh" => Language::ZhHans,
            "ko" => Language::Ko,
            "de" => Language::De,
            "fr" => Language::Fr,
            "es" => Language::Es,
            "it" => Language::It,
            "pt" => Language::Pt,
            "ru" => Language::Ru,
            _ => Language::En,
        }
    }
}

/// 今の表示言語（Auto は解決済みの言語に置き換えて持つ）。Language::ALL の添字。
static CURRENT: AtomicU8 = AtomicU8::new(1);

pub fn current() -> Language {
    Language::ALL[CURRENT.load(Ordering::Relaxed) as usize]
}

/// OS の言語設定（LC_ALL → LC_MESSAGES → LANG）。
fn os_language() -> Language {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
        .map(|v| Language::from_os_locale(&v))
        .unwrap_or(Language::En)
}

/// 表示言語を切り替える（gpui-component の部品の文言も、訳があれば合わせる）。
pub fn set(language: Language) {
    let resolved = if language == Language::Auto { os_language() } else { language };
    let index = Language::ALL.iter().position(|l| *l == resolved).unwrap_or(2);
    CURRENT.store(index as u8, Ordering::Relaxed);
    rust_i18n::set_locale(resolved.code());
}

/// 実行時に組み立てたキーの文言（`region.ja_jp` など）。
pub fn text(key: &str) -> String {
    rust_i18n::t!(key).into_owned()
}

/// 日付（`20261108` 形式）を今の表示言語の書式にする。`with_year` で年も付ける。
pub fn format_date(y: i32, m: u32, d: u32, weekday: usize, with_year: bool) -> String {
    let mi = (m - 1) as usize;
    match current() {
        Language::Ja | Language::Auto => {
            let w = ["日", "月", "火", "水", "木", "金", "土"][weekday];
            if with_year { format!("{y}年{m}月{d}日（{w}）") } else { format!("{m}月{d}日（{w}）") }
        }
        Language::ZhHans | Language::ZhHant => {
            let w = if current() == Language::ZhHans {
                ["周日", "周一", "周二", "周三", "周四", "周五", "周六"][weekday]
            } else {
                ["週日", "週一", "週二", "週三", "週四", "週五", "週六"][weekday]
            };
            if with_year { format!("{y}年{m}月{d}日 {w}") } else { format!("{m}月{d}日 {w}") }
        }
        Language::Ko => {
            let w = ["일", "월", "화", "수", "목", "금", "토"][weekday];
            if with_year { format!("{y}년 {m}월 {d}일 ({w})") } else { format!("{m}월 {d}일 ({w})") }
        }
        Language::En => {
            let w = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][weekday];
            let mo = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][mi];
            if with_year { format!("{w}, {mo} {d}, {y}") } else { format!("{w}, {mo} {d}") }
        }
        Language::De => {
            let w = ["So.", "Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa."][weekday];
            let mo = ["Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sept.", "Okt.", "Nov.", "Dez."][mi];
            if with_year { format!("{w}, {d}. {mo} {y}") } else { format!("{w}, {d}. {mo}") }
        }
        Language::Fr => {
            let w = ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."][weekday];
            let mo = ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."][mi];
            if with_year { format!("{w} {d} {mo} {y}") } else { format!("{w} {d} {mo}") }
        }
        Language::Es => {
            let w = ["dom", "lun", "mar", "mié", "jue", "vie", "sáb"][weekday];
            let mo = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic"][mi];
            if with_year { format!("{w}, {d} {mo} {y}") } else { format!("{w}, {d} {mo}") }
        }
        Language::It => {
            let w = ["dom", "lun", "mar", "mer", "gio", "ven", "sab"][weekday];
            let mo = ["gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic"][mi];
            if with_year { format!("{w} {d} {mo} {y}") } else { format!("{w} {d} {mo}") }
        }
        Language::Pt => {
            let w = ["dom.", "seg.", "ter.", "qua.", "qui.", "sex.", "sáb."][weekday];
            let mo = ["jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.", "nov.", "dez."][mi];
            if with_year { format!("{w}, {d} de {mo} de {y}") } else { format!("{w}, {d} de {mo}") }
        }
        Language::Ru => {
            let w = ["вс", "пн", "вт", "ср", "чт", "пт", "сб"][weekday];
            let mo = ["янв.", "февр.", "мар.", "апр.", "мая", "июн.", "июл.", "авг.", "сент.", "окт.", "нояб.", "дек."][mi];
            if with_year { format!("{w}, {d} {mo} {y} г.") } else { format!("{w}, {d} {mo}") }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn keys(lang: &str) -> BTreeSet<String> {
        let path = format!("{}/locales/{lang}.yml", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("{path} がありません"))
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .filter_map(|l| l.split_once(": ").map(|(k, _)| k.to_string()))
            .collect()
    }

    /// すべての言語の翻訳ファイルに、英語と同じキーがそろっていること。
    #[test]
    fn every_language_has_every_key() {
        let english = keys("en");
        for lang in Language::ALL.iter().filter(|l| **l != Language::Auto) {
            let other = keys(lang.code());
            let missing: Vec<_> = english.difference(&other).collect();
            let extra: Vec<_> = other.difference(&english).collect();
            assert!(missing.is_empty() && extra.is_empty(), "{}: 不足 {missing:?} 余分 {extra:?}", lang.code());
        }
    }

    #[test]
    fn os_locale_detection() {
        assert_eq!(Language::from_os_locale("ja_JP.UTF-8"), Language::Ja);
        assert_eq!(Language::from_os_locale("zh_TW.UTF-8"), Language::ZhHant);
        assert_eq!(Language::from_os_locale("zh_CN.UTF-8"), Language::ZhHans);
        assert_eq!(Language::from_os_locale("pt_BR.UTF-8"), Language::Pt);
        assert_eq!(Language::from_os_locale("sv_SE.UTF-8"), Language::En);
    }
}
