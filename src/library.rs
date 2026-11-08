//! 保存先ディレクトリに画像を保管するライブラリ。
//!
//! ```text
//! <保存先>/
//!   20261108_OHR.Foo_UHD.jpg        壁紙用の画像（UHD、無ければ 1920x1080）
//!   .bing-wallpaper/meta/<key>.json  タイトルなどのメタデータ（key は OHR.Foo のような写真名）
//!   .bing-wallpaper/thumbs/<key>.jpg ダッシュボード用サムネイル (640x360)
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use crate::bing::{self, BingImage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredImage {
    pub key: String,
    pub startdate: String,
    pub title: String,
    pub copyright: String,
    pub urlbase: String,
    /// この画像が配信された地域。
    #[serde(default)]
    pub markets: Vec<String>,
    /// 保存先からの相対パス。
    #[serde(default)]
    pub image_file: Option<String>,
}

impl StoredImage {
    pub fn display_title(&self) -> String {
        bing::display_title(&self.title, &self.copyright)
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

    pub fn image_path(&self, image: &StoredImage) -> Option<PathBuf> {
        let path = self.dir.join(image.image_file.as_ref()?);
        path.is_file().then_some(path)
    }

    fn read_meta(&self, key: &str) -> Option<StoredImage> {
        let s = fs::read_to_string(self.meta_dir().join(format!("{key}.json"))).ok()?;
        serde_json::from_str(&s).ok()
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

    /// 画像をダウンロードして保管する（保管済みのファイルは再取得しない）。
    pub fn store(&self, img: &BingImage, market: &str) -> Result<StoredImage> {
        let key = img.key();
        let mut meta = self.read_meta(&key).unwrap_or_else(|| StoredImage {
            key: key.clone(),
            startdate: img.startdate.clone(),
            title: img.title.clone(),
            copyright: img.copyright.clone(),
            urlbase: img.urlbase.clone(),
            markets: Vec::new(),
            image_file: None,
        });
        // 地域によって配信日がずれることがあるので、新しい方の日付を使う
        if img.startdate > meta.startdate {
            meta.startdate = img.startdate.clone();
        }
        if !meta.markets.iter().any(|m| m == market) {
            meta.markets.push(market.to_string());
        }

        bing::download(&img.url("640x360"), &self.thumb_path(&key))?;

        if self.image_path(&meta).is_none() {
            let stem = format!("{}_{key}", img.startdate);
            let uhd = format!("{stem}_UHD.jpg");
            let file = match bing::download(&img.url("UHD"), &self.dir.join(&uhd)) {
                Ok(()) => uhd,
                Err(err) => {
                    let fhd = format!("{stem}_1920x1080.jpg");
                    bing::download(&img.url("1920x1080"), &self.dir.join(&fhd))
                        .map_err(|e| e.context(err))?;
                    fhd
                }
            };
            meta.image_file = Some(file);
        }

        fs::create_dir_all(self.meta_dir())?;
        let path = self.meta_dir().join(format!("{key}.json"));
        fs::write(&path, serde_json::to_string_pretty(&meta)?)
            .with_context(|| format!("{} に書き込めません", path.display()))?;
        Ok(meta)
    }
}
