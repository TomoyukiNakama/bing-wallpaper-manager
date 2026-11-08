//! 設定ファイル (`~/.config/bing-wallpaper-manager/config.json`)。

use std::fs;
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 画像の保存先。未設定なら `~/Pictures/BingWallpaper`。
    #[serde(default)]
    pub save_dir: Option<PathBuf>,
    /// 写真とモニターの縦横比が違うときの合わせ方。
    #[serde(default)]
    pub fit: FitMode,
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

    pub fn label(self) -> &'static str {
        match self {
            FitMode::Zoom => "画面いっぱいに拡大",
            FitMode::Scaled => "写真全体を表示",
            FitMode::Stretched => "引き伸ばす",
            FitMode::Centered => "中央に元の大きさで表示",
            FitMode::Spanned => "複数のモニターにまたがる",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            FitMode::Zoom => "縦横比を保ったまま画面を埋めます。はみ出した上下または左右は切り取られます。",
            FitMode::Scaled => "写真が切れないように縮小します。余った部分は単色の帯になります。",
            FitMode::Stretched => "画面の形に合わせて引き伸ばします。写真の縦横比が変わります。",
            FitMode::Centered => "拡大も縮小もせずに中央に置きます。画面より大きい写真は周りが切れます。",
            FitMode::Spanned => "すべてのモニターを 1 枚の画面として写真を広げます。",
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
            .with_context(|| format!("設定を保存できません: {}", path.display()))
    }

    pub fn save_dir(&self) -> PathBuf {
        self.save_dir.clone().unwrap_or_else(default_save_dir)
    }
}
