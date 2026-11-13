//! 保存先ディレクトリに画像を保管するライブラリ。
//!
//! ```text
//! <保存先>/
//!   20261108_OHR.Foo_UHD.jpg           壁紙用の画像（UHD、無ければ 1920x1080）
//!   .bing-wallpaper/meta/<key>.json     タイトルなどのメタデータ（key は OHR.Foo のような写真名）
//!   .bing-wallpaper/thumbs/<key>.jpg    ダッシュボード用サムネイル (640x360)
//!   .bing-wallpaper/previews/<key>.jpg  大きく表示するための中サイズ (1920x1080、古い写真だけ)
//! ```
//!
//! 最近の写真（Bing 公式 API）は壁紙用の画像まで保管する。アーカイブから読み込んだ
//! 古い写真はサムネイルだけを保管し、壁紙にするときに画像を取得する。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use crate::bing::{self, BingImage};


/// 写真の取得元。Bing 公式の URL か、アーカイブにしかない画像のどちらか（または両方）を持つ。
#[derive(Debug, Clone)]
pub struct PhotoSource {
    pub key: String,
    pub startdate: String,
    pub title: String,
    pub copyright: String,
    /// Bing の画像 URL の共通部分（`/th?id=OHR.Foo_JA-JP123`）。古い写真では空。
    pub urlbase: String,
    /// アーカイブが保存している画像。Bing の URL が無い古い写真で使う。
    pub mirror_url: Option<String>,
    /// 地域ごとの見出しと説明（地域コード, 見出し, 説明）。表示言語に合うものを選ぶ。
    pub localized: Vec<(String, String, String)>,
    /// 地域ごとの解説文（地域コード, 解説文）。検索と「写真の情報」に使う。
    pub descriptions: Vec<(String, String)>,
    /// 地域ごとの、この写真について Bing で調べるページ（地域コード, URL）。
    pub links: Vec<(String, String)>,
}

impl PhotoSource {
    /// 公式 API の 1 枚。配信した地域の見出しと説明も持つ。
    pub fn from_bing(img: &BingImage, market: &str) -> Self {
        Self {
            key: img.key(),
            startdate: img.startdate.clone(),
            title: img.title.clone(),
            copyright: img.copyright.clone(),
            urlbase: img.urlbase.clone(),
            mirror_url: None,
            localized: vec![(market.to_string(), img.title.clone(), img.copyright.clone())],
            descriptions: Vec::new(),
            // 写真が無い日などは、リンクの代わりに javascript:void(0) が入っている
            links: Some(img.copyrightlink.clone())
                .filter(|l| l.starts_with("https://"))
                .map(|l| vec![(market.to_string(), l)])
                .unwrap_or_default(),
        }
    }

    fn bing_url(&self, resolution: &str) -> Option<String> {
        (!self.urlbase.is_empty()).then(|| format!("https://www.bing.com{}_{resolution}.jpg", self.urlbase))
    }

    /// サムネイルの URL（Bing の 640x360、無ければアーカイブの画像そのもの）。
    fn thumb_url(&self) -> Option<String> {
        self.bing_url("640x360").or_else(|| self.mirror_url.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredImage {
    pub key: String,
    pub startdate: String,
    pub title: String,
    pub copyright: String,
    pub urlbase: String,
    #[serde(default)]
    pub mirror_url: Option<String>,
    /// この画像が配信された地域。
    #[serde(default)]
    pub markets: Vec<String>,
    /// 地域ごとの見出しと説明（地域コード → (見出し, 説明)）。表示言語に合うものを選ぶ。
    #[serde(default)]
    pub localized: BTreeMap<String, (String, String)>,
    /// 地域ごとの解説文（地域コード → 解説文）。検索と「写真の情報」に使う。
    #[serde(default)]
    pub descriptions: BTreeMap<String, String>,
    /// 地域ごとの、この写真について Bing で調べるページ（地域コード → URL）。
    #[serde(default)]
    pub links: BTreeMap<String, String>,
    /// 含まれる色の割合（%、colors::ColorBucket::ALL の順）。色で探すのに使う。
    #[serde(default)]
    pub colors: Option<Vec<u8>>,
    /// お気に入りに登録しているか。
    #[serde(default)]
    pub favorite: bool,
    /// 以前の形式（英語版だけを持っていた）。読み込み時だけ使う。
    #[serde(default, skip_serializing)]
    title_en: Option<String>,
    #[serde(default, skip_serializing)]
    copyright_en: Option<String>,
    /// 壁紙用の画像（保存先からの相対パス）。サムネイルだけ保管している間は None。
    #[serde(default)]
    pub image_file: Option<String>,
}

impl StoredImage {
    /// 表示言語に合う地域の見出しと説明。無ければ最初に保管したときのもの。
    fn texts(&self) -> (&str, &str) {
        let language = crate::i18n::current();
        for market in language.content_markets() {
            if let Some((t, c)) = self.localized.get(*market) {
                return (t, c);
            }
            if market.starts_with("en-") {
                if let (Some(t), Some(c)) = (&self.title_en, &self.copyright_en) {
                    return (t, c);
                }
            }
        }
        (&self.title, &self.copyright)
    }

    /// 表示言語に合わせた見出し。
    pub fn display_title(&self) -> String {
        let (title, copyright) = self.texts();
        bing::display_title(title, copyright)
    }

    /// 表示言語に合わせた説明（撮影地と著作権表示）。
    pub fn display_copyright(&self) -> &str {
        self.texts().1
    }

    /// 表示言語に合う地域の値。無ければ、ある中で最初のもの。
    fn localized_value<'a>(&self, values: &'a BTreeMap<String, String>) -> Option<&'a str> {
        crate::i18n::current()
            .content_markets()
            .iter()
            .find_map(|market| values.get(*market))
            .or_else(|| values.values().next())
            .map(String::as_str)
    }

    /// 表示言語に合わせた解説文。
    pub fn display_description(&self) -> Option<&str> {
        self.localized_value(&self.descriptions)
    }

    /// この写真について Bing で調べるページ。配信時のリンクが無い古い写真は、題名で検索する。
    pub fn bing_link(&self) -> String {
        match self.localized_value(&self.links) {
            Some(link) => link.to_string(),
            None => format!("https://www.bing.com/search?q={}", percent_encode(&self.display_title())),
        }
    }

    fn source(&self) -> PhotoSource {
        PhotoSource {
            key: self.key.clone(),
            startdate: self.startdate.clone(),
            title: self.title.clone(),
            copyright: self.copyright.clone(),
            urlbase: self.urlbase.clone(),
            mirror_url: self.mirror_url.clone(),
            localized: Vec::new(),
            descriptions: Vec::new(),
            links: Vec::new(),
        }
    }
}

/// URL の検索語として使えるように、英数字以外をパーセントエンコードする。
fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct Library {
    dir: PathBuf,
}

impl Library {
    pub fn new(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);
        let dir = dir.canonicalize().unwrap_or(dir);
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn meta_dir(&self) -> PathBuf {
        self.dir.join(".bing-wallpaper").join("meta")
    }

    pub fn thumb_path(&self, key: &str) -> PathBuf {
        self.dir.join(".bing-wallpaper").join("thumbs").join(format!("{key}.jpg"))
    }

    fn preview_path(&self, key: &str) -> PathBuf {
        self.dir.join(".bing-wallpaper").join("previews").join(format!("{key}.jpg"))
    }

    /// 壁紙用の画像のパス（ファイルがあるかは確かめない。描画中の判定など、頻繁に呼ぶところで使う）。
    pub fn image_path_unchecked(&self, image: &StoredImage) -> Option<PathBuf> {
        Some(self.dir.join(image.image_file.as_ref()?))
    }

    pub fn image_path(&self, image: &StoredImage) -> Option<PathBuf> {
        let path = self.dir.join(image.image_file.as_ref()?);
        path.is_file().then_some(path)
    }

    /// 大きく表示するのに使える、手元にある一番大きな画像。
    pub fn display_path(&self, image: &StoredImage) -> Option<PathBuf> {
        self.image_path(image)
            .or_else(|| Some(self.preview_path(&image.key)).filter(|p| p.is_file()))
    }

    /// 大きく表示するための中サイズの画像が要るか（サムネイルしか無い写真）。
    pub fn needs_preview(&self, image: &StoredImage) -> bool {
        // アーカイブの画像しか無い写真は、サムネイル自体が元の画像なので不要
        self.display_path(image).is_none() && !image.urlbase.is_empty()
    }

    /// 中サイズの画像を取得する。
    pub fn ensure_preview(&self, image: &StoredImage) -> Result<PathBuf> {
        let path = self.preview_path(&image.key);
        let url = image
            .source()
            .bing_url("1920x1080")
            .context(tr!("error.no_bing_image"))?;
        bing::download(&url, &path)?;
        Ok(path)
    }

    fn read_meta(&self, key: &str) -> Option<StoredImage> {
        let s = fs::read_to_string(self.meta_dir().join(format!("{key}.json"))).ok()?;
        serde_json::from_str(&s).ok()
    }

    fn write_meta(&self, meta: &StoredImage) -> Result<()> {
        fs::create_dir_all(self.meta_dir())?;
        let path = self.meta_dir().join(format!("{}.json", meta.key));
        fs::write(&path, serde_json::to_string_pretty(meta)?)
            .with_context(|| tr!("error.write", path = path.display()))
    }

    /// 保管済みの画像をすべて新しい順に返す。
    pub fn load_all(&self) -> Vec<StoredImage> {
        let mut images: Vec<StoredImage> = fs::read_dir(self.meta_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| serde_json::from_str(&fs::read_to_string(e.path()).ok()?).ok())
            .collect();
        images.sort_by(|a, b| b.startdate.cmp(&a.startdate).then(a.key.cmp(&b.key)));
        images
    }

    /// 写真を保管する。`with_image` なら壁紙用の画像まで、そうでなければサムネイルだけ取得する。
    /// 保管済みのファイルは再取得しない。
    pub fn store(&self, src: &PhotoSource, markets: &[&str], with_image: bool) -> Result<StoredImage> {
        let mut meta = self.read_meta(&src.key).unwrap_or_else(|| StoredImage {
            key: src.key.clone(),
            startdate: src.startdate.clone(),
            title: src.title.clone(),
            copyright: src.copyright.clone(),
            urlbase: src.urlbase.clone(),
            mirror_url: src.mirror_url.clone(),
            markets: Vec::new(),
            localized: BTreeMap::new(),
            descriptions: BTreeMap::new(),
            links: BTreeMap::new(),
            colors: None,
            favorite: false,
            title_en: None,
            copyright_en: None,
            image_file: None,
        });
        // 地域によって配信日がずれることがあるので、新しい方の日付を使う
        if src.startdate > meta.startdate {
            meta.startdate = src.startdate.clone();
        }
        if meta.urlbase.is_empty() && !src.urlbase.is_empty() {
            meta.urlbase = src.urlbase.clone();
        }
        if meta.mirror_url.is_none() {
            meta.mirror_url = src.mirror_url.clone();
        }
        for market in markets {
            if !meta.markets.iter().any(|m| m == market) {
                meta.markets.push(market.to_string());
            }
        }
        for (market, title, copyright) in &src.localized {
            meta.localized.insert(market.clone(), (title.clone(), copyright.clone()));
        }
        for (market, description) in &src.descriptions {
            meta.descriptions.insert(market.clone(), description.clone());
        }

        for (market, link) in &src.links {
            meta.links.insert(market.clone(), link.clone());
        }

        let thumb_url = src.thumb_url().context(tr!("error.no_image_url"))?;
        bing::download(&thumb_url, &self.thumb_path(&src.key))?;

        if with_image && self.image_path(&meta).is_none() {
            meta.image_file = Some(self.download_image(src)?);
        }
        self.write_meta(&meta)?;
        Ok(meta)
    }

    /// お気に入りの登録・解除を保存する。
    pub fn set_favorite(&self, image: &StoredImage, favorite: bool) -> Result<StoredImage> {
        let mut meta = self.read_meta(&image.key).unwrap_or_else(|| image.clone());
        meta.favorite = favorite;
        self.write_meta(&meta)?;
        Ok(meta)
    }

    /// 解説文を保存する（保管済みの写真だけ。無ければ None）。
    pub fn add_description(&self, key: &str, market: &str, description: &str) -> Option<StoredImage> {
        let mut meta = self.read_meta(key)?;
        if meta.descriptions.get(market).is_some_and(|d| d == description) {
            return None;
        }
        meta.descriptions.insert(market.to_string(), description.to_string());
        self.write_meta(&meta).ok()?;
        Some(meta)
    }

    fn deleted_path(&self) -> PathBuf {
        self.dir.join(".bing-wallpaper").join("deleted.txt")
    }

    /// 削除した写真のキー。取得し直さないように覚えておく。
    pub fn deleted_keys(&self) -> std::collections::HashSet<String> {
        fs::read_to_string(self.deleted_path())
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect()
    }

    fn write_deleted(&self, keys: &std::collections::HashSet<String>) -> Result<()> {
        let path = self.deleted_path();
        let mut keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        keys.sort_unstable();
        fs::write(&path, keys.join("\n")).with_context(|| tr!("error.write", path = path.display()))
    }

    /// 写真を削除する（壁紙用の画像・サムネイル・中サイズの画像・メタデータ）。
    /// 更新や古い写真の読み込みで戻ってこないように、キーを覚えておく。
    pub fn delete(&self, image: &StoredImage) -> Result<()> {
        let mut deleted = self.deleted_keys();
        deleted.insert(image.key.clone());
        self.write_deleted(&deleted)?;
        let files = [
            self.image_path_unchecked(image),
            Some(self.thumb_path(&image.key)),
            Some(self.preview_path(&image.key)),
            Some(self.meta_dir().join(format!("{}.json", image.key))),
        ];
        for path in files.into_iter().flatten() {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err).with_context(|| tr!("error.remove", path = path.display())),
            }
        }
        Ok(())
    }

    /// 削除した写真を、また保管できるようにする（今日の写真として配信されたとき）。
    pub fn forget_deleted(&self, key: &str) -> Result<()> {
        let mut deleted = self.deleted_keys();
        if deleted.remove(key) {
            self.write_deleted(&deleted)?;
        }
        Ok(())
    }

    /// 保存先の使用量（バイト数）。
    pub fn usage(&self) -> u64 {
        fn walk(dir: &Path) -> u64 {
            fs::read_dir(dir)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| match e.metadata() {
                    Ok(m) if m.is_dir() => walk(&e.path()),
                    Ok(m) => m.len(),
                    Err(_) => 0,
                })
                .sum()
        }
        walk(&self.dir)
    }

    /// 色の割合を計算して保存する（サムネイルから）。
    pub fn analyze_colors(&self, image: &StoredImage) -> Result<StoredImage> {
        let thumb = self.thumb_path(&image.key);
        let colors = crate::colors::analyze(&thumb)?;
        let mut meta = self.read_meta(&image.key).unwrap_or_else(|| image.clone());
        meta.colors = Some(colors);
        self.write_meta(&meta)?;
        Ok(meta)
    }

    /// 壁紙用の画像が無ければ取得する（アーカイブから読み込んだ古い写真を壁紙にするとき）。
    pub fn ensure_image(&self, image: &StoredImage) -> Result<(StoredImage, PathBuf)> {
        if let Some(path) = self.image_path(image) {
            return Ok((image.clone(), path));
        }
        let mut meta = image.clone();
        let file = self.download_image(&image.source())?;
        meta.image_file = Some(file.clone());
        self.write_meta(&meta)?;
        Ok((meta, self.dir.join(file)))
    }

    /// 壁紙用の画像を保存先に保存し、保存先からの相対パスを返す。
    /// UHD → 1920x1080 → アーカイブの画像 の順に試す。
    fn download_image(&self, src: &PhotoSource) -> Result<String> {
        let stem = format!("{}_{}", src.startdate, src.key);
        let mut candidates = Vec::new();
        if let Some(url) = src.bing_url("UHD") {
            candidates.push((url, format!("{stem}_UHD.jpg")));
        }
        if let Some(url) = src.bing_url("1920x1080") {
            candidates.push((url, format!("{stem}_1920x1080.jpg")));
        }
        if let Some(url) = &src.mirror_url {
            candidates.push((url.clone(), format!("{stem}_archive.jpg")));
        }
        let mut last_err = None;
        for (url, file) in candidates {
            match bing::download(&url, &self.dir.join(&file)) {
                Ok(()) => return Ok(file),
                Err(err) => last_err = Some(err),
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!(tr!("error.no_image_url"))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_library(name: &str) -> Library {
        let dir = std::env::temp_dir().join(format!("bwm-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Library::new(dir)
    }

    fn stored(library: &Library, key: &str) -> StoredImage {
        let meta: StoredImage = serde_json::from_value(serde_json::json!({
            "key": key, "startdate": "20261108", "title": "題名", "copyright": "場所 (© 撮影者)",
            "urlbase": "", "image_file": format!("20261108_{key}_UHD.jpg"),
        }))
        .unwrap();
        fs::create_dir_all(library.thumb_path(key).parent().unwrap()).unwrap();
        fs::write(library.thumb_path(key), b"thumb").unwrap();
        fs::write(library.dir().join(meta.image_file.as_ref().unwrap()), b"image").unwrap();
        library.write_meta(&meta).unwrap();
        meta
    }

    #[test]
    fn delete_removes_files_and_remembers_key() {
        let library = temp_library("delete");
        let image = stored(&library, "OHR.Foo");
        let other = stored(&library, "OHR.Bar");
        assert!(library.usage() > 0);
        library.delete(&image).unwrap();
        assert!(library.image_path(&image).is_none());
        assert!(!library.thumb_path("OHR.Foo").exists());
        assert_eq!(library.load_all().iter().map(|s| s.key.as_str()).collect::<Vec<_>>(), ["OHR.Bar"]);
        assert!(library.image_path(&other).is_some());
        assert!(library.deleted_keys().contains("OHR.Foo"));
        library.forget_deleted("OHR.Foo").unwrap();
        assert!(library.deleted_keys().is_empty());
        fs::remove_dir_all(library.dir()).unwrap();
    }

    #[test]
    fn description_and_link_follow_language() {
        let library = temp_library("description");
        stored(&library, "OHR.Foo");
        let meta = library.add_description("OHR.Foo", "en-US", "A lake.").unwrap();
        assert_eq!(meta.display_description(), Some("A lake."));
        // 同じ解説文は書き直さない
        assert!(library.add_description("OHR.Foo", "en-US", "A lake.").is_none());
        assert!(library.add_description("OHR.Missing", "en-US", "x").is_none());
        assert_eq!(meta.bing_link(), "https://www.bing.com/search?q=%E9%A1%8C%E5%90%8D");
        fs::remove_dir_all(library.dir()).unwrap();
    }
}
