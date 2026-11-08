//! 写真の色味。サムネイルから「どの色がどれくらい含まれるか」を計算し、色で探すのに使う。

use std::path::Path;

use anyhow::Result;
use gpui_kit::{Hsla, rgb};

/// 探せる色。並び順は設定画面・検索欄の色見本の順。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorBucket {
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Blue,
    Purple,
    Pink,
    White,
    Black,
}

impl ColorBucket {
    pub const ALL: [ColorBucket; 10] = [
        ColorBucket::Red,
        ColorBucket::Orange,
        ColorBucket::Yellow,
        ColorBucket::Green,
        ColorBucket::Cyan,
        ColorBucket::Blue,
        ColorBucket::Purple,
        ColorBucket::Pink,
        ColorBucket::White,
        ColorBucket::Black,
    ];

    pub fn index(self) -> usize {
        ColorBucket::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }

    /// 翻訳キー（`color.red` など）。
    pub fn label(self) -> String {
        match self {
            ColorBucket::Red => tr!("color.red"),
            ColorBucket::Orange => tr!("color.orange"),
            ColorBucket::Yellow => tr!("color.yellow"),
            ColorBucket::Green => tr!("color.green"),
            ColorBucket::Cyan => tr!("color.cyan"),
            ColorBucket::Blue => tr!("color.blue"),
            ColorBucket::Purple => tr!("color.purple"),
            ColorBucket::Pink => tr!("color.pink"),
            ColorBucket::White => tr!("color.white"),
            ColorBucket::Black => tr!("color.black"),
        }
    }

    /// 色見本に塗る色。
    pub fn swatch(self) -> Hsla {
        rgb(match self {
            ColorBucket::Red => 0xD64545,
            ColorBucket::Orange => 0xE8893A,
            ColorBucket::Yellow => 0xE8C547,
            ColorBucket::Green => 0x4E9A51,
            ColorBucket::Cyan => 0x3FB8C4,
            ColorBucket::Blue => 0x3A6FC4,
            ColorBucket::Purple => 0x7E57C2,
            ColorBucket::Pink => 0xE07AA8,
            ColorBucket::White => 0xF4F4F4,
            ColorBucket::Black => 0x222222,
        })
        .into()
    }
}

/// この割合（%）以上含まれていれば「その色の写真」とみなす。
pub const MATCH_THRESHOLD: u8 = 12;

/// 1 画素を色の区分に分ける。灰色（彩度が低く、白でも黒でもない）は None。
fn classify(r: u8, g: u8, b: u8) -> Option<ColorBucket> {
    let (r, g, b) = (r as f32 / 255., g as f32 / 255., b as f32 / 255.);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.;
    let d = max - min;
    let s = if d == 0. { 0. } else { d / (1. - (2. * l - 1.).abs()) };
    if l > 0.85 && s < 0.35 {
        return Some(ColorBucket::White);
    }
    if l < 0.13 {
        return Some(ColorBucket::Black);
    }
    if s < 0.2 || d < 0.08 {
        return None;
    }
    let h = if max == r {
        60. * (((g - b) / d).rem_euclid(6.))
    } else if max == g {
        60. * ((b - r) / d + 2.)
    } else {
        60. * ((r - g) / d + 4.)
    };
    Some(match h {
        h if h < 15. => ColorBucket::Red,
        h if h < 40. => ColorBucket::Orange,
        h if h < 65. => ColorBucket::Yellow,
        h if h < 165. => ColorBucket::Green,
        h if h < 195. => ColorBucket::Cyan,
        h if h < 255. => ColorBucket::Blue,
        h if h < 290. => ColorBucket::Purple,
        h if h < 345. => ColorBucket::Pink,
        _ => ColorBucket::Red,
    })
}

/// 画像に含まれる各色の割合（%、ColorBucket::ALL の順）。
pub fn analyze(path: &Path) -> Result<Vec<u8>> {
    let image = image::open(path)?.thumbnail(96, 54).to_rgb8();
    let mut counts = [0u32; ColorBucket::ALL.len()];
    let total = (image.width() * image.height()).max(1);
    for pixel in image.pixels() {
        if let Some(bucket) = classify(pixel[0], pixel[1], pixel[2]) {
            counts[bucket.index()] += 1;
        }
    }
    Ok(counts.iter().map(|c| ((c * 100 + total / 2) / total) as u8).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_basic_colors() {
        assert_eq!(classify(220, 40, 40), Some(ColorBucket::Red));
        assert_eq!(classify(240, 150, 40), Some(ColorBucket::Orange));
        assert_eq!(classify(40, 160, 60), Some(ColorBucket::Green));
        assert_eq!(classify(40, 90, 200), Some(ColorBucket::Blue));
        assert_eq!(classify(250, 250, 250), Some(ColorBucket::White));
        assert_eq!(classify(10, 10, 15), Some(ColorBucket::Black));
        assert_eq!(classify(128, 128, 128), None);
    }
}
