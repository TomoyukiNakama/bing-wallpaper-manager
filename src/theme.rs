//! 配色とフォント。
//!
//! 写真が主役なので、UI は写真と競わない冷たい霧の色でまとめ、
//! 操作できる場所だけを湖の青緑で示す。ダークモードでは夜の湖の配色にする。

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Hsla, Window, WindowAppearance, rgb};

pub const FONT: &str = "Noto Sans CJK JP";

#[derive(Clone, Copy)]
pub struct Palette {
    /// 画面全体の地の色（霧）
    pub fog: Hsla,
    /// タイトルバー・フッターなど一段上の面
    pub surface: Hsla,
    /// 本文
    pub ink: Hsla,
    /// 補足テキスト
    pub slate: Hsla,
    /// 区切り線
    pub line: Hsla,
    /// 操作・選択のアクセント（湖）
    pub lake: Hsla,
    pub lake_hover: Hsla,
    pub on_lake: Hsla,
    pub danger: Hsla,
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

impl Palette {
    pub fn light() -> Self {
        Self {
            fog: c(0xE9EDEF),
            surface: c(0xF6F8F9),
            ink: c(0x1C2A33),
            slate: c(0x5E6E78),
            line: c(0xD3DADE),
            lake: c(0x2D6A8E),
            lake_hover: c(0x255A79),
            on_lake: c(0xFFFFFF),
            danger: c(0xA8402F),
        }
    }

    pub fn dark() -> Self {
        Self {
            fog: c(0x18232A),
            surface: c(0x1F2A31),
            ink: c(0xE4EAED),
            slate: c(0x93A3AD),
            line: c(0x2E3B43),
            lake: c(0x6FA8C9),
            lake_hover: c(0x86B8D4),
            on_lake: c(0x10202A),
            danger: c(0xE88C7A),
        }
    }

    pub fn current(window: &Window) -> Self {
        if is_dark(window) { Self::dark() } else { Self::light() }
    }
}

/// `BWM_THEME=light|dark` で固定でき、なければシステムの外観に従う。
pub fn is_dark(window: &Window) -> bool {
    match std::env::var("BWM_THEME").as_deref() {
        Ok("dark") => true,
        Ok("light") => false,
        _ => matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    }
}

/// gpui-component の部品（ボタン・タイトルバー・スクロールバー）に配色とフォントを反映する。
pub fn apply(window: &Window, cx: &mut App) {
    let dark = is_dark(window);
    let p = if dark { Palette::dark() } else { Palette::light() };
    Theme::change(if dark { ThemeMode::Dark } else { ThemeMode::Light }, None, cx);
    Theme::update(cx, |t| {
        t.font_family = FONT.into();
        let colors = &mut t.colors;
        colors.background = p.fog;
        colors.foreground = p.ink;
        colors.muted_foreground = p.slate;
        colors.border = p.line;
        colors.ring = p.lake;
        colors.primary = p.lake;
        colors.primary_hover = p.lake_hover;
        colors.primary_active = p.lake_hover;
        colors.primary_foreground = p.on_lake;
        colors.button_primary = p.lake;
        colors.button_primary_hover = p.lake_hover;
        colors.button_primary_active = p.lake_hover;
        colors.button_primary_foreground = p.on_lake;
        colors.secondary = p.surface;
        colors.secondary_hover = p.line;
        colors.secondary_foreground = p.ink;
        colors.title_bar = p.surface;
        colors.title_bar_border = p.line;
        colors.danger = p.danger;
    });
}
