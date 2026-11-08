//! GNOME（Ubuntu 標準デスクトップ）の壁紙設定。

use std::path::Path;
use std::process::Command;

use anyhow::{Context as _, Result, bail};

fn gsettings(key: &str, value: &str) -> Result<()> {
    let status = Command::new("gsettings")
        .args(["set", "org.gnome.desktop.background", key, value])
        .status()
        .context("gsettings を実行できません（GNOME 環境が必要です）")?;
    if !status.success() {
        bail!("gsettings set {key} が失敗しました ({status})");
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

/// 指定した画像をライト・ダーク両方の壁紙に設定する。
pub fn set_wallpaper(path: &Path) -> Result<()> {
    let path = path
        .canonicalize()
        .with_context(|| format!("{} が見つかりません", path.display()))?;
    let uri = file_uri(&path);
    gsettings("picture-uri", &uri)?;
    gsettings("picture-uri-dark", &uri)?;
    gsettings("picture-options", "zoom")?;
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

pub fn open_folder(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .context("xdg-open を実行できません")?;
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
