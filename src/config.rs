//! 設定ファイル (`~/.config/bing-wallpaper-manager/config.json`)。

use std::fs;
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

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

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bing-wallpaper-manager")
        .join("config.json")
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
}
