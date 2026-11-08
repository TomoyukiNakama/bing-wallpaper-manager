//! 表示言語（日本語・英語）。
//!
//! 文言はその場で `tr!("日本語", "English")` のように両方を書く。
//! 片方だけが評価されるので、`format!` を渡してもよい。

/// `tr!("日本語", "English")`: 今の表示言語の方だけを評価して返す。
#[macro_export]
macro_rules! tr {
    ($ja:expr, $en:expr $(,)?) => {
        if $crate::i18n::is_en() { $en } else { $ja }
    };
}

use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

/// 設定画面で選ぶ言語。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// OS の言語に合わせる
    #[default]
    Auto,
    Ja,
    En,
}

impl Language {
    pub const ALL: [Language; 3] = [Language::Auto, Language::Ja, Language::En];

    pub fn label(self) -> &'static str {
        match self {
            Language::Auto => tr!("自動（OS に合わせる）", "Automatic (follow the OS)"),
            // 言語名は、その言語自身の表記で出す
            Language::Ja => "日本語",
            Language::En => "English",
        }
    }
}

static ENGLISH: AtomicBool = AtomicBool::new(false);

/// 英語で表示するか。
pub fn is_en() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// OS の言語設定（LC_ALL → LC_MESSAGES → LANG）が日本語か。
fn os_is_japanese() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty())
        .is_none_or(|v| v.starts_with("ja"))
}

/// 表示言語を切り替える（gpui-component の部品の文言も合わせる）。
pub fn set(language: Language) {
    let en = match language {
        Language::Auto => !os_is_japanese(),
        Language::Ja => false,
        Language::En => true,
    };
    ENGLISH.store(en, Ordering::Relaxed);
    gpui_kit::component::set_locale(if en { "en" } else { "ja" });
}

