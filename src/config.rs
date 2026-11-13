//! 設定ファイル (`~/.config/bing-wallpaper-manager/config.json`)。

use std::fs;
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use crate::bing::{DEFAULT_MARKET_COUNT, MARKETS};
pub use crate::i18n::Language;
pub use crate::theme::ThemePref;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 画像の保存先。未設定なら `~/Pictures/BingWallpaper`。
    #[serde(default)]
    pub save_dir: Option<PathBuf>,
    /// 写真とモニターの縦横比が違うときの合わせ方。
    #[serde(default)]
    pub fit: FitMode,
    /// 表示言語。
    #[serde(default)]
    pub language: Language,
    /// 配色。
    #[serde(default)]
    pub theme: ThemePref,
    /// ダッシュボードに出す地域（Bing の地域コード）。未設定なら既定の 6 地域。
    #[serde(default)]
    pub markets: Option<Vec<String>>,
    /// 壁紙の自動更新。
    #[serde(default)]
    pub auto: AutoUpdate,
}

/// 壁紙の自動更新（systemd のユーザータイマーで `--apply` を実行する）。
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoUpdate {
    #[serde(default)]
    pub enabled: bool,
    /// 更新のしかた（enabled でないときも、前に選んだものを覚えておく）。
    #[serde(default)]
    pub mode: AutoMode,
    /// 「今日の写真」で使う地域。未設定なら表示している地域の最初のもの。
    #[serde(default)]
    pub market: Option<String>,
    /// お気に入り・保存した写真から選ぶときの切り替えの間隔。
    #[serde(default)]
    pub interval: Interval,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoMode {
    /// 新しい「今日の写真」が配信されたら壁紙にする
    #[default]
    Today,
    /// お気に入りから無作為に選ぶ
    Favorites,
    /// 保存した写真すべてから無作為に選ぶ
    Random,
}

impl AutoMode {
    pub const ALL: [AutoMode; 3] = [AutoMode::Today, AutoMode::Favorites, AutoMode::Random];

    pub fn label(self) -> String {
        match self {
            AutoMode::Today => tr!("auto.today"),
            AutoMode::Favorites => tr!("auto.favorites"),
            AutoMode::Random => tr!("auto.random"),
        }
    }

    pub fn description(self) -> String {
        match self {
            AutoMode::Today => tr!("auto.today_desc"),
            AutoMode::Favorites => tr!("auto.favorites_desc"),
            AutoMode::Random => tr!("auto.random_desc"),
        }
    }
}

/// 壁紙を切り替える間隔。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Interval {
    Minutes30,
    Hour1,
    Hours3,
    Hours6,
    #[default]
    Daily,
}

impl Interval {
    pub const ALL: [Interval; 5] =
        [Interval::Minutes30, Interval::Hour1, Interval::Hours3, Interval::Hours6, Interval::Daily];

    pub fn label(self) -> String {
        match self {
            Interval::Minutes30 => tr!("interval.minutes", count = 30),
            Interval::Hour1 => tr!("interval.hour"),
            Interval::Hours3 => tr!("interval.hours", count = 3),
            Interval::Hours6 => tr!("interval.hours", count = 6),
            Interval::Daily => tr!("interval.daily"),
        }
    }

    /// systemd の `OnCalendar` の値。
    pub fn on_calendar(self) -> &'static str {
        match self {
            Interval::Minutes30 => "*:0/30",
            Interval::Hour1 => "hourly",
            Interval::Hours3 => "0/3:00",
            Interval::Hours6 => "0/6:00",
            Interval::Daily => "daily",
        }
    }
}

/// 写真の縦横比がモニターと違うときの、壁紙への合わせ方（GNOME の picture-options）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FitMode {
    #[default]
    Zoom,
    Scaled,
    Stretched,
    Centered,
    Spanned,
}

impl FitMode {
    pub const ALL: [FitMode; 5] = [
        FitMode::Zoom,
        FitMode::Scaled,
        FitMode::Stretched,
        FitMode::Centered,
        FitMode::Spanned,
    ];

    /// `org.gnome.desktop.background picture-options` の値。
    pub fn gsettings_value(self) -> &'static str {
        match self {
            FitMode::Zoom => "zoom",
            FitMode::Scaled => "scaled",
            FitMode::Stretched => "stretched",
            FitMode::Centered => "centered",
            FitMode::Spanned => "spanned",
        }
    }

    pub fn label(self) -> String {
        match self {
            FitMode::Zoom => tr!("fit.zoom"),
            FitMode::Scaled => tr!("fit.scaled"),
            FitMode::Stretched => tr!("fit.stretched"),
            FitMode::Centered => tr!("fit.centered"),
            FitMode::Spanned => tr!("fit.spanned"),
        }
    }

    pub fn description(self) -> String {
        match self {
            FitMode::Zoom => tr!("fit.zoom_desc"),
            FitMode::Scaled => tr!("fit.scaled_desc"),
            FitMode::Stretched => tr!("fit.stretched_desc"),
            FitMode::Centered => tr!("fit.centered_desc"),
            FitMode::Spanned => tr!("fit.spanned_desc"),
        }
    }
}

fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("bing-wallpaper-manager")
}

fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

/// 設定ではない、アプリが覚えておく状態（`~/.config/bing-wallpaper-manager/state.json`）。
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// 前回のウィンドウの大きさ（幅, 高さ）。
    #[serde(default)]
    pub window: Option<(f32, f32)>,
    /// 自動更新で最後に壁紙にした「今日の写真」のキー。
    /// 同じ写真を何度も設定し直さない（手で選んだ壁紙を、次の写真が配信されるまで残す）ために使う。
    #[serde(default)]
    pub last_auto: Option<String>,
}

impl State {
    fn path() -> PathBuf {
        config_dir().join("state.json")
    }

    pub fn load() -> Self {
        fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// 読み込み直してから変更を書き込む（アプリと `--apply` が別々に書くため）。
    pub fn update(change: impl FnOnce(&mut State)) -> Result<()> {
        let mut state = Self::load();
        change(&mut state);
        let path = Self::path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&path, serde_json::to_string_pretty(&state)?)
            .with_context(|| tr!("error.save_settings", path = path.display()))
    }
}

pub fn default_save_dir() -> PathBuf {
    dirs::picture_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Pictures")))
        .unwrap_or_else(std::env::temp_dir)
        .join("BingWallpaper")
}

impl Config {
    pub fn load() -> Self {
        fs::read_to_string(config_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&path, serde_json::to_string_pretty(self)?)
            .with_context(|| tr!("error.save_settings", path = path.display()))
    }

    pub fn save_dir(&self) -> PathBuf {
        self.save_dir.clone().unwrap_or_else(default_save_dir)
    }

    /// ダッシュボードに出す地域（MARKETS の順）。設定が無い・空のときは既定の地域。
    pub fn markets(&self) -> Vec<&'static str> {
        let chosen: Vec<&'static str> = match &self.markets {
            Some(codes) => MARKETS
                .iter()
                .map(|(code, _, _)| *code)
                .filter(|code| codes.iter().any(|c| c.eq_ignore_ascii_case(code)))
                .collect(),
            None => Vec::new(),
        };
        if chosen.is_empty() {
            MARKETS.iter().take(DEFAULT_MARKET_COUNT).map(|(code, _, _)| *code).collect()
        } else {
            chosen
        }
    }

    /// 自動更新の「今日の写真」で使う地域。
    pub fn auto_market(&self) -> &'static str {
        self.auto
            .market
            .as_deref()
            .and_then(crate::bing::known_market)
            .unwrap_or_else(|| self.markets()[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markets_fall_back_to_default() {
        let mut config = Config::default();
        assert_eq!(config.markets().len(), DEFAULT_MARKET_COUNT);
        assert_eq!(config.auto_market(), "ja-JP");
        config.markets = Some(vec!["pt-BR".into(), "en-US".into(), "xx-XX".into()]);
        assert_eq!(config.markets(), ["en-US", "pt-BR"]);
        assert_eq!(config.auto_market(), "en-US");
        config.markets = Some(Vec::new());
        assert_eq!(config.markets().len(), DEFAULT_MARKET_COUNT);
    }

    #[test]
    fn old_config_still_loads() {
        let config: Config = serde_json::from_str(r#"{"save_dir": "/tmp/x", "fit": "scaled"}"#).unwrap();
        assert!(!config.auto.enabled);
        assert_eq!(config.auto.mode, AutoMode::Today);
        assert_eq!(config.auto.interval, Interval::Daily);
    }
}
