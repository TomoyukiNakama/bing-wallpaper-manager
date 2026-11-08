//! GNOME（Ubuntu 標準デスクトップ）の壁紙設定。

use std::path::Path;
use std::process::Command;

use anyhow::{Context as _, Result, bail};

use crate::config::FitMode;

fn gsettings(key: &str, value: &str) -> Result<()> {
    let status = Command::new("gsettings")
        .args(["set", "org.gnome.desktop.background", key, value])
        .status()
        .context(tr!("gsettings を実行できません（GNOME 環境が必要です）", "Cannot run gsettings (GNOME is required)"))?;
    if !status.success() {
        bail!(tr!(format!("gsettings set {key} が失敗しました ({status})"), format!("gsettings set {key} failed ({status})")));
    }
    Ok(())
}

/// パスを `file://` URI に変換する（非 ASCII 文字はパーセントエンコード）。
pub fn file_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut uri = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            uri.push(b as char);
        } else {
            uri.push_str(&format!("%{b:02X}"));
        }
    }
    uri
}

/// 縦横比の合わせ方だけを変える（今の壁紙にもすぐ反映される）。
pub fn set_fit(fit: FitMode) -> Result<()> {
    gsettings("picture-options", fit.gsettings_value())
}

/// 指定した画像をライト・ダーク両方の壁紙に設定する。
pub fn set_wallpaper(path: &Path, fit: FitMode) -> Result<()> {
    let path = path
        .canonicalize()
        .with_context(|| tr!(format!("{} が見つかりません", path.display()), format!("{} not found", path.display())))?;
    let uri = file_uri(&path);
    gsettings("picture-uri", &uri)?;
    gsettings("picture-uri-dark", &uri)?;
    set_fit(fit)?;
    Ok(())
}

/// 現在の壁紙 URI（ライトモード側）。
pub fn current_uri() -> Option<String> {
    let out = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.background", "picture-uri"])
        .output()
        .ok()?;
    let s = String::from_utf8(out.stdout).ok()?;
    Some(s.trim().trim_matches('\'').to_string())
}

/// 画像を既定のアプリで開く。
/// 余白に塗られる色（picture-options が scaled などのとき）。`0xRRGGBB` で返す。
pub fn primary_color() -> Option<u32> {
    let out = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.background", "primary-color"])
        .output()
        .ok()?;
    let s = String::from_utf8(out.stdout).ok()?;
    let hex = s.trim().trim_matches('\'').strip_prefix('#')?;
    u32::from_str_radix(hex.get(..6)?, 16).ok()
}

/// 画像を既定のアプリで開く。
pub fn open_file(path: &Path) -> Result<()> {
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .context(tr!("xdg-open を実行できません", "Cannot run xdg-open"))?;
    Ok(())
}

/// ファイルマネージャーでフォルダを開き、そのファイルを選択した状態にする。
/// 対応していないファイルマネージャーでは、フォルダを開くだけにする。
pub fn show_in_folder(path: &Path) -> Result<()> {
    let uri = file_uri(path);
    let shown = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.FileManager1",
            "--object-path",
            "/org/freedesktop/FileManager1",
            "--method",
            "org.freedesktop.FileManager1.ShowItems",
            &format!("['{uri}']"),
            "",
        ])
        .output()
        .is_ok_and(|o| o.status.success());
    if !shown {
        if let Some(dir) = path.parent() {
            open_folder(dir)?;
        }
    }
    Ok(())
}

pub fn open_folder(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .context(tr!("xdg-open を実行できません", "Cannot run xdg-open"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_non_ascii_path() {
        assert_eq!(
            file_uri(Path::new("/home/u/ピクチャ/a b.jpg")),
            "file:///home/u/%E3%83%94%E3%82%AF%E3%83%81%E3%83%A3/a%20b.jpg"
        );
    }
}
