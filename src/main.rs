#[macro_use]
mod i18n;

rust_i18n::i18n!("locales", fallback = "en");

mod archive;
mod bing;
mod colors;
mod config;
mod library;
mod search;
mod settings;
mod theme;
mod wallpaper;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{
    Disableable as _, Selectable as _, Icon, IconName, Sizable as _, TitleBar, WindowExt as _, h_flex,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use bing::{BingImage, MARKETS};
use config::{Config, FitMode, Language, ThemePref};
use archive::ArchivePhoto;
use library::{Library, PhotoSource, StoredImage};
use colors::ColorBucket;
use search::Query;
use settings::SettingsView;
use theme::Palette;

gpui_kit::actions!(
    bing_wallpaper,
    [SelectPrevious, SelectNext, ApplySelected, Refresh, OpenSettings, FocusSearch, CloseSearch, ToggleFavorite]
);

/// キーボード操作（ダッシュボードにフォーカスがあるとき）。
fn key_bindings() -> Vec<KeyBinding> {
    let ctx = Some("Dashboard");
    vec![
        KeyBinding::new("left", SelectPrevious, ctx),
        KeyBinding::new("right", SelectNext, ctx),
        KeyBinding::new("enter", ApplySelected, ctx),
        KeyBinding::new("f5", Refresh, ctx),
        KeyBinding::new("ctrl-r", Refresh, ctx),
        KeyBinding::new("ctrl-,", OpenSettings, ctx),
        KeyBinding::new("ctrl-f", FocusSearch, ctx),
        KeyBinding::new("ctrl-d", ToggleFavorite, ctx),
        KeyBinding::new("escape", CloseSearch, ctx),
    ]
}

/// 「過去の写真」を一度に表示する枚数。
const PAST_PAGE: usize = 48;
/// 「さらに古い写真を読み込む」で一度に読み込む日数。
const OLDER_DAYS: usize = 30;
/// タイトルバーとフッターの高さの合計（大きな写真の高さの計算用）。
const CHROME_HEIGHT: Pixels = px(64.);
/// 大きな写真を固定しても、一覧に必ず残す高さ。
const LIST_MIN_HEIGHT: Pixels = px(260.);
/// 一覧をスクロールしたときの、大きな写真の細い帯の高さ。
const HERO_COLLAPSED_HEIGHT: f32 = 140.;
/// 大きな写真の切り替え（クロスフェード）と、高さの変化にかける時間。
const HERO_FADE: Duration = Duration::from_millis(600);
const HERO_RESIZE: Duration = Duration::from_millis(300);

enum LatestState {
    Loading,
    Ready,
    Failed,
}

/// 地域 1 つ分の最新の写真。
struct MarketLatest {
    info: Option<BingImage>,
    state: LatestState,
}

/// 「今日の写真」の 1 枚。複数の地域で同じ写真が配信されることが多いのでまとめる。
struct TodayPhoto {
    key: String,
    /// 表示に使う情報（最初に見つかった地域＝日本を優先）
    info: BingImage,
    markets: Vec<usize>,
}

/// 大きく表示する写真の情報。
struct Hero {
    key: String,
    title: String,
    description: String,
    startdate: String,
    markets: String,
    image: Option<PathBuf>,
    can_apply: bool,
    /// 壁紙用の画像が手元にあるか（「画像を開く」などに使う）
    can_open: bool,
    current: bool,
}

struct Dashboard {
    config: Config,
    library: Library,
    stored: Vec<StoredImage>,
    latest: Vec<MarketLatest>,
    /// 大きく表示している写真のキー（None なら今日の最初の写真）
    selected: Option<String>,
    /// 実際に大きく表示している写真のキー（描画のたびに更新）
    shown: Option<String>,
    current_uri: Option<String>,
    generation: u64,
    refreshing: bool,
    applying: bool,
    progress: SharedString,
    status: SharedString,
    is_error: bool,
    past_limit: usize,
    /// アーカイブの一覧（最初に「さらに古い写真を読み込む」を押したときに取得）
    archive: Option<Rc<Vec<ArchivePhoto>>>,
    loading_older: bool,
    /// アーカイブにもこれより古い写真が無い
    older_exhausted: bool,
    /// 中サイズの画像を取得中（または取得を試みた）写真
    previews_pending: HashSet<String>,
    focus_handle: FocusHandle,
    /// 一覧のスクロール位置（キーボードで選んだ写真を見える位置に動かすため自前で持つ）
    list_scroll: ScrollHandle,
    /// 前回描画したときの各タイルの位置
    tile_bounds: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    /// 「写真全体を表示」などで余った部分に GNOME が塗る色（設定画面のプレビュー用）
    margin_color: Hsla,
    /// 検索欄と検索条件
    search_input: Entity<InputState>,
    query: Query,
    /// 色見本の帯を出しているか（検索欄を使い始めたら出し、閉じるまで出したまま）
    search_open: bool,
    /// 検索結果を一度に表示する枚数
    results_limit: usize,
    /// 保存済みの写真にアーカイブの解説文を補ったか / 補っている最中か
    descriptions_ready: bool,
    indexing: bool,
    /// 色の解析中
    analyzing_colors: bool,
    /// 大きく表示している写真（キーと画像）。選んだ写真の画像の読み込みが終わるまで、前の写真を出し続ける
    hero_front: Option<(String, Option<PathBuf>)>,
    /// 切り替えのときに下に残す、前の写真の画像（クロスフェード用）
    hero_back: Option<PathBuf>,
    /// 切り替えのたびに増やす（フェードのアニメーションをやり直すため）
    hero_swap: usize,
    /// 一覧をスクロールして大きな写真を細い帯にしているか
    hero_collapsed: bool,
    /// 高さのアニメーション（開始の高さ, 終了の高さ, 識別番号）
    hero_height: (f32, f32, usize),
    _appearance: Subscription,
    _search_events: Subscription,
}

fn markets_label(markets: impl IntoIterator<Item = String>) -> String {
    let names: Vec<String> = markets.into_iter().collect();
    if names.len() == MARKETS.len() {
        tr!("hero.shown_all", count = names.len())
    } else {
        tr!("hero.shown_in", regions = names.join(tr!("list.separator").as_ref()))
    }
}

/// タイル用の短い地域表記。
fn regions_caption(names: &[String]) -> String {
    match names.len() {
        n if n == MARKETS.len() => tr!("tile.all_regions").to_string(),
        0..=3 => names.join(tr!("list.separator").as_ref()),
        n => tr!("tile.regions_more", regions = names[..2].join(tr!("list.separator").as_ref()), count = n - 2),
    }
}

impl Dashboard {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = Config::load();
        let library = Library::new(config.save_dir());
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            theme::apply(window, cx);
        });
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder(tr!("search.placeholder")));
        let search_events = cx.subscribe_in(&search_input, window, |this, input, event: &InputEvent, _, cx| {
            match event {
                InputEvent::Change => {
                    this.query.text = input.read(cx).value().to_string();
                    this.on_query_changed(cx);
                }
                InputEvent::Focus => {
                    this.search_open = true;
                    this.ensure_descriptions(cx);
                    cx.notify();
                }
                _ => {}
            }
        });
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let mut this = Self {
            focus_handle,
            list_scroll: ScrollHandle::new(),
            tile_bounds: Rc::default(),
            margin_color: wallpaper::primary_color().map(|c| rgb(c).into()).unwrap_or(black()),
            stored: library.load_all(),
            config,
            library,
            latest: MARKETS
                .iter()
                .map(|_| MarketLatest { info: None, state: LatestState::Loading })
                .collect(),
            selected: None,
            shown: None,
            current_uri: wallpaper::current_uri(),
            generation: 0,
            refreshing: false,
            applying: false,
            progress: "".into(),
            status: "".into(),
            is_error: false,
            past_limit: PAST_PAGE,
            archive: None,
            loading_older: false,
            older_exhausted: false,
            previews_pending: HashSet::new(),
            search_input,
            query: Query::default(),
            search_open: false,
            results_limit: PAST_PAGE,
            descriptions_ready: false,
            indexing: false,
            analyzing_colors: false,
            hero_front: None,
            hero_back: None,
            hero_swap: 0,
            hero_collapsed: false,
            hero_height: (0., 0., 0),
            _appearance: appearance,
            _search_events: search_events,
        };
        this.refresh(cx);
        this.analyze_missing_colors(cx);
        this
    }

    fn set_status(&mut self, msg: impl Into<SharedString>, is_error: bool) {
        self.status = msg.into();
        self.is_error = is_error;
    }

    fn find_stored(&self, key: &str) -> Option<&StoredImage> {
        self.stored.iter().find(|s| s.key == key)
    }

    fn upsert(&mut self, image: StoredImage) {
        match self.stored.iter_mut().find(|s| s.key == image.key) {
            Some(slot) => *slot = image,
            None => {
                self.stored.push(image);
                self.stored
                    .sort_by(|a, b| b.startdate.cmp(&a.startdate).then(a.key.cmp(&b.key)));
            }
        }
    }

    fn is_current(&self, image: &StoredImage) -> bool {
        match (&self.current_uri, self.library.image_path(image)) {
            (Some(uri), Some(path)) => *uri == wallpaper::file_uri(&path),
            _ => false,
        }
    }

    fn today_photos(&self) -> Vec<TodayPhoto> {
        let mut photos: Vec<TodayPhoto> = Vec::new();
        for (i, latest) in self.latest.iter().enumerate() {
            let Some(info) = &latest.info else { continue };
            let key = info.key();
            match photos.iter_mut().find(|p| p.key == key) {
                Some(photo) => photo.markets.push(i),
                None => photos.push(TodayPhoto { key, info: info.clone(), markets: vec![i] }),
            }
        }
        // 見出しと説明は、表示言語に合う地域で配信されたものを優先する
        let preferred = i18n::current().content_markets();
        for photo in &mut photos {
            let best = preferred
                .iter()
                .find_map(|code| photo.markets.iter().find(|&&i| MARKETS[i].0 == *code));
            if let Some(info) = best.and_then(|&i| self.latest[i].info.clone()) {
                photo.info = info;
            }
        }
        photos
    }

    fn hero(&self, today: &[TodayPhoto]) -> Option<Hero> {
        let key = self
            .selected
            .clone()
            .or_else(|| today.first().map(|p| p.key.clone()))
            .or_else(|| self.stored.first().map(|s| s.key.clone()))?;
        self.hero_for(key, today)
    }

    fn hero_for(&self, key: String, today: &[TodayPhoto]) -> Option<Hero> {
        let stored = self.find_stored(&key);
        let image = stored
            .and_then(|s| self.library.display_path(s))
            .or_else(|| Some(self.library.thumb_path(&key)).filter(|p| p.is_file()));
        // サムネイルしか無い古い写真も、「壁紙にする」で画像を取得して設定できる
        let can_apply = stored.is_some();
        let can_open = stored.is_some_and(|s| self.library.image_path(s).is_some());
        let current = stored.is_some_and(|s| self.is_current(s));

        if let Some(photo) = today.iter().find(|p| p.key == key) {
            return Some(Hero {
                title: photo.info.display_title(),
                description: photo.info.copyright.clone(),
                startdate: photo.info.startdate.clone(),
                markets: markets_label(photo.markets.iter().map(|&i| bing::market_name(MARKETS[i].0).to_string())),
                key,
                image,
                can_apply,
                can_open,
                current,
            });
        }
        let s = stored?;
        Some(Hero {
            title: s.display_title(),
            description: s.display_copyright().to_string(),
            startdate: s.startdate.clone(),
            markets: markets_label(s.markets.iter().map(|m| bing::market_name(m).to_string())),
            key,
            image,
            can_apply,
            can_open,
            current,
        })
    }

    /// 全地域の画像一覧を取得し、最新のものから順に保存先へ保管する。
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        self.progress = tr!("status.checking").into();
        for latest in &mut self.latest {
            latest.state = LatestState::Loading;
        }
        cx.notify();

        let generation = self.generation;
        let library = self.library.clone();
        cx.spawn(async move |this, cx| {
            let tasks: Vec<_> = MARKETS
                .iter()
                .map(|(code, _, _)| {
                    let code = code.to_string();
                    cx.background_executor()
                        .spawn(async move { bing::fetch_recent(&code) })
                })
                .collect();
            let mut archives = Vec::new();
            for task in tasks {
                archives.push(task.await);
            }

            // 各地域の最新 → それ以前、の順に保管する
            let mut queue: Vec<(usize, BingImage, bool)> = Vec::new();
            for (i, archive) in archives.iter().enumerate() {
                if let Ok(images) = archive {
                    queue.push((i, images[0].clone(), true));
                }
            }
            for (i, archive) in archives.iter().enumerate() {
                if let Ok(images) = archive {
                    queue.extend(images.iter().skip(1).map(|img| (i, img.clone(), false)));
                }
            }

            let ok = this
                .update(cx, |this, cx| {
                    if this.generation != generation {
                        return false;
                    }
                    for (latest, archive) in this.latest.iter_mut().zip(&archives) {
                        match archive {
                            Ok(images) => latest.info = Some(images[0].clone()),
                            Err(_) => {
                                latest.state = LatestState::Failed
                            }
                        }
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !ok {
                return;
            }

            let total = queue.len();
            let mut failures = archives.iter().filter(|a| a.is_err()).count();
            for (n, (i, image, is_latest)) in queue.into_iter().enumerate() {
                let lib = library.clone();
                let market = MARKETS[i].0;
                let result = cx
                    .background_executor()
                    .spawn(async move { lib.store(&PhotoSource::from_bing(&image, market), &[market], true) })
                    .await;
                let ok = this
                    .update(cx, |this, cx| {
                        if this.generation != generation {
                            return false;
                        }
                        match result {
                            Ok(stored) => {
                                this.upsert(stored);
                                if is_latest {
                                    this.latest[i].state = LatestState::Ready;
                                }
                            }
                            Err(err) => {
                                failures += 1;
                                eprintln!("store failed: {err:#}");
                                if is_latest {
                                    this.latest[i].state =
                                        LatestState::Failed;
                                }
                            }
                        }
                        this.progress = tr!("status.saving", done = n + 1, total = total)
                        .into();
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !ok {
                    return;
                }
            }

            this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.refreshing = false;
                this.analyze_missing_colors(cx);
                if failures == 0 {
                    this.set_status(tr!("status.refreshed"), false);
                } else {
                    this.set_status(
                        tr!("status.refresh_failed", count = failures),
                        true,
                    );
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn apply(&mut self, key: String, cx: &mut Context<Self>) {
        if self.applying {
            return;
        }
        let Some(stored) = self.find_stored(&key).cloned() else { return };
        let title = stored.display_title();
        if self.library.image_path(&stored).is_none() {
            // アーカイブから読み込んだ古い写真は、ここで壁紙用の画像を取得する
            self.set_status(
                tr!("status.downloading", title = title),
                false,
            );
        }
        self.applying = true;
        let fit = self.config.fit;
        let library = self.library.clone();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let (meta, path) = library.ensure_image(&stored)?;
                    wallpaper::set_wallpaper(&path, fit)?;
                    anyhow::Ok((meta, path))
                })
                .await;
            this.update(cx, |this, cx| {
                this.applying = false;
                match result {
                    Ok((meta, path)) => {
                        this.upsert(meta);
                        this.current_uri = Some(wallpaper::file_uri(&path));
                        this.set_status(
                            tr!("status.applied", title = title),
                            false,
                        );
                    }
                    Err(err) => this.set_status(format!("{err:#}"), true),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// サムネイルしか無い写真を大きく表示するときに、中サイズの画像を取得する。
    fn request_preview(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(stored) = self.find_stored(key).cloned() else { return };
        if !self.library.needs_preview(&stored) || !self.previews_pending.insert(key.to_string()) {
            return;
        }
        let library = self.library.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { library.ensure_preview(&stored) })
                .await;
            if let Err(err) = result {
                eprintln!("preview download failed: {err:#}");
            }
            this.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
    }

    /// アーカイブから、まだ保管していない写真を新しい方から約 1 か月分読み込む。
    /// サムネイルだけを保管し、壁紙用の画像は「壁紙にする」ときに取得する。
    fn load_older(&mut self, cx: &mut Context<Self>) {
        if self.loading_older || self.older_exhausted {
            return;
        }
        self.loading_older = true;
        self.progress = tr!("status.older_listing").into();
        cx.notify();

        let generation = self.generation;
        let library = self.library.clone();
        let cached = self.archive.clone();
        let stored_keys: HashSet<String> = self.stored.iter().map(|s| s.key.clone()).collect();
        cx.spawn(async move |this, cx| {
            let archive = match cached {
                Some(archive) => archive,
                None => {
                    let (photos, errors) =
                        cx.background_executor().spawn(async move { archive::fetch_all() }).await;
                    for err in &errors {
                        eprintln!("archive: {err:#}");
                    }
                    if photos.is_empty() {
                        let message = match errors.first() {
                            Some(err) => tr!("status.older_list_failed", error = format!("{err:#}")),
                            None => tr!("status.archive_empty").to_string(),
                        };
                        this.update(cx, |this, cx| {
                            this.loading_older = false;
                            this.set_status(message, true);
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                    let archive = Rc::new(photos);
                    this.update(cx, |this, _| this.archive = Some(archive.clone())).ok();
                    archive
                }
            };

            // まだ保管していない写真を、新しい日付から OLDER_DAYS 日分
            // （日付ではなく写真で判定するので、途中に抜けがあっても埋まる）
            let mut dates: Vec<&str> = Vec::new();
            let batch: Vec<ArchivePhoto> = archive
                .iter()
                .filter(|p| !stored_keys.contains(&p.source.key))
                .take_while(|p| {
                    let date = p.source.startdate.as_str();
                    if dates.last() != Some(&date) {
                        dates.push(date);
                    }
                    dates.len() <= OLDER_DAYS
                })
                .cloned()
                .collect();

            if batch.is_empty() {
                this.update(cx, |this, cx| {
                    this.loading_older = false;
                    this.older_exhausted = true;
                    this.set_status(tr!("status.archive_exhausted"), false);
                    cx.notify();
                })
                .ok();
                return;
            }

            let total = batch.len();
            let range = format!(
                "{}{}{}",
                bing::date_text(&batch[total - 1].source.startdate, true),
                tr!("list.range_separator"),
                bing::date_text(&batch[0].source.startdate, true)
            );
            let mut loaded = 0;
            for (n, photo) in batch.into_iter().enumerate() {
                let lib = library.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move { lib.store(&photo.source, &photo.markets, false) })
                    .await;
                let ok = this
                    .update(cx, |this, cx| {
                        if this.generation != generation {
                            return false;
                        }
                        match result {
                            Ok(stored) => {
                                loaded += 1;
                                this.upsert(stored);
                                this.past_limit += 1;
                            }
                            Err(err) => eprintln!("archive store failed: {err:#}"),
                        }
                        this.progress = tr!("status.older_loading", done = n + 1, total = total)
                        .into();
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !ok {
                    return;
                }
            }

            this.update(cx, |this, cx| {
                this.loading_older = false;
                this.analyze_missing_colors(cx);
                if loaded == 0 {
                    this.set_status(
                        tr!("status.older_failed"),
                        true,
                    );
                } else {
                    this.set_status(
                        tr!("status.older_loaded", range = range, count = loaded),
                        false,
                    );
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn choose_save_dir(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr!("dialog.choose_dir_prompt").into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = rx.await else { return };
            if let Some(dir) = paths.pop() {
                this.update(cx, |this, cx| this.set_save_dir(dir, cx)).ok();
            }
        })
        .detach();
    }

    fn set_save_dir(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        self.config.save_dir = Some(dir);
        if let Err(err) = self.config.save() {
            self.set_status(format!("{err:#}"), true);
        }
        self.library = Library::new(self.config.save_dir());
        self.stored = self.library.load_all();
        self.past_limit = PAST_PAGE;
        self.older_exhausted = false;
        self.loading_older = false;
        self.generation += 1;
        self.refreshing = false;
        self.set_status(
            tr!("status.storage_changed", path = self.library.dir().display()),
            false,
        );
        self.refresh(cx);
    }

    fn select(&mut self, key: String, cx: &mut Context<Self>) {
        self.selected = Some(key.clone());
        if self.hero_collapsed {
            self.hero_collapsed = false;
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(HERO_RESIZE + Duration::from_millis(50)).await;
                this.update(cx, |this, cx| {
                    this.ensure_visible(&key);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }

    /// 一覧のスクロール位置に合わせて、大きな写真を細い帯にする（一番上に戻ったら広げる）。
    fn on_list_scrolled(&mut self, cx: &mut Context<Self>) {
        let collapse = self.list_scroll.offset().y < px(-24.);
        if collapse != self.hero_collapsed {
            self.hero_collapsed = collapse;
            cx.notify();
        }
        self.load_more_if_near_end(cx);
    }

    /// 表示していない保存済みの写真の数（検索中は検索結果のうち表示していない数）。
    fn hidden_count(&self) -> usize {
        if self.query.is_active() {
            search::filter(&self.stored, &self.query).len().saturating_sub(self.results_limit)
        } else {
            let today: HashSet<String> = self.today_photos().into_iter().map(|t| t.key).collect();
            let past = self.stored.iter().filter(|s| !today.contains(&s.key)).count();
            past.saturating_sub(self.past_limit)
        }
    }

    /// 無限スクロール: 一覧の下端に近づいたら（または一覧が画面に収まっていたら）、保存済みの写真の続きを表示する。
    /// まだダウンロードしていない写真は「さらに古い写真を読み込む」ボタンで取得する。
    fn load_more_if_near_end(&mut self, cx: &mut Context<Self>) {
        let remaining_px = self.list_scroll.max_offset().y + self.list_scroll.offset().y;
        if remaining_px > px(800.) || self.hidden_count() == 0 {
            return;
        }
        if self.query.is_active() {
            self.results_limit += PAST_PAGE;
        } else {
            self.past_limit += PAST_PAGE;
        }
        cx.notify();
    }

    /// 広げたときの大きな写真の高さ（一覧の領域 LIST_MIN_HEIGHT を必ず残す）。
    fn hero_full_height(window: &Window) -> f32 {
        let viewport = window.viewport_size().height;
        f32::from(
            (viewport * 0.5)
                .min(viewport - CHROME_HEIGHT - LIST_MIN_HEIGHT)
                .clamp(px(200.), px(600.)),
        )
    }

    /// 今の壁紙が、このアプリの保存先の写真かどうか。
    fn wallpaper_is_ours(&self) -> bool {
        let dir = wallpaper::file_uri(self.library.dir());
        self.current_uri.as_ref().is_some_and(|uri| uri.starts_with(&dir))
    }

    /// 表示方法を変える。今の壁紙がこのアプリの写真なら、すぐ反映する。
    fn set_fit(&mut self, fit: FitMode, cx: &mut Context<Self>) {
        if self.config.fit == fit {
            return;
        }
        self.config.fit = fit;
        if let Err(err) = self.config.save() {
            self.set_status(format!("{err:#}"), true);
            cx.notify();
            return;
        }
        self.set_status(
            tr!("status.fit_changed", fit = fit.label()),
            false,
        );
        cx.notify();
        if self.wallpaper_is_ours() {
            cx.spawn(async move |this, cx| {
                let result = cx.background_executor().spawn(async move { wallpaper::set_fit(fit) }).await;
                if let Err(err) = result {
                    this.update(cx, |this, cx| {
                        this.set_status(format!("{err:#}"), true);
                        cx.notify();
                    })
                    .ok();
                }
            })
            .detach();
        }
    }

    /// 設定画面のプレビューに使う、表示中の写真とその実際の大きさ。
    fn preview_photo(&self) -> Option<(PathBuf, Size<f32>)> {
        let key = self.shown.as_ref()?;
        let stored = self.find_stored(key);
        if let Some(path) = stored.and_then(|s| self.library.image_path(s)) {
            let uhd = path.to_string_lossy().ends_with("_UHD.jpg");
            let photo = if uhd { size(3840., 2160.) } else { size(1920., 1080.) };
            return Some((path, photo));
        }
        let thumb = self.library.thumb_path(key);
        thumb.is_file().then(|| (thumb, size(3840., 2160.)))
    }

    fn open_save_dir(&mut self, cx: &mut Context<Self>) {
        if let Err(err) = wallpaper::open_folder(self.library.dir()) {
            self.set_status(format!("{err:#}"), true);
            cx.notify();
        }
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dashboard = cx.entity();
        let language = self.config.language;
        let view = cx.new(|cx| SettingsView::new(dashboard, language, window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(tr!("settings.title")).w(px(880.)).child(view.clone())
        });
    }

    /// 表示言語を変える（すぐに画面全体へ反映する）。
    fn set_language(&mut self, language: Language, window: &mut Window, cx: &mut Context<Self>) {
        if self.config.language == language {
            return;
        }
        self.config.language = language;
        i18n::set(language);
        // 状態表示は前の言語で書かれているので消す
        self.set_status("", false);
        self.search_input
            .update(cx, |input, cx| input.set_placeholder(tr!("search.placeholder"), window, cx));
        self.save_config(cx);
        window.refresh();
        cx.notify();
    }

    /// 配色を変える（すぐに画面全体へ反映する）。
    fn set_theme(&mut self, pref: ThemePref, window: &mut Window, cx: &mut Context<Self>) {
        if self.config.theme == pref {
            return;
        }
        self.config.theme = pref;
        theme::set_pref(pref);
        theme::apply(window, cx);
        self.save_config(cx);
        window.refresh();
        cx.notify();
    }

    fn save_config(&mut self, cx: &mut Context<Self>) {
        if let Err(err) = self.config.save() {
            self.set_status(format!("{err:#}"), true);
            cx.notify();
        }
    }

    fn on_query_changed(&mut self, cx: &mut Context<Self>) {
        self.results_limit = PAST_PAGE;
        if self.query.is_active() {
            self.search_open = true;
            self.ensure_descriptions(cx);
        }
        cx.notify();
    }

    fn is_favorite(&self, key: &str) -> bool {
        self.find_stored(key).is_some_and(|s| s.favorite)
    }

    /// お気に入りの登録・解除。画面はすぐ切り替え、保存は裏で行う。
    fn toggle_favorite(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(image) = self.find_stored(key).cloned() else { return };
        let favorite = !image.favorite;
        let mut updated = image.clone();
        updated.favorite = favorite;
        self.upsert(updated);
        cx.notify();
        let library = self.library.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { library.set_favorite(&image, favorite) })
                .await;
            if let Err(err) = result {
                this.update(cx, |this, cx| {
                    this.set_status(format!("{err:#}"), true);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// お気に入りだけを表示する／やめる。
    fn toggle_favorites_filter(&mut self, cx: &mut Context<Self>) {
        self.query.favorites = !self.query.favorites;
        self.search_open = self.search_open || self.query.favorites;
        self.on_query_changed(cx);
    }

    fn toggle_color(&mut self, color: ColorBucket, cx: &mut Context<Self>) {
        self.query.color = if self.query.color == Some(color) { None } else { Some(color) };
        self.on_query_changed(cx);
    }

    fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = true;
        self.search_input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    /// 検索を閉じて、いつもの一覧に戻る。
    fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_open && !self.query.is_active() {
            return;
        }
        self.search_input.update(cx, |input, cx| input.set_value("", window, cx));
        self.query = Query::default();
        self.search_open = false;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// 保存済みの写真に、アーカイブの解説文を補う（最初に検索したときに一度だけ）。
    /// 写真のファイルは取得し直さず、メタデータだけを更新する。
    fn ensure_descriptions(&mut self, cx: &mut Context<Self>) {
        if self.descriptions_ready || self.indexing {
            return;
        }
        self.indexing = true;
        self.progress = tr!("status.indexing").into();
        cx.notify();

        let generation = self.generation;
        let library = self.library.clone();
        let cached = self.archive.clone();
        let stored_keys: HashSet<String> = self.stored.iter().map(|s| s.key.clone()).collect();
        cx.spawn(async move |this, cx| {
            let archive = match cached {
                Some(archive) => Some(archive),
                None => {
                    let (photos, errors) =
                        cx.background_executor().spawn(async move { archive::fetch_all() }).await;
                    for err in &errors {
                        eprintln!("archive: {err:#}");
                    }
                    (!photos.is_empty()).then(|| Rc::new(photos))
                }
            };
            let Some(archive) = archive else {
                this.update(cx, |this, cx| {
                    this.indexing = false;
                    this.set_status(tr!("status.indexing_failed"), true);
                    cx.notify();
                })
                .ok();
                return;
            };
            this.update(cx, |this, _| this.archive = Some(archive.clone())).ok();

            let targets: Vec<ArchivePhoto> = archive
                .iter()
                .filter(|p| stored_keys.contains(&p.source.key) && !p.source.descriptions.is_empty())
                .cloned()
                .collect();
            let updated = cx
                .background_executor()
                .spawn(async move {
                    targets
                        .iter()
                        .filter_map(|p| library.store(&p.source, &p.markets, false).ok())
                        .collect::<Vec<_>>()
                })
                .await;
            this.update(cx, |this, cx| {
                this.indexing = false;
                if this.generation != generation {
                    return;
                }
                this.descriptions_ready = true;
                for image in updated {
                    this.upsert(image);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// まだ色を解析していない写真を、サムネイルから解析する。
    fn analyze_missing_colors(&mut self, cx: &mut Context<Self>) {
        if self.analyzing_colors {
            return;
        }
        let targets: Vec<StoredImage> = self
            .stored
            .iter()
            .filter(|s| s.colors.is_none() && self.library.thumb_path(&s.key).is_file())
            .cloned()
            .collect();
        if targets.is_empty() {
            return;
        }
        self.analyzing_colors = true;
        let generation = self.generation;
        let library = self.library.clone();
        let total = targets.len();
        cx.spawn(async move |this, cx| {
            for (n, image) in targets.into_iter().enumerate() {
                let lib = library.clone();
                let result = cx.background_executor().spawn(async move { lib.analyze_colors(&image) }).await;
                let ok = this
                    .update(cx, |this, cx| {
                        if this.generation != generation {
                            return false;
                        }
                        match result {
                            Ok(image) => this.upsert(image),
                            Err(err) => eprintln!("color analysis failed: {err:#}"),
                        }
                        this.progress = tr!("status.analyzing_colors", done = n + 1, total = total).into();
                        // 色で探しているときだけ、結果を描き直す
                        if this.query.color.is_some() {
                            cx.notify();
                        }
                        true
                    })
                    .unwrap_or(false);
                if !ok {
                    break;
                }
            }
            this.update(cx, |this, cx| {
                this.analyzing_colors = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// 写真のファイルに対する操作（画像を開く・フォルダで表示）。
    fn reveal(&mut self, key: &str, open: bool, cx: &mut Context<Self>) {
        let Some(path) = self.find_stored(key).and_then(|s| self.library.image_path(s)) else {
            return;
        };
        let result = if open { wallpaper::open_file(&path) } else { wallpaper::show_in_folder(&path) };
        if let Err(err) = result {
            self.set_status(format!("{err:#}"), true);
            cx.notify();
        }
    }

    /// 画面に並んでいる順（今日の写真 → 過去の写真）のキー。キーボードでの移動に使う。
    fn ordered_keys(&self) -> Vec<String> {
        if self.query.is_active() {
            return search::filter(&self.stored, &self.query)
                .into_iter()
                .take(self.results_limit)
                .map(|s| s.key.clone())
                .collect();
        }
        let today = self.today_photos();
        let mut keys: Vec<String> = today.iter().map(|t| t.key.clone()).collect();
        keys.extend(
            self.stored
                .iter()
                .filter(|s| !today.iter().any(|t| t.key == s.key))
                .take(self.past_limit)
                .map(|s| s.key.clone()),
        );
        keys
    }

    fn select_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let keys = self.ordered_keys();
        if keys.is_empty() {
            return;
        }
        let current = self.shown.as_ref().and_then(|k| keys.iter().position(|x| x == k));
        let next = match current {
            Some(i) => (i as isize + delta).clamp(0, keys.len() as isize - 1) as usize,
            None => 0,
        };
        let key = keys[next].clone();
        self.ensure_visible(&key);
        self.select(key, cx);
    }

    /// タイルが一覧の表示範囲から外れていれば、見える位置までスクロールする。
    fn ensure_visible(&self, key: &str) {
        let Some(tile) = self.tile_bounds.borrow().get(key).copied() else { return };
        let view = self.list_scroll.bounds();
        let margin = px(24.);
        let mut offset = self.list_scroll.offset();
        if tile.bottom() > view.bottom() {
            offset.y -= tile.bottom() - view.bottom() + margin;
        } else if tile.top() < view.top() {
            offset.y += view.top() - tile.top() + margin;
        } else {
            return;
        }
        let max = self.list_scroll.max_offset().y;
        offset.y = offset.y.clamp(-max, px(0.));
        self.list_scroll.set_offset(offset);
    }

    /// 写真に対する操作のメニュー（右クリックと、大きな写真の「…」ボタンで共通）。
    fn photo_menu(
        entity: WeakEntity<Self>,
        key: String,
        can_apply: bool,
        can_open: bool,
        current: bool,
        favorite: bool,
        menu: PopupMenu,
    ) -> PopupMenu {
        let item = |label: String, f: fn(&mut Self, &str, &mut Context<Self>)| {
            let entity = entity.clone();
            let key = key.clone();
            PopupMenuItem::new(label).on_click(move |_, _, cx| {
                entity.update(cx, |this, cx| f(this, &key, cx)).ok();
            })
        };
        let favorite_label = if favorite { tr!("favorite.remove") } else { tr!("favorite.add") };
        menu.item(
            item(tr!("action.set_wallpaper"), |this, key, cx| this.apply(key.to_string(), cx))
                .disabled(!can_apply || current),
        )
        .item(item(favorite_label, |this, key, cx| this.toggle_favorite(key, cx)).disabled(!can_apply))
        .separator()
        .item(item(tr!("action.open_image"), |this, key, cx| this.reveal(key, true, cx)).disabled(!can_open))
        .item(item(tr!("action.show_in_folder"), |this, key, cx| this.reveal(key, false, cx)).disabled(!can_open))
    }

    // ---- 描画 ----

    fn render_title_bar(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        TitleBar::new().child(
            h_flex()
                .flex_1()
                .pr_2()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.ink)
                        .child(tr!("app.name")),
                )
                .child(
                    div().w(px(360.)).child(
                        Input::new(&self.search_input)
                            .small()
                            .prefix(Icon::new(IconName::Search).small().text_color(p.slate))
                            .cleanable(true),
                    ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new("refresh")
                                .small()
                                .ghost()
                                .icon(IconName::RefreshCw)
                                .tooltip(tr!("action.refresh_tooltip"))
                                .loading(self.refreshing)
                                .disabled(self.refreshing)
                                .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                        )
                        .child(
                            Button::new("show-favorites")
                                .small()
                                .ghost()
                                .icon(if self.query.favorites { IconName::StarFill } else { IconName::Star })
                                .tooltip(tr!("favorite.show"))
                                .selected(self.query.favorites)
                                .on_click(cx.listener(|this, _, _, cx| this.toggle_favorites_filter(cx))),
                        )
                        .child(
                            Button::new("settings")
                                .small()
                                .ghost()
                                .label(tr!("settings.title"))
                                .tooltip(tr!("action.settings_tooltip"))
                                .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
                        ),
                ),
        )
    }

    /// 選んだ写真を全幅で表示する。この画面で唯一の大きな要素。
    /// `hero` は今表示している写真、`loading` は選んだ別の写真の画像を読み込み中かどうか。
    fn render_hero(&self, hero: Option<Hero>, loading: bool, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let (from, to, anim) = self.hero_height;
        let collapsed = self.hero_collapsed;
        let compact = to < 380.;
        let frame = div().relative().w_full().flex_none().overflow_hidden().bg(p.surface);
        // 高さは「開始 → 終了」をアニメーションで変える（スクロールで細く、写真を選ぶと広く）
        let with_height = move |el: Div| {
            el.with_animation(
                ("hero-height", anim),
                Animation::new(HERO_RESIZE).with_easing(ease_in_out),
                move |el, t| el.h(px(from + (to - from) * t)),
            )
            .into_any_element()
        };

        let Some(hero) = hero else {
            let failed = !self.refreshing;
            return with_height(frame.child(
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .text_color(p.slate)
                    .when(!failed, |el| {
                        el.child(Spinner::new().large()).child(tr!("hero.loading"))
                    })
                    .when(failed, |el| {
                        el.child(tr!("hero.failed"))
                    }),
            ));
        };

        // 前の写真を下に残し、新しい写真をゆっくり重ねる（読み込みが終わってから切り替えるので、画面が消えない）
        let back = self
            .hero_back
            .clone()
            .filter(|b| Some(b) != hero.image.as_ref())
            .map(|path| {
                div()
                    .absolute()
                    .inset_0()
                    .child(img(path).size_full().object_fit(ObjectFit::Cover))
            });
        let image = match &hero.image {
            Some(path) => div()
                .absolute()
                .inset_0()
                .child(img(path.clone()).size_full().object_fit(ObjectFit::Cover))
                .with_animation(
                    ("hero-fade", self.hero_swap),
                    Animation::new(HERO_FADE).with_easing(ease_in_out),
                    |el, t| el.opacity(t),
                )
                .into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(Spinner::new().large())
                .into_any_element(),
        };
        // 選んだ写真を読み込んでいる間の表示
        let loading_badge = loading.then(|| {
            h_flex()
                .absolute()
                .top_3()
                .right_4()
                .gap_2()
                .px_3()
                .py_1()
                .rounded_full()
                .bg(black().opacity(0.55))
                .text_xs()
                .text_color(white())
                .child(Spinner::new().xsmall().color(white()))
                .child(tr!("hero.loading_image"))
        });

        let key = hero.key.clone();
        let menu_button = {
            let entity = cx.entity().downgrade();
            let (key, can_apply, can_open, current) =
                (hero.key.clone(), hero.can_apply, hero.can_open, hero.current);
            let favorite = self.is_favorite(&key);
            // 写真の上に置くので、「壁紙に設定中」と同じ暗い下地と白い記号にする
            // （ButtonCustomVariant は背景を 20% ほどに薄めて使うため、ここでは直接指定する）
            Button::new("photo-menu")
                .icon(Icon::new(IconName::Ellipsis).text_color(white()))
                .tooltip(tr!("action.photo_menu"))
                .custom(
                    ButtonCustomVariant::new(cx)
                        .color(black())
                        .foreground(white())
                        .hover(black().opacity(0.75))
                        .active(black().opacity(0.85)),
                )
                .bg(black().opacity(0.55))
                .border_1()
                .border_color(white().opacity(0.28))
                .dropdown_menu(move |menu, _, _| {
                    Self::photo_menu(entity.clone(), key.clone(), can_apply, can_open, current, favorite, menu)
                })
        };
        let favorite_button = {
            let favorite = self.is_favorite(&hero.key);
            let key = hero.key.clone();
            Button::new("favorite")
                .icon(
                    Icon::new(if favorite { IconName::StarFill } else { IconName::Star })
                        .text_color(if favorite { rgb(0xF2C14E).into() } else { white() }),
                )
                .tooltip(format!(
                    "{}（Ctrl+D）",
                    if favorite { tr!("favorite.remove") } else { tr!("favorite.add") }
                ))
                .custom(
                    ButtonCustomVariant::new(cx)
                        .color(black())
                        .foreground(white())
                        .hover(black().opacity(0.75))
                        .active(black().opacity(0.85)),
                )
                .bg(black().opacity(0.55))
                .border_1()
                .border_color(white().opacity(0.28))
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_favorite(&key, cx)))
        };
        let button = if hero.current {
            // 押せるボタンではなく「状態」なので、無効化したボタン（半透明で読みにくい）にはしない。
            // 題名と同じ白い文字を、写真の明るさに左右されない暗い下地に載せる。
            h_flex()
                .flex_none()
                .gap_1p5()
                .px_3()
                .py_1p5()
                .rounded_md()
                .bg(black().opacity(0.55))
                .border_1()
                .border_color(white().opacity(0.28))
                .text_sm()
                .text_color(white())
                .child(Icon::new(IconName::Check).small().text_color(white()))
                .child(tr!("hero.current"))
                .into_any_element()
        } else {
            Button::new("apply")
                .primary()
                .label(tr!("action.set_wallpaper"))
                .loading(self.applying)
                .disabled(!hero.can_apply || self.applying)
                .on_click(cx.listener(move |this, _, _, cx| this.apply(key.clone(), cx)))
                .into_any_element()
        };

        with_height(frame.children(back).child(image).children(loading_badge).child(
            h_flex()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .px_8()
                .pt(px(if collapsed { 28. } else if compact { 56. } else { 120. }))
                .pb(px(if collapsed { 16. } else if compact { 18. } else { 28. }))
                .gap_6()
                .items_end()
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(black().opacity(0.), 0.),
                    linear_color_stop(black().opacity(0.72), 1.),
                ))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_2()
                        .text_color(white())
                        .child(
                            div()
                                .text_size(px(if collapsed { 22. } else if compact { 26. } else { 34. }))
                                .line_height(px(if collapsed { 30. } else if compact { 34. } else { 44. }))
                                .font_weight(FontWeight::LIGHT)
                                .truncate()
                                .child(hero.title),
                        )
                        .when(!collapsed, |el| el.child(
                            div()
                                .max_w(px(720.))
                                .text_sm()
                                .line_height(px(22.))
                                .text_color(white().opacity(0.88))
                                .child(hero.description),
                        )
                        .child(
                            div()
                                .pt_1()
                                .text_xs()
                                .text_color(white().opacity(0.72))
                                .child(format!(
                                    "{}　{}",
                                    bing::date_text(&hero.startdate, true),
                                    hero.markets
                                )),
                        )),
                )
                .child(
                    h_flex()
                        .flex_none()
                        .gap_2()
                        .when(hero.can_apply, |el| el.child(favorite_button))
                        .child(menu_button)
                        .child(button),
                ),
        ))
    }

    /// 枠のない写真タイル。選択中は湖の色の輪郭で示す。
    #[allow(clippy::too_many_arguments)]
    fn tile(
        &self,
        id: impl Into<ElementId>,
        width: f32,
        key: &str,
        title: String,
        caption: String,
        current: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.shown.as_deref() == Some(key);
        let thumb = self.library.thumb_path(key);
        let key_owned = key.to_string();
        let can_apply = self.find_stored(key).is_some();
        let can_open = self.find_stored(key).is_some_and(|s| self.library.image_path(s).is_some());
        let entity = cx.entity().downgrade();
        let menu_key = key.to_string();
        let favorite = self.is_favorite(key);
        let (bounds_map, bounds_key) = (self.tile_bounds.clone(), key.to_string());
        v_flex()
            .id(id.into())
            .relative()
            // 描画位置を記録しておき、キーボードで選んだときのスクロールに使う
            .child(
                canvas(
                    move |bounds, _, _| {
                        bounds_map.borrow_mut().insert(bounds_key, bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .w(px(width))
            .flex_none()
            .gap_1p5()
            .on_click(cx.listener(move |this, _, _, cx| this.select(key_owned.clone(), cx)))
            .context_menu(move |menu, _, _| {
                Self::photo_menu(entity.clone(), menu_key.clone(), can_apply, can_open, current, favorite, menu)
            })
            .child(
                div()
                    .w_full()
                    .h(px(width * 9. / 16.))
                    .p(px(2.))
                    .rounded(px(7.))
                    .border_2()
                    .border_color(if selected { p.lake } else { transparent_black() })
                    .child(
                        div()
                            .size_full()
                            .rounded(px(4.))
                            .overflow_hidden()
                            .relative()
                            .bg(p.line)
                            .when(thumb.is_file(), |el| {
                                el.child(img(thumb).size_full().object_fit(ObjectFit::Cover))
                            })
                            .when(favorite, |el| {
                                el.child(
                                    div()
                                        .absolute()
                                        .top_1p5()
                                        .right_1p5()
                                        .p_1()
                                        .rounded_full()
                                        .bg(black().opacity(0.55))
                                        .child(Icon::new(IconName::StarFill).xsmall().text_color(rgb(0xF2C14E))),
                                )
                            }),
                    ),
            )
            .child(
                v_flex()
                    .px_1()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .truncate()
                            .text_color(if selected { p.lake } else { p.ink })
                            .child(title),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .text_xs()
                            .text_color(p.slate)
                            .child(div().flex_1().min_w_0().truncate().child(caption))
                            .when(current, |el| {
                                el.child(div().flex_none().text_color(p.lake).child(tr!("hero.current")))
                            }),
                    ),
            )
    }

    fn section_heading(title: String, note: Option<String>, p: Palette) -> impl IntoElement {
        h_flex()
            .gap_3()
            .items_baseline()
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.ink)
                    .child(title),
            )
            .when_some(note, |el, note| el.child(div().text_xs().text_color(p.slate).child(note)))
    }

    fn render_today(&self, today: &[TodayPhoto], p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let failed: Vec<String> = self
            .latest
            .iter()
            .enumerate()
            .filter(|(_, l)| l.info.is_none() && matches!(l.state, LatestState::Failed))
            .map(|(i, _)| bing::market_name(MARKETS[i].0))
            .collect();
        let tiles: Vec<AnyElement> = today
            .iter()
            .map(|photo| {
                let current = self.find_stored(&photo.key).is_some_and(|s| self.is_current(s));
                let regions = photo.markets.iter().map(|&i| bing::market_name(MARKETS[i].0)).collect::<Vec<_>>();
                self.tile(
                    SharedString::from(format!("today-{}", photo.key)),
                    300.,
                    &photo.key,
                    photo.info.display_title(),
                    regions_caption(&regions),
                    current,
                    p,
                    cx,
                )
                .into_any_element()
            })
            .collect();

        v_flex()
            .gap_3()
            .child(Self::section_heading(
                tr!("today.title"),
                (today.len() > 1).then(|| tr!("today.differ").to_string()),
                p,
            ))
            .child(h_flex().flex_wrap().gap_5().children(tiles))
            .when(!failed.is_empty(), |el| {
                el.child(
                    div()
                        .text_xs()
                        .text_color(p.danger)
                        .child(tr!("today.failed", regions = failed.join(tr!("list.separator").as_ref()))),
                )
            })
    }

    fn render_past(&self, today: &[TodayPhoto], p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let today_keys: HashSet<&str> = today.iter().map(|t| t.key.as_str()).collect();
        let past: Vec<&StoredImage> =
            self.stored.iter().filter(|s| !today_keys.contains(s.key.as_str())).collect();
        let total = past.len();

        // 日付ごとにまとめる（stored は新しい順）
        let mut groups: Vec<(String, Vec<&StoredImage>)> = Vec::new();
        for s in past.iter().take(self.past_limit) {
            match groups.last_mut() {
                Some((date, items)) if *date == s.startdate => items.push(s),
                _ => groups.push((s.startdate.clone(), vec![s])),
            }
        }

        let group_elements: Vec<AnyElement> = groups
            .into_iter()
            .map(|(date, items)| {
                let tiles: Vec<AnyElement> = items
                    .into_iter()
                    .map(|s| {
                        let regions =
                            s.markets.iter().map(|m| bing::market_name(m)).collect::<Vec<_>>();
                        self.tile(
                            SharedString::from(format!("past-{}", s.key)),
                            220.,
                            &s.key,
                            s.display_title(),
                            regions_caption(&regions),
                            self.is_current(s),
                            p,
                            cx,
                        )
                        .into_any_element()
                    })
                    .collect();
                // 日付の線は、その日の写真の幅だけ引いて「同じ日」のまとまりを示す
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .pt_1p5()
                            .border_t_1()
                            .border_color(p.line)
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.slate)
                            .child(bing::date_text(&date, false)),
                    )
                    .child(h_flex().gap_4().children(tiles))
                    .into_any_element()
            })
            .collect();

        let remaining = total.saturating_sub(self.past_limit);
        v_flex()
            .gap_4()
            .child(Self::section_heading(
                tr!("past.title"),
                (total > 0).then(|| tr!("past.count", count = total)),
                p,
            ))
            .when(total == 0, |el| {
                el.child(
                    div()
                        .text_sm()
                        .text_color(p.slate)
                        .child(tr!("past.empty")),
                )
            })
            .child(h_flex().flex_wrap().items_start().gap_x_8().gap_y_6().children(group_elements))
            // 保存済みの写真は、下端に近づくと自動で続きを表示する（load_more_if_near_end）。
            // 手元の写真をすべて表示したら、アーカイブからさらに古い写真を読み込めるようにする
            .when(remaining == 0, |el| {
                el.child(
                    v_flex()
                        .gap_2()
                        .items_start()
                        .child(
                            Button::new("load-older")
                                .small()
                                .label(tr!("past.load_older"))
                                .loading(self.loading_older)
                                .disabled(self.loading_older || self.older_exhausted)
                                .on_click(cx.listener(|this, _, _, cx| this.load_older(cx))),
                        )
                        .child(div().text_xs().line_height(px(18.)).text_color(p.slate).child(
                            if self.older_exhausted {
                                tr!("past.no_older").to_string()
                            } else {
                                tr!("past.load_older_note", source = archive::SOURCE_NAME)
                            },
                        )),
                )
            })
    }

    /// 検索中に一覧の上に出す帯（色見本と、検索を閉じるボタン）。
    fn render_search_bar(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let swatches = ColorBucket::ALL.iter().enumerate().map(|(i, &color)| {
            let selected = self.query.color == Some(color);
            h_flex()
                .id(("color", i))
                .gap_1p5()
                .px_2()
                .py_1()
                .rounded_full()
                .border_1()
                .border_color(if selected { p.lake } else { p.line })
                .when(selected, |el| el.bg(p.lake.opacity(0.12)))
                .text_xs()
                .text_color(if selected { p.lake } else { p.ink })
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_color(color, cx)))
                .child(
                    div()
                        .size(px(12.))
                        .rounded_full()
                        .bg(color.swatch())
                        .border_1()
                        .border_color(p.slate.opacity(0.4)),
                )
                .child(color.label())
        });
        h_flex()
            .flex_none()
            .flex_wrap()
            .gap_2()
            .px_8()
            .py_2()
            .border_b_1()
            .border_color(p.line)
            .bg(p.surface)
            .child({
                let on = self.query.favorites;
                h_flex()
                    .id("favorites-chip")
                    .gap_1p5()
                    .px_2()
                    .py_1()
                    .rounded_full()
                    .border_1()
                    .border_color(if on { p.lake } else { p.line })
                    .when(on, |el| el.bg(p.lake.opacity(0.12)))
                    .text_xs()
                    .text_color(if on { p.lake } else { p.ink })
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_favorites_filter(cx)))
                    .child(Icon::new(if on { IconName::StarFill } else { IconName::Star }).xsmall())
                    .child(tr!("favorite.filter"))
            })
            .child(div().w(px(1.)).h(px(18.)).bg(p.line))
            .child(div().text_xs().text_color(p.slate).child(tr!("search.by_color")))
            .children(swatches)
            .child(div().flex_1())
            .child(
                Button::new("close-search")
                    .xsmall()
                    .ghost()
                    .icon(IconName::Close)
                    .label(tr!("search.close"))
                    .on_click(cx.listener(|this, _, window, cx| this.close_search(window, cx))),
            )
    }

    /// 検索結果。言葉だけなら日付ごと、色を指定したらその色が多い順に並べる。
    fn render_results(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let hits = search::filter(&self.stored, &self.query);
        let total = hits.len();
        let tile = |s: &StoredImage, cx: &mut Context<Self>| {
            let regions = s.markets.iter().map(|m| bing::market_name(m)).collect::<Vec<_>>();
            let caption = if self.query.color.is_some() {
                // 色で並べたときは日付の見出しが無いので、日付を添える
                bing::date_text(&s.startdate, true)
            } else {
                regions_caption(&regions)
            };
            self.tile(
                SharedString::from(format!("hit-{}", s.key)),
                220.,
                &s.key,
                s.display_title(),
                caption,
                self.is_current(s),
                p,
                cx,
            )
            .into_any_element()
        };
        let shown: Vec<&StoredImage> = hits.into_iter().take(self.results_limit).collect();
        let body: AnyElement = if self.query.color.is_some() {
            let tiles: Vec<AnyElement> = shown.iter().map(|s| tile(s, cx)).collect();
            h_flex().flex_wrap().gap_4().children(tiles).into_any_element()
        } else {
            let mut groups: Vec<(String, Vec<&StoredImage>)> = Vec::new();
            for s in &shown {
                match groups.last_mut() {
                    Some((date, items)) if *date == s.startdate => items.push(s),
                    _ => groups.push((s.startdate.clone(), vec![s])),
                }
            }
            let groups: Vec<AnyElement> = groups
                .into_iter()
                .map(|(date, items)| {
                    let tiles: Vec<AnyElement> = items.into_iter().map(|s| tile(s, cx)).collect();
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .pt_1p5()
                                .border_t_1()
                                .border_color(p.line)
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.slate)
                                .child(bing::date_text(&date, false)),
                        )
                        .child(h_flex().gap_4().children(tiles))
                        .into_any_element()
                })
                .collect();
            h_flex().flex_wrap().items_start().gap_x_8().gap_y_6().children(groups).into_any_element()
        };
        v_flex()
            .gap_4()
            .child({
                // お気に入りだけを表示しているときは「お気に入り n 枚」にする
                let only_favorites =
                    self.query.favorites && self.query.color.is_none() && self.query.text.trim().is_empty();
                if only_favorites {
                    Self::section_heading(tr!("favorite.filter"), Some(tr!("past.count", count = total)), p)
                } else {
                    Self::section_heading(tr!("search.results", count = total), Some(tr!("search.hint")), p)
                }
            })
            .when(total == 0, |el| {
                let only_favorites =
                    self.query.favorites && self.query.color.is_none() && self.query.text.trim().is_empty();
                el.child(div().text_sm().text_color(p.slate).child(if only_favorites {
                    tr!("favorite.empty")
                } else {
                    tr!("search.no_results")
                }))
            })
            // 続きは下端に近づくと自動で表示する（load_more_if_near_end）
            .child(body)
    }

    fn render_footer(&self, p: Palette) -> impl IntoElement {
        let busy = self.refreshing
            || self.loading_older
            || self.indexing
            || (self.analyzing_colors && self.query.color.is_some());
        let (text, color) = if busy {
            (self.progress.clone(), p.slate)
        } else if self.is_error {
            (self.status.clone(), p.danger)
        } else {
            (self.status.clone(), p.slate)
        };
        h_flex()
            .flex_none()
            .gap_2()
            .px_4()
            .py_1()
            .bg(p.surface)
            .border_t_1()
            .border_color(p.line)
            .text_xs()
            .when(busy, |el| el.child(Spinner::new().xsmall().color(p.slate)))
            .child(div().flex_1().min_w_0().truncate().text_color(color).child(text))
            .child(
                div()
                    .flex_none()
                    .text_color(p.slate)
                    .child(tr!("footer.keys")),
            )
    }
}

impl Render for Dashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::current(window);
        let today = self.today_photos();
        let target = self.hero(&today);
        // 選択の印（タイルの輪郭）はすぐ動かす
        self.shown = target.as_ref().map(|h| h.key.clone());
        // 大きな写真は、選んだ写真の画像の読み込みが終わってから切り替える
        let target_id = target.as_ref().map(|h| (h.key.clone(), h.image.clone()));
        let target_ready = target
            .as_ref()
            .and_then(|h| h.image.as_ref())
            .is_none_or(|path| {
                window
                    .use_asset::<ImgResourceLoader>(&Resource::Path(Arc::from(path.as_path())), cx)
                    .is_some()
            });
        if target_id != self.hero_front && target_ready {
            self.hero_back = self.hero_front.as_ref().and_then(|(_, path)| path.clone());
            self.hero_front = target_id.clone();
            self.hero_swap += 1;
        }
        let loading = target_id != self.hero_front;
        let hero = self.hero_front.clone().and_then(|(key, path)| {
            self.hero_for(key, &today).map(|mut h| {
                h.image = path;
                h
            })
        });
        // 無限スクロール: 描画後の大きさで、下端に近ければ続きを表示する
        if self.hidden_count() > 0 {
            let entity = cx.entity().downgrade();
            window.on_next_frame(move |_, cx| {
                entity.update(cx, |this, cx| this.load_more_if_near_end(cx)).ok();
            });
        }
        // 高さ: スクロール中は細い帯、それ以外は広げた高さ。変わったらアニメーションをやり直す
        let target_height =
            if self.hero_collapsed { HERO_COLLAPSED_HEIGHT } else { Self::hero_full_height(window) };
        if self.hero_height.1 != target_height {
            let from = if self.hero_height.2 == 0 { target_height } else { self.hero_height.1 };
            self.hero_height = (from, target_height, self.hero_height.2 + 1);
        }
        if let Some(key) = self.shown.clone() {
            self.request_preview(&key, cx);
        }
        v_flex()
            .size_full()
            .key_context("Dashboard")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.select_step(-1, cx)))
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.select_step(1, cx)))
            .on_action(cx.listener(|this, _: &ApplySelected, _, cx| {
                if let Some(key) = this.shown.clone() {
                    this.apply(key, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Refresh, _, cx| this.refresh(cx)))
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| this.open_settings(window, cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| this.focus_search(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleFavorite, _, cx| {
                if let Some(key) = this.shown.clone() {
                    this.toggle_favorite(&key, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &CloseSearch, window, cx| this.close_search(window, cx)))
            .font_family(theme::FONT)
            .bg(p.fog)
            .text_color(p.ink)
            .child(self.render_title_bar(p, cx))
            // 大きな写真は上部に固定し、一覧だけをスクロールさせる。
            // 下の方の写真を選んでも、選んだ写真が常に見える。
            .child(self.render_hero(hero, loading, p, cx))
            .when(self.search_open || self.query.is_active(), |el| el.child(self.render_search_bar(p, cx)))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("list")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.list_scroll)
                            // スクロールが反映されたあとの位置で、大きな写真を細くするか決める
                            .on_scroll(cx.listener(|_, _: &ScrollEvent, window, cx| {
                                let entity = cx.entity().downgrade();
                                window.on_next_frame(move |_, cx| {
                                    entity.update(cx, |this, cx| this.on_list_scrolled(cx)).ok();
                                });
                            }))
                            .child(
                                v_flex()
                                    .px_8()
                                    .py_7()
                                    .gap_10()
                                    .map(|el| {
                                        if self.query.is_active() {
                                            el.child(self.render_results(p, cx))
                                        } else {
                                            el.child(self.render_today(&today, p, cx))
                                                .child(self.render_past(&today, p, cx))
                                        }
                                    }),
                            ),
                    )
                    .vertical_scrollbar(&self.list_scroll)
                    // スクロールした写真が大きな写真の下端に接して見えないよう、上端を地の色でなじませる
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .h(px(20.))
                            .bg(linear_gradient(
                                180.,
                                linear_color_stop(p.fog, 0.),
                                linear_color_stop(p.fog.opacity(0.), 1.),
                            )),
                    ),
            )
            .child(self.render_footer(p))
    }
}

/// `--apply [--mkt ja-JP]`: GUI を開かずに今日の写真を壁紙にする（cron / systemd timer 用）。
fn run_cli(args: &[String]) -> anyhow::Result<()> {
    let mkt = args
        .iter()
        .position(|a| a == "--mkt")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
        .unwrap_or(MARKETS[0].0);
    let config = Config::load();
    let library = Library::new(config.save_dir());
    let image = bing::fetch_archive(mkt, 0, 1)?
        .into_iter()
        .next()
        .expect("fetch_archive returns at least one image");
    let stored = library.store(&PhotoSource::from_bing(&image, mkt), &[mkt], true)?;
    let path = library
        .image_path(&stored)
        .ok_or_else(|| anyhow::anyhow!(tr!("error.saved_image_missing")))?;
    wallpaper::set_wallpaper(&path, config.fit)?;
    println!("{} {}", bing::date_text(&image.startdate, true), image.copyright);
    println!("{}: {}", tr!("cli.wallpaper_set"), path.display());
    Ok(())
}

fn main() {
    let config = Config::load();
    // BWM_LANG=de などで表示言語を固定できる（撮影・確認用）
    let language = std::env::var("BWM_LANG")
        .ok()
        .and_then(|code| Language::ALL.into_iter().find(|l| *l != Language::Auto && l.code() == code))
        .unwrap_or(config.language);
    i18n::set(language);
    theme::set_pref(config.theme);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "{}",
            tr!("cli.usage")
        );
        return;
    }
    if args.iter().any(|a| a == "--apply") {
        if let Err(err) = run_cli(&args) {
            eprintln!("{}: {err:#}", tr!("cli.error"));
            std::process::exit(1);
        }
        return;
    }

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.bind_keys(key_bindings());
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1280.), px(900.)), cx)),
                window_min_size: Some(size(px(820.), px(560.))),
                app_id: Some("bing-wallpaper-manager".into()),
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                theme::apply(window, cx);
                cx.new(|cx| Dashboard::new(window, cx))
            })
            .expect("failed to open window");
            cx.activate(true);
        });
}
