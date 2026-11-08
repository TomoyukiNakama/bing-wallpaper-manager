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
    /// 英語圏の地域で配信された見出しと説明（英語表示で使う）。
    pub english: Option<(String, String)>,
}

impl PhotoSource {
    /// 公式 API の 1 枚。英語圏の地域（`en-`）なら、その見出しと説明を英語版としても持つ。
    pub fn from_bing(img: &BingImage, market: &str) -> Self {
        Self {
            key: img.key(),
            startdate: img.startdate.clone(),
            title: img.title.clone(),
            copyright: img.copyright.clone(),
            urlbase: img.urlbase.clone(),
            mirror_url: None,
            english: market
                .starts_with("en-")
                .then(|| (img.title.clone(), img.copyright.clone())),
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
    /// 英語圏の地域で配信されたときの見出しと説明（英語表示で使う）
    #[serde(default)]
    pub title_en: Option<String>,
    #[serde(default)]
    pub copyright_en: Option<String>,
    /// 壁紙用の画像（保存先からの相対パス）。サムネイルだけ保管している間は None。
    #[serde(default)]
    pub image_file: Option<String>,
}

impl StoredImage {
    /// 表示言語に合わせた見出し。
    pub fn display_title(&self) -> String {
        match (crate::i18n::is_en(), &self.title_en, &self.copyright_en) {
            (true, Some(t), Some(c)) => bing::display_title(t, c),
            _ => bing::display_title(&self.title, &self.copyright),
        }
    }

    /// 表示言語に合わせた説明（撮影地と著作権表示）。
    pub fn display_copyright(&self) -> &str {
        match (crate::i18n::is_en(), &self.copyright_en) {
            (true, Some(c)) => c,
            _ => &self.copyright,
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
            english: None,
        }
    }
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
            .context(tr!("この写真には Bing の画像がありません", "This photo has no Bing image"))?;
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
            .with_context(|| tr!(format!("{} に書き込めません", path.display()), format!("Cannot write {}", path.display())))
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
        if let Some((title, copyright)) = &src.english {
            meta.title_en = Some(title.clone());
            meta.copyright_en = Some(copyright.clone());
        }

        let thumb_url = src.thumb_url().context(tr!("この写真には画像の URL がありません", "This photo has no image URL"))?;
        bing::download(&thumb_url, &self.thumb_path(&src.key))?;

        if with_image && self.image_path(&meta).is_none() {
            meta.image_file = Some(self.download_image(src)?);
        }
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
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!(tr!("この写真には画像の URL がありません", "This photo has no image URL"))))
    }
}
