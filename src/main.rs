#[macro_use]
mod i18n;

mod info;

rust_i18n::i18n!("locales", fallback = "en");

mod archive;
mod bing;
mod colors;
mod config;
mod library;
mod schedule;
mod search;
mod settings;
mod single;
mod theme;
mod wallpaper;

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
    Disableable as _, Icon, IconName, InteractiveElementExt as _, Sizable as _,
    WindowExt as _, h_flex, v_flex, window_corner_radii,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use bing::BingImage;
use config::{AutoUpdate, Config, FitMode, Language, OlderMode, ThemePref};
use archive::ArchivePhoto;
use library::{Library, PhotoSource, StoredImage};
use colors::ColorBucket;
use search::Query;
use settings::SettingsView;
use theme::Palette;

gpui_kit::actions!(
    bing_wallpaper,
    [
        SelectPrevious,
        SelectNext,
        ApplySelected,
        Refresh,
        OpenSettings,
        FocusSearch,
        CloseSearch,
        ToggleFavorite,
        ToggleFullscreen,
        ShowInfo,
        DeleteSelected,
        ScrollHome,
        ScrollEnd,
        ScrollPageUp,
        ScrollPageDown
    ]
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
        KeyBinding::new("f11", ToggleFullscreen, ctx),
        KeyBinding::new("ctrl-i", ShowInfo, ctx),
        KeyBinding::new("delete", DeleteSelected, ctx),
        KeyBinding::new("home", ScrollHome, ctx),
        KeyBinding::new("end", ScrollEnd, ctx),
        KeyBinding::new("pageup", ScrollPageUp, ctx),
        KeyBinding::new("pagedown", ScrollPageDown, ctx),
    ]
}

/// 一覧のタイルの幅と間隔（行の組み立てに使う）。
const TILE_WIDTH: f32 = 220.;
const TILE_GAP: f32 = 16.;
const GROUP_GAP: f32 = 32.;
/// 一覧の左右の余白（px_8）とスクロールバーの分。
const LIST_SIDE_PADDING: f32 = 64. + 12.;

/// 一覧（仮想リスト）の 1 項目。gpui の list が、見えている項目だけを描く。
/// 写真が数百枚あっても、スクロールのたびに描くのは画面に入る数行だけになる。
#[derive(Clone, PartialEq, Hash)]
enum ListItem {
    /// 今日の写真
    Today,
    /// 「過去の写真」の見出し（枚数）
    PastHeading(usize),
    /// 検索結果の見出し（件数）
    ResultsHeading(usize),
    /// 日付ごとのまとまりを横に並べた 1 行（日付, 写真のキー）
    Groups(Vec<(String, Vec<String>)>),
    /// 色で探したときの 1 行（写真のキー。その色が多い順）
    Tiles(Vec<String>),
    /// 「さらに古い写真を読み込む」
    LoadOlder,
}
/// 起動したままのとき、新しい写真を確かめに行く間隔。
const AUTO_REFRESH: Duration = Duration::from_secs(60 * 60);
/// 「さらに古い写真を読み込む」で一度に読み込む日数。
const OLDER_DAYS: usize = 30;
/// 「古い写真を読み込む」1 回で、途中の抜けを埋める写真の上限。
const GAP_PHOTOS: usize = 90;
/// フッターの高さ（大きな写真の高さの計算用）。タイトルバーは大きな写真に重ねるので含めない。
const CHROME_HEIGHT: Pixels = px(30.);
/// 大きな写真に重ねるタイトルバーの高さと、表示・非表示を切り替える時間。
const WINDOW_BAR_HEIGHT: Pixels = px(48.);
const WINDOW_BAR_FADE: Duration = Duration::from_millis(180);
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
    /// 削除できるか（今日の写真と今の壁紙は削除できない）
    can_delete: bool,
}

/// 写真のメニューで、どの操作ができるか。
#[derive(Clone, Copy)]
struct PhotoActions {
    can_apply: bool,
    can_open: bool,
    can_delete: bool,
    current: bool,
    favorite: bool,
}

struct Dashboard {
    config: Config,
    library: Library,
    /// ダッシュボードに出す地域（設定で選んだもの）。latest と同じ並び
    markets: Vec<&'static str>,
    /// 削除した写真のキー（更新や古い写真の読み込みで取得し直さない）
    deleted: HashSet<String>,
    /// 最後に更新を始めた時刻（起動したままのとき、1 時間ごとに確かめ直す）
    last_refresh: std::time::Instant,
    /// 自動更新の設定に失敗したときのエラー（設定画面に出す）
    auto_error: Option<SharedString>,
    /// 保存先の使用量（設定画面を開いたときに数える）
    usage: Option<u64>,
    /// ウィンドウの大きさの保存を間引くための番号
    bounds_save: usize,
    stored: Vec<StoredImage>,
    /// stored の索引（キー → 位置）。写真が数百枚あっても描画中の検索を速くするため
    stored_index: HashMap<String, usize>,
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
    /// アーカイブの一覧（最初に「さらに古い写真を読み込む」を押したときに取得）
    archive: Option<Rc<Vec<ArchivePhoto>>>,
    loading_older: bool,
    /// アーカイブにもこれより古い写真が無い
    older_exhausted: bool,
    /// 前回の読み込みが失敗した。自動では繰り返さず、ボタンを出してやり直せるようにする。
    older_failed: bool,
    /// 中サイズの画像を取得中（または取得を試みた）写真
    previews_pending: HashSet<String>,
    focus_handle: FocusHandle,
    /// 一覧（仮想リスト）の状態と、今の項目
    list_state: ListState,
    list_items: Vec<ListItem>,
    /// 項目を作り直す必要があるか判断するための、前回の項目の要約
    list_signature: u64,
    /// 「写真全体を表示」などで余った部分に GNOME が塗る色（設定画面のプレビュー用）
    margin_color: Hsla,
    /// 検索欄と検索条件
    search_input: Entity<InputState>,
    query: Query,
    /// 色見本の帯を出しているか（検索欄を使い始めたら出し、閉じるまで出したまま）
    search_open: bool,
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
    /// 高さのアニメーションを始めた時刻（アニメーション中は細い帯の切り替えをしない）
    hero_height_changed: Option<std::time::Instant>,
    /// 大きな写真にマウスが乗っているか（乗っている間だけタイトルバーを出す）
    hero_hovered: bool,
    /// タイトルバーのフェードのアニメーション（表示するか, 識別番号）
    window_bar: (bool, usize),
    /// 大きな写真を押したまま動かしたら、ウィンドウの移動を始める
    window_bar_pressed: bool,
    /// 先に読み込んでおく前後の写真の画像（どの写真・どの並びのときに求めたか, 画像）。
    /// ←→ で隣に移ったとき、画像のデコードを待たずに切り替えられる
    preload: (Option<String>, u64, Vec<PathBuf>),
    _appearance: Subscription,
    _search_events: Subscription,
    _bounds: Subscription,
}

/// `all` は、表示している地域のすべてで配信されたか。
fn markets_label(markets: impl IntoIterator<Item = String>, all: bool) -> String {
    let names: Vec<String> = markets.into_iter().collect();
    if all && names.len() > 1 {
        tr!("hero.shown_all", count = names.len())
    } else {
        tr!("hero.shown_in", regions = names.join(tr!("list.separator").as_ref()))
    }
}

/// タイル用の短い地域表記。
fn regions_caption(names: &[String], all: bool) -> String {
    match names.len() {
        n if all && n > 1 => tr!("tile.all_regions").to_string(),
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
        // ウィンドウの大きさを覚えておき、次の起動で同じ大きさにする
        let bounds = cx.observe_window_bounds(window, |this, window, cx| this.remember_window_size(window, cx));
        // 起動したままでも、日付が変わったあとの写真が出るように、定期的に確かめ直す
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(10 * 60)).await;
                let alive = this.update(cx, |this, cx| {
                    if this.last_refresh.elapsed() >= AUTO_REFRESH {
                        this.refresh(cx);
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        let markets = config.markets();
        let mut this = Self {
            focus_handle,
            list_state: {
                let state = ListState::new(0, ListAlignment::Top, px(800.));
                state.set_scroll_handler(
                    // リストの状態を借りている最中に呼ばれるので、位置の確認はそのあとで行う
                    cx.listener(|_, _: &ListScrollEvent, window, cx| {
                        cx.defer_in(window, |this, _, cx| this.on_list_scrolled(cx));
                    }),
                );
                state
            },
            list_items: Vec::new(),
            list_signature: 0,
            margin_color: wallpaper::primary_color().map(|c| rgb(c).into()).unwrap_or(black()),
            stored: Vec::new(),
            stored_index: HashMap::new(),
            deleted: library.deleted_keys(),
            last_refresh: std::time::Instant::now(),
            auto_error: None,
            usage: None,
            bounds_save: 0,
            _bounds: bounds,
            config,
            library,
            latest: markets
                .iter()
                .map(|_| MarketLatest { info: None, state: LatestState::Loading })
                .collect(),
            markets,
            selected: None,
            shown: None,
            current_uri: wallpaper::current_uri(),
            generation: 0,
            refreshing: false,
            applying: false,
            progress: "".into(),
            status: "".into(),
            is_error: false,
            archive: None,
            loading_older: false,
            older_exhausted: false,
            older_failed: false,
            previews_pending: HashSet::new(),
            search_input,
            query: Query::default(),
            search_open: false,
            descriptions_ready: false,
            indexing: false,
            analyzing_colors: false,
            hero_front: None,
            hero_back: None,
            hero_swap: 0,
            hero_collapsed: false,
            hero_height: (0., 0., 0),
            hero_height_changed: None,
            hero_hovered: false,
            window_bar: (false, 0),
            window_bar_pressed: false,
            preload: (None, 0, Vec::new()),
            _appearance: appearance,
            _search_events: search_events,
        };
        this.set_stored(this.library.load_all());
        this.refresh(cx);
        this.analyze_missing_colors(cx);
        this
    }

    fn set_status(&mut self, msg: impl Into<SharedString>, is_error: bool) {
        self.status = msg.into();
        self.is_error = is_error;
    }

    fn find_stored(&self, key: &str) -> Option<&StoredImage> {
        self.stored_index.get(key).and_then(|&i| self.stored.get(i))
    }

    fn set_stored(&mut self, stored: Vec<StoredImage>) {
        self.stored = stored;
        self.rebuild_index();
    }

    fn rebuild_index(&mut self) {
        self.stored_index = self.stored.iter().enumerate().map(|(i, s)| (s.key.clone(), i)).collect();
    }

    fn upsert(&mut self, image: StoredImage) {
        let moved = match self.stored_index.get(&image.key) {
            // ほかの地域での配信で日付が新しくなったら、並べ直す
            Some(&i) => {
                let moved = self.stored[i].startdate != image.startdate;
                self.stored[i] = image;
                moved
            }
            None => {
                self.stored.push(image);
                true
            }
        };
        if moved {
            self.stored.sort_by(|a, b| b.startdate.cmp(&a.startdate).then(a.key.cmp(&b.key)));
            self.rebuild_index();
        }
    }

    /// 写真が保存先に保管済みか（サムネイルは保存処理でメタデータより先に書くので、保管済みならある）。
    fn has_thumb(&self, key: &str) -> bool {
        self.stored_index.contains_key(key) || self.library.thumb_path(key).is_file()
    }

    fn is_current(&self, image: &StoredImage) -> bool {
        // 描画中に何度も呼ぶので、ファイルの有無は確かめずにパスだけ比べる
        match (&self.current_uri, self.library.image_path_unchecked(image)) {
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
                .find_map(|code| photo.markets.iter().find(|&&i| self.markets[i] == *code));
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
        let can_open = stored.is_some_and(|s| s.image_file.is_some());
        let current = stored.is_some_and(|s| self.is_current(s));
        let can_delete = self.can_delete(&key, today);

        if let Some(photo) = today.iter().find(|p| p.key == key) {
            return Some(Hero {
                title: photo.info.display_title(),
                description: photo.info.copyright.clone(),
                startdate: photo.info.startdate.clone(),
                markets: markets_label(
                    photo.markets.iter().map(|&i| bing::market_name(self.markets[i])),
                    photo.markets.len() == self.markets.len(),
                ),
                key,
                image,
                can_apply,
                can_open,
                current,
                can_delete,
            });
        }
        let s = stored?;
        Some(Hero {
            title: s.display_title(),
            description: s.display_copyright().to_string(),
            startdate: s.startdate.clone(),
            markets: markets_label(s.markets.iter().map(|m| bing::market_name(m)), self.in_all_markets(s)),
            key,
            image,
            can_apply,
            can_open,
            current,
            can_delete,
        })
    }

    /// 表示している地域のすべてで配信された写真か。
    fn in_all_markets(&self, image: &StoredImage) -> bool {
        self.markets.iter().all(|m| image.markets.iter().any(|x| x == m))
    }

    /// 写真を削除できるか。今日の写真（更新のたびに戻ってくる）と今の壁紙は削除できない。
    fn can_delete(&self, key: &str, today: &[TodayPhoto]) -> bool {
        self.find_stored(key).is_some_and(|s| !self.is_current(s)) && !today.iter().any(|p| p.key == key)
    }

    fn photo_actions(&self, key: &str, today: &[TodayPhoto]) -> PhotoActions {
        let stored = self.find_stored(key);
        PhotoActions {
            can_apply: stored.is_some(),
            can_open: stored.is_some_and(|s| s.image_file.is_some()),
            can_delete: self.can_delete(key, today),
            current: stored.is_some_and(|s| self.is_current(s)),
            favorite: stored.is_some_and(|s| s.favorite),
        }
    }

    /// 全地域の画像一覧を取得し、最新のものから順に保存先へ保管する。
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        self.last_refresh = std::time::Instant::now();
        self.progress = tr!("status.checking").into();
        for latest in &mut self.latest {
            latest.state = LatestState::Loading;
        }
        cx.notify();

        let generation = self.generation;
        let library = self.library.clone();
        let markets = self.markets.clone();
        let deleted = self.deleted.clone();
        cx.spawn(async move |this, cx| {
            let tasks: Vec<_> = markets
                .iter()
                .map(|&code| cx.background_executor().spawn(async move { bing::fetch_recent(code) }))
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
            // 削除した写真は取得し直さない（今日の写真としてまた配信されたときは保管する）
            queue.retain(|(_, image, is_latest)| *is_latest || !deleted.contains(&image.key()));

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
                let market = markets[i];
                let restore = deleted.contains(&image.key());
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        if restore {
                            lib.forget_deleted(&image.key())?;
                        }
                        lib.store(&PhotoSource::from_bing(&image, market), &[market], true)
                    })
                    .await;
                let ok = this
                    .update(cx, |this, cx| {
                        if this.generation != generation {
                            return false;
                        }
                        match result {
                            Ok(stored) => {
                                this.deleted.remove(&stored.key);
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

            // 最近の写真の解説文を補う（「写真の情報」と検索で使う）。取得できなくても続ける
            this.update(cx, |this, cx| {
                this.progress = tr!("status.descriptions").into();
                cx.notify();
            })
            .ok();
            let described = cx
                .background_executor()
                .spawn(async move {
                    let mut updated = Vec::new();
                    for market in markets {
                        match bing::fetch_descriptions(market) {
                            Ok(list) => updated.extend(
                                list.iter().filter_map(|(key, text)| library.add_description(key, market, text)),
                            ),
                            Err(err) => eprintln!("descriptions: {err:#}"),
                        }
                    }
                    updated
                })
                .await;

            this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                for image in described {
                    if this.stored_index.contains_key(&image.key) {
                        this.upsert(image);
                    }
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
            this.update(cx, |this, cx| {
                // 先読みする画像を、取得した中サイズの画像に差し替える
                this.preload.0 = None;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// アーカイブから、いちばん古い写真より前の約 1 か月分を読み込む。途中の抜けも少しずつ埋める。
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
        // 保管済みの写真と、削除した写真は読み込まない
        let stored_keys: HashSet<String> =
            self.stored.iter().map(|s| s.key.clone()).chain(self.deleted.iter().cloned()).collect();
        let markets = self.markets.clone();
        let oldest = self.stored.iter().map(|s| s.startdate.clone()).min();
        cx.spawn(async move |this, cx| {
            let archive = match cached {
                Some(archive) => archive,
                None => {
                    let (photos, errors) =
                        cx.background_executor().spawn(async move { archive::fetch_all(&markets) }).await;
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
                            this.older_failed = true;
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

            // いちばん古い写真より前の OLDER_DAYS 日分と、途中の抜け（地域を足したときなど）を読み込む
            let batch = archive::older_batch(&archive, &stored_keys, oldest.as_deref(), OLDER_DAYS, GAP_PHOTOS);

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
            // 抜けを埋めた分も混ざるので、範囲は日付の最小と最大で示す
            let first = batch.iter().map(|p| p.source.startdate.as_str()).min().unwrap_or_default();
            let last = batch.iter().map(|p| p.source.startdate.as_str()).max().unwrap_or_default();
            let range = format!(
                "{}{}{}",
                bing::date_text(first, true),
                tr!("list.range_separator"),
                bing::date_text(last, true)
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
                this.older_failed = loaded == 0;
                this.analyze_missing_colors(cx);
                // 途中の抜けだけが埋まって最後が見えたままなら、続けて読み込む
                this.auto_load_older_soon(cx);
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

    /// 古い写真の読み込み方（ボタン／最後までスクロールしたら自動）を変える。
    fn set_older_mode(&mut self, mode: OlderMode, cx: &mut Context<Self>) {
        if self.config.older == mode {
            return;
        }
        self.config.older = mode;
        self.save_config(cx);
        self.auto_load_older_soon(cx);
        cx.notify();
    }

    /// 自動で読み込む設定のとき、一覧の最後（読み込みの案内の行）が見えていたら古い写真を読み込む。
    fn auto_load_older(&mut self, cx: &mut Context<Self>) {
        if self.config.older != OlderMode::Auto || self.loading_older || self.older_exhausted || self.older_failed {
            return;
        }
        // 検索結果の一覧には案内の行が無い
        if !matches!(self.list_items.last(), Some(ListItem::LoadOlder)) {
            return;
        }
        let last = self.list_items.len() - 1;
        if self.list_state.item_is_below_viewport(last) == Some(false) {
            self.load_older(cx);
        }
    }

    /// キー操作などでスクロールした直後は一覧の配置がまだ古いので、描き直しを待ってから確かめる。
    fn auto_load_older_soon(&mut self, cx: &mut Context<Self>) {
        if self.config.older != OlderMode::Auto {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(120)).await;
            this.update(cx, |this, cx| this.auto_load_older(cx)).ok();
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
        self.set_stored(self.library.load_all());
        self.deleted = self.library.deleted_keys();
        self.usage = None;
        self.measure_usage(cx);
        self.restart_loading();
        self.set_status(
            tr!("status.storage_changed", path = self.library.dir().display()),
            false,
        );
        self.refresh(cx);
    }

    /// 進行中の取得をやめて、最初からやり直せるようにする（保存先や地域を変えたとき）。
    fn restart_loading(&mut self) {
        self.older_exhausted = false;
        self.older_failed = false;
        self.loading_older = false;
        self.generation += 1;
        self.refreshing = false;
    }

    /// 表示する地域を変える。少なくとも 1 つは残す。
    fn set_market_enabled(&mut self, code: &'static str, enabled: bool, cx: &mut Context<Self>) {
        let mut markets = self.markets.clone();
        markets.retain(|m| *m != code);
        if enabled {
            markets.push(code);
        }
        if markets.is_empty() {
            return;
        }
        self.config.markets = Some(markets.iter().map(|m| m.to_string()).collect());
        self.save_config(cx);
        // MARKETS の順に並べ直したものを使う
        self.markets = self.config.markets();
        self.latest = self.markets.iter().map(|_| MarketLatest { info: None, state: LatestState::Loading }).collect();
        // アーカイブの一覧と解説文は地域ごとなので、取り直す
        self.archive = None;
        self.descriptions_ready = false;
        self.restart_loading();
        self.refresh(cx);
        // 自動更新の地域が外れたら、タイマーも作り直す
        if self.config.auto.enabled {
            self.set_auto(self.config.auto.clone(), cx);
        }
    }

    /// 壁紙の自動更新の設定を変え、systemd のタイマーを作り直す。
    fn set_auto(&mut self, mut auto: AutoUpdate, cx: &mut Context<Self>) {
        // 「今日の写真」の地域は、表示している地域から選ぶ
        if auto.market.as_deref().is_some_and(|m| !self.markets.iter().any(|x| *x == m)) {
            auto.market = None;
        }
        let previous = std::mem::replace(&mut self.config.auto, auto.clone());
        self.auto_error = None;
        self.save_config(cx);
        cx.notify();
        let market = self.config.auto_market();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { schedule::apply(&auto, market) }).await;
            this.update(cx, |this, cx| {
                if let Err(err) = result {
                    // タイマーを作れなかったら、設定を元に戻してエラーを設定画面に出す
                    this.config.auto = previous;
                    this.save_config(cx);
                    this.auto_error = Some(format!("{err:#}").into());
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// 保存先の使用量を裏で数える（設定画面に出す）。
    fn measure_usage(&mut self, cx: &mut Context<Self>) {
        let library = self.library.clone();
        cx.spawn(async move |this, cx| {
            let usage = cx.background_executor().spawn(async move { library.usage() }).await;
            this.update(cx, |this, cx| {
                this.usage = Some(usage);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// ウィンドウの大きさを覚える（大きさを変えている間は書き込まず、落ち着いてから保存する）。
    fn remember_window_size(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let WindowBounds::Windowed(bounds) = window.window_bounds() else { return };
        // 枠をアプリが描くときは、影の分だけ外側に広い。次の起動で大きくならないよう、影を除いた大きさを覚える
        let shadow = gpui_kit::component::window_paddings(window);
        let size = (
            f32::from(bounds.size.width - shadow.left - shadow.right),
            f32::from(bounds.size.height - shadow.top - shadow.bottom),
        );
        self.bounds_save += 1;
        let id = self.bounds_save;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(800)).await;
            if this.update(cx, |this, _| this.bounds_save == id).unwrap_or(false) {
                cx.background_executor()
                    .spawn(async move {
                        if let Err(err) = config::State::update(|state| state.window = Some(size)) {
                            eprintln!("state: {err:#}");
                        }
                    })
                    .await;
            }
        })
        .detach();
    }

    /// 写真を削除してよいか確かめる。
    fn confirm_delete(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let today = self.today_photos();
        if !self.can_delete(key, &today) {
            return;
        }
        let Some(title) = self.find_stored(key).map(|s| s.display_title()) else { return };
        let entity = cx.entity().downgrade();
        let key = key.to_string();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let entity = entity.clone();
            let key = key.clone();
            alert
                .confirm()
                .title(tr!("delete.title", title = title))
                .description(tr!("delete.body"))
                .ok_text(tr!("action.delete"))
                .ok_variant(gpui_kit::component::button::ButtonVariant::Danger)
                .cancel_text(tr!("action.cancel"))
                .on_ok(move |_, _, cx| {
                    entity.update(cx, |this, cx| this.delete(&key, cx)).ok();
                    true
                })
        });
        cx.notify();
    }

    /// 写真を保存先から削除する。選んでいた写真なら、隣の写真に移る。
    fn delete(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(&index) = self.stored_index.get(key) else { return };
        if self.shown.as_deref() == Some(key) {
            let keys = self.ordered_keys();
            let position = keys.iter().position(|k| k == key);
            self.selected = position.and_then(|i| keys.get(i + 1).or_else(|| i.checked_sub(1).and_then(|j| keys.get(j)))).cloned();
        }
        let image = self.stored.remove(index);
        self.rebuild_index();
        self.deleted.insert(image.key.clone());
        self.usage = None;
        self.set_status(tr!("status.deleted", title = image.display_title()), false);
        cx.notify();
        let library = self.library.clone();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { library.delete(&image) }).await;
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

    /// 写真の情報（解説文など）を横のパネルに出す。
    fn open_info(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.shown.clone() else { return };
        // 解説文がまだ無い古い写真は、アーカイブから補う
        if self.find_stored(&key).is_some_and(|s| s.descriptions.is_empty()) {
            self.ensure_descriptions(cx);
        }
        let dashboard = cx.entity();
        let view = cx.new(|cx| info::InfoView::new(dashboard, cx));
        let weak = cx.entity().downgrade();
        let radii = window_corner_radii(window, cx);
        window.open_sheet(cx, move |sheet, _, _| {
            let weak = weak.clone();
            sheet
                .title(tr!("info.title"))
                .size(px(420.))
                // 窓の右の角に接するので、同じ丸みにする
                .rounded_tr(radii.top_right)
                .rounded_br(radii.bottom_right)
                .child(view.clone())
                .on_close(move |_, _, cx| {
                    weak.update(cx, |_, cx| cx.notify()).ok();
                })
        });
        cx.notify();
    }

    /// 題名と撮影地（著作権表示を含む）をクリップボードにコピーする。
    fn copy_caption(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(image) = self.find_stored(key) else { return };
        let text = format!("{}\n{}", image.display_title(), image.display_copyright());
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.set_status(tr!("status.copied"), false);
        cx.notify();
    }

    /// この写真について Bing で調べるページを、ブラウザーで開く。
    fn open_bing(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(image) = self.find_stored(key) {
            cx.open_url(&image.bing_link());
        }
    }

    /// Home / End: 一覧の最初・最後までスクロールする。
    fn scroll_to_edge(&mut self, end: bool, cx: &mut Context<Self>) {
        if end {
            self.list_state.scroll_to_end();
        } else {
            self.list_state.scroll_to(ListOffset { item_ix: 0, offset_in_item: px(0.) });
        }
        // 最初に戻ったら大きな写真を広げ、最後へ行ったら細い帯にする（ホイールで動かしたときと同じ）
        self.hero_collapsed = end && self.list_state.item_count() > 1;
        self.auto_load_older_soon(cx);
        cx.notify();
    }

    /// PageUp / PageDown: 一覧を 1 画面分（少し重ねて）スクロールする。
    fn scroll_page(&mut self, direction: f32, cx: &mut Context<Self>) {
        // まだ高さを測っていない行を飛び越さないよう、先読みの範囲（800px）より短くする
        let page = (self.list_state.viewport_bounds().size.height * 0.9).clamp(px(200.), px(720.));
        self.list_state.scroll_by(page * direction);
        self.on_list_scrolled(cx);
        self.auto_load_older_soon(cx);
        cx.notify();
    }

    /// 一覧を、その年月の最初の写真までスクロールする（`month` は `202611` の形）。
    fn jump_to_month(&mut self, month: &str, cx: &mut Context<Self>) {
        let ix = self.list_items.iter().position(|item| match item {
            ListItem::Groups(groups) => groups.iter().any(|(date, _)| date.starts_with(month)),
            _ => false,
        });
        if let Some(ix) = ix {
            self.list_state.scroll_to(ListOffset { item_ix: ix, offset_in_item: px(0.) });
            // 一覧を広く使えるように、大きな写真は細い帯にする
            self.hero_collapsed = true;
            cx.notify();
        }
    }

    /// 過去の写真がある年月（新しい順、`202611` の形）。
    fn past_months(&self) -> Vec<String> {
        let mut months: Vec<String> = Vec::new();
        for item in &self.list_items {
            let ListItem::Groups(groups) = item else { continue };
            for (date, _) in groups {
                let month = date.get(..6).unwrap_or(date);
                if months.last().map(String::as_str) != Some(month) {
                    months.push(month.to_string());
                }
            }
        }
        months.dedup();
        months
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
        self.auto_load_older(cx);
        // 高さが変わっている最中は切り替えない（一覧の大きさが変わり、位置の判定が揺れるため）
        if self.hero_height_changed.is_some_and(|t| t.elapsed() < HERO_RESIZE) {
            return;
        }
        // 遊びを持たせる: 120px 以上下で細くし、一番上近くまで戻したら広げる
        let top = self.list_state.logical_scroll_top();
        let y = if top.item_ix > 0 { px(f32::MAX) } else { top.offset_in_item };
        let collapse = if self.hero_collapsed { y > px(8.) } else { y > px(120.) };
        if collapse != self.hero_collapsed {
            self.hero_collapsed = collapse;
            cx.notify();
        }
    }

    /// 写真の並び（日付ごと）を、ウィンドウ幅に収まる行に詰める。
    fn pack_rows(groups: Vec<(String, Vec<String>)>, width: f32) -> Vec<ListItem> {
        let per_row = (((width + TILE_GAP) / (TILE_WIDTH + TILE_GAP)).floor() as usize).max(1);
        let mut rows = Vec::new();
        let mut row: Vec<(String, Vec<String>)> = Vec::new();
        let mut row_width = 0.;
        for (date, keys) in groups {
            // 1 日の写真が 1 行に収まらないときは分ける
            for chunk in keys.chunks(per_row) {
                let w = chunk.len() as f32 * TILE_WIDTH + (chunk.len() as f32 - 1.) * TILE_GAP;
                if !row.is_empty() && row_width + GROUP_GAP + w > width {
                    rows.push(ListItem::Groups(std::mem::take(&mut row)));
                    row_width = 0.;
                }
                if !row.is_empty() {
                    row_width += GROUP_GAP;
                }
                row_width += w;
                row.push((date.clone(), chunk.to_vec()));
            }
        }
        if !row.is_empty() {
            rows.push(ListItem::Groups(row));
        }
        rows
    }

    fn group_by_date<'a>(images: impl Iterator<Item = &'a StoredImage>) -> Vec<(String, Vec<String>)> {
        let mut groups: Vec<(String, Vec<String>)> = Vec::new();
        for s in images {
            match groups.last_mut() {
                Some((date, keys)) if *date == s.startdate => keys.push(s.key.clone()),
                _ => groups.push((s.startdate.clone(), vec![s.key.clone()])),
            }
        }
        groups
    }

    /// 一覧の項目を組み立てる（今日の写真・過去の写真、または検索結果）。
    fn build_list_items(&self, today: &[TodayPhoto], width: f32) -> Vec<ListItem> {
        let mut items = Vec::new();
        if self.query.is_active() {
            let hits = search::filter(&self.stored, &self.query);
            items.push(ListItem::ResultsHeading(hits.len()));
            if self.query.color.is_some() {
                let per_row = (((width + TILE_GAP) / (TILE_WIDTH + TILE_GAP)).floor() as usize).max(1);
                let keys: Vec<String> = hits.iter().map(|s| s.key.clone()).collect();
                items.extend(keys.chunks(per_row).map(|c| ListItem::Tiles(c.to_vec())));
            } else {
                items.extend(Self::pack_rows(Self::group_by_date(hits.into_iter()), width));
            }
        } else {
            let today_keys: HashSet<&str> = today.iter().map(|t| t.key.as_str()).collect();
            let past: Vec<&StoredImage> =
                self.stored.iter().filter(|s| !today_keys.contains(s.key.as_str())).collect();
            items.push(ListItem::Today);
            items.push(ListItem::PastHeading(past.len()));
            items.extend(Self::pack_rows(Self::group_by_date(past.into_iter()), width));
            items.push(ListItem::LoadOlder);
        }
        items
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
        let init = settings::Init {
            language,
            auto: self.config.auto.clone(),
            markets: self.markets.clone(),
        };
        self.measure_usage(cx);
        let view = cx.new(|cx| SettingsView::new(dashboard, init, window, cx));
        let weak = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            dialog
                .title(tr!("settings.title"))
                .w(px(880.))
                .child(view.clone())
                // 閉じたら、窓の形の暗幕を外すために描き直す
                .on_close(move |_, _, cx| {
                    weak.update(cx, |_, cx| cx.notify()).ok();
                })
        });
        cx.notify();
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
        let markets = self.markets.clone();
        cx.spawn(async move |this, cx| {
            let archive = match cached {
                Some(archive) => Some(archive),
                None => {
                    let (photos, errors) =
                        cx.background_executor().spawn(async move { archive::fetch_all(&markets) }).await;
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
        let mut keys = Vec::new();
        for item in &self.list_items {
            match item {
                ListItem::Today => keys.extend(self.today_photos().into_iter().map(|t| t.key)),
                ListItem::Groups(groups) => {
                    keys.extend(groups.iter().flat_map(|(_, k)| k.iter().cloned()))
                }
                ListItem::Tiles(k) => keys.extend(k.iter().cloned()),
                _ => {}
            }
        }
        keys
    }

    /// 選んでいる写真の前後の写真の画像を、裏で読み込んでおく。
    /// 前後を求め直すのは選択か並びが変わったときだけ（写真が数百枚あるので毎フレームは探さない）。
    fn preload_neighbors(&mut self, today: &[TodayPhoto], window: &mut Window, cx: &mut Context<Self>) {
        if self.preload.0 != self.shown || self.preload.1 != self.list_signature {
            let mut paths = Vec::new();
            if let Some(shown) = self.shown.clone() {
                let keys = self.ordered_keys();
                if let Some(i) = keys.iter().position(|k| *k == shown) {
                    let neighbors = [i.checked_sub(1), Some(i + 1)];
                    for key in neighbors.into_iter().flatten().filter_map(|j| keys.get(j)) {
                        // サムネイルしか無い写真は、中サイズの画像も先に取得しておく
                        self.request_preview(key, cx);
                        paths.extend(self.hero_for(key.clone(), today).and_then(|h| h.image));
                    }
                }
            }
            self.preload = (self.shown.clone(), self.list_signature, paths);
        }
        // 毎フレーム使っていることにして、デコード済みの画像をキャッシュに残す
        for path in &self.preload.2 {
            window.use_asset::<ImgResourceLoader>(&Resource::Path(Arc::from(path.as_path())), cx);
        }
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

    /// 写真を含む行が一覧の表示範囲に入るようにスクロールする。
    fn ensure_visible(&self, key: &str) {
        let ix = self.list_items.iter().position(|item| match item {
            ListItem::Groups(groups) => groups.iter().any(|(_, k)| k.iter().any(|x| x == key)),
            ListItem::Tiles(k) => k.iter().any(|x| x == key),
            _ => false,
        });
        // 今日の写真（先頭の項目）なら一番上へ
        self.list_state.scroll_to_reveal_item(ix.unwrap_or(0));
    }

    /// 写真に対する操作のメニュー（右クリックと、大きな写真の「…」ボタンで共通）。
    fn photo_menu(entity: WeakEntity<Self>, key: String, actions: PhotoActions, menu: PopupMenu) -> PopupMenu {
        let item = |label: String, f: fn(&mut Self, &str, &mut Window, &mut Context<Self>)| {
            let entity = entity.clone();
            let key = key.clone();
            PopupMenuItem::new(label).on_click(move |_, window, cx| {
                entity.update(cx, |this, cx| f(this, &key, window, cx)).ok();
            })
        };
        let PhotoActions { can_apply, can_open, can_delete, current, favorite } = actions;
        let favorite_label = if favorite { tr!("favorite.remove") } else { tr!("favorite.add") };
        menu.item(
            item(tr!("action.set_wallpaper"), |this, key, _, cx| this.apply(key.to_string(), cx))
                .disabled(!can_apply || current),
        )
        .item(item(favorite_label, |this, key, _, cx| this.toggle_favorite(key, cx)).disabled(!can_apply))
        .separator()
        .item(
            item(tr!("action.info"), |this, key, window, cx| {
                this.select(key.to_string(), cx);
                this.shown = Some(key.to_string());
                this.open_info(window, cx);
            })
            .disabled(!can_apply),
        )
        .item(item(tr!("action.open_bing"), |this, key, _, cx| this.open_bing(key, cx)).disabled(!can_apply))
        .item(item(tr!("action.copy_caption"), |this, key, _, cx| this.copy_caption(key, cx)).disabled(!can_apply))
        .separator()
        .item(item(tr!("action.open_image"), |this, key, _, cx| this.reveal(key, true, cx)).disabled(!can_open))
        .item(item(tr!("action.show_in_folder"), |this, key, _, cx| this.reveal(key, false, cx)).disabled(!can_open))
        .separator()
        .item(
            item(tr!("action.delete_photo"), |this, key, window, cx| this.confirm_delete(key, window, cx))
                .disabled(!can_delete),
        )
    }

    // ---- 描画 ----

    /// 大きな写真に重ねるタイトルバー。写真にマウスが乗っている間だけ出し、写真を隠さない。
    /// 左に全画面表示、右にアプリの操作とウィンドウの操作（最小化・最大化・閉じる）を置く。
    /// 見えていないときも、上端をつかんでウィンドウを動かせる。
    fn render_window_bar(
        &self,
        p: Palette,
        radii: Corners<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // 写真の上に置くので、白い記号だけを出し、触れたときだけ薄い白で示す
        let on_photo = |cx: &mut Context<Self>| {
            ButtonCustomVariant::new(cx)
                .color(transparent_black())
                .foreground(white())
                .hover(white().opacity(0.18))
                .active(white().opacity(0.28))
        };
        // ボタンは暗い半透明の帯にまとめ、明るい写真（空など）の上でも白い記号が読めるようにする。
        // ボタンを押したときに、ウィンドウの移動が始まらないようにもする
        let pill = |el: Div| {
            el.p_0p5()
                .rounded_full()
                .bg(black().opacity(0.5))
                .border_1()
                .border_color(white().opacity(0.12))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        };
        let fullscreen = window.is_fullscreen();
        let app_buttons = h_flex()
            .gap_1()
            .child(
                Button::new("search")
                    .small()
                    .icon(Icon::new(IconName::Search).text_color(white()))
                    .tooltip(tr!("action.search_tooltip"))
                    .custom(on_photo(cx))
                    .on_click(cx.listener(|this, _, window, cx| this.focus_search(window, cx))),
            )
            .child(
                Button::new("show-favorites")
                    .small()
                    .icon(
                        Icon::new(if self.query.favorites { IconName::StarFill } else { IconName::Star })
                            .text_color(white()),
                    )
                    .tooltip(tr!("favorite.show"))
                    .custom(on_photo(cx))
                    .when(self.query.favorites, |b| b.bg(white().opacity(0.22)))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_favorites_filter(cx))),
            )
            .child(
                Button::new("refresh")
                    .small()
                    .icon(Icon::new(IconName::RefreshCw).text_color(white()))
                    .tooltip(tr!("action.refresh_tooltip"))
                    .custom(on_photo(cx))
                    .loading(self.refreshing)
                    .disabled(self.refreshing)
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            )
            .child(
                Button::new("settings")
                    .small()
                    .icon(Icon::new(IconName::Menu).text_color(white()))
                    .tooltip(tr!("action.settings_tooltip"))
                    .custom(on_photo(cx))
                    .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
            );
        // ウィンドウの操作は、このアプリが枠を描いているとき（GNOME の Wayland など）だけ出す
        let client_decorated = matches!(window.window_decorations(), Decorations::Client { .. });
        let controls = client_decorated.then(|| {
            let supported = window.window_controls();
            let control = |id: &'static str, icon: IconName, close: bool| {
                div()
                    .id(id)
                    .flex()
                    .flex_none()
                    .size(px(28.))
                    .rounded_full()
                    .items_center()
                    .justify_center()
                    .text_color(white())
                    .hover(move |s| if close { s.bg(p.danger) } else { s.bg(white().opacity(0.24)) })
                    .active(move |s| if close { s.bg(p.danger.opacity(0.85)) } else { s.bg(white().opacity(0.34)) })
                    .child(Icon::new(icon).xsmall().text_color(white()))
            };
            h_flex()
                .gap_1()
                .when(supported.minimize, |el| {
                    el.child(
                        control("minimize", IconName::WindowMinimize, false)
                            .on_click(|_, window, cx| {
                                cx.stop_propagation();
                                window.minimize_window();
                            }),
                    )
                })
                .when(supported.maximize, |el| {
                    let icon = if window.is_maximized() { IconName::WindowRestore } else { IconName::WindowMaximize };
                    el.child(control("maximize", icon, false).on_click(|_, window, cx| {
                        cx.stop_propagation();
                        window.zoom_window();
                    }))
                })
                .child(control("close", IconName::WindowClose, true).on_click(|_, window, cx| {
                    cx.stop_propagation();
                    window.remove_window();
                }))
        });

        let (visible, anim) = self.window_bar;
        h_flex()
            .id("window-bar")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(WINDOW_BAR_HEIGHT)
            .rounded_tl(radii.top_left)
            .rounded_tr(radii.top_right)
            .pl_3()
            .pr_3()
            .gap_2()
            .items_center()
            .justify_between()
            // ダブルクリックで最大化、右クリックでウィンドウのメニュー（移動は大きな写真全体で受け付ける）
            .on_double_click(|_, window, _| window.zoom_window())
            .on_mouse_down(MouseButton::Right, |ev, window, _| window.show_window_menu(ev.position))
            // 明るい写真（空など）の上でも白い記号が読めるよう、上端を暗くする。
            // バーより少し長く伸ばして、暗さの境目を目立たせない
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(WINDOW_BAR_HEIGHT * 1.75)
                    .rounded_tl(radii.top_left)
                    .rounded_tr(radii.top_right)
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(black().opacity(0.4), 0.),
                        linear_color_stop(black().opacity(0.), 1.),
                    )),
            )
            .child(
                pill(div()).child(
                    Button::new("fullscreen")
                        .small()
                        .icon(
                            Icon::new(if fullscreen { IconName::Minimize } else { IconName::Maximize })
                                .text_color(white()),
                        )
                        .tooltip(if fullscreen {
                            tr!("action.exit_fullscreen_tooltip")
                        } else {
                            tr!("action.fullscreen_tooltip")
                        })
                        .custom(on_photo(cx))
                        .on_click(|_, window, _| window.toggle_fullscreen()),
                ),
            )
            .child(
                pill(h_flex().gap_1()).child(app_buttons).children(controls.map(|c| {
                    h_flex()
                        .gap_1()
                        .child(div().mx_1().w(px(1.)).h(px(16.)).bg(white().opacity(0.3)))
                        .child(c)
                })),
            )
            .with_animation(
                ("window-bar-content", anim),
                Animation::new(WINDOW_BAR_FADE).with_easing(ease_in_out),
                move |el, t| el.opacity(if visible { t } else { 1. - t }),
            )
    }

    /// 選んだ写真を全幅で表示する。この画面で唯一の大きな要素。
    /// `hero` は今表示している写真、`loading` は選んだ別の写真の画像を読み込み中かどうか。
    fn render_hero(
        &self,
        hero: Option<Hero>,
        loading: bool,
        p: Palette,
        radii: Corners<Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (from, to, anim) = self.hero_height;
        let collapsed = self.hero_collapsed;
        let compact = to < 380.;
        // ウィンドウの上の角は丸いので、写真と下地の上の角も同じ丸みにする
        // （gpui の切り抜きは四角いため、はみ出す要素それぞれに丸みを付ける）
        let (tl, tr) = (radii.top_left, radii.top_right);
        let frame = div()
            .relative()
            .w_full()
            .flex_none()
            .overflow_hidden()
            .rounded_tl(tl)
            .rounded_tr(tr)
            .bg(p.surface);
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
                    .child(img(path).size_full().rounded_tl(tl).rounded_tr(tr).object_fit(ObjectFit::Cover))
            });
        let image = match &hero.image {
            Some(path) => div()
                .absolute()
                .inset_0()
                .child(img(path.clone()).size_full().rounded_tl(tl).rounded_tr(tr).object_fit(ObjectFit::Cover))
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
                .top(WINDOW_BAR_HEIGHT + px(8.))
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
            let key = hero.key.clone();
            let actions = PhotoActions {
                can_apply: hero.can_apply,
                can_open: hero.can_open,
                can_delete: hero.can_delete,
                current: hero.current,
                favorite: self.is_favorite(&key),
            };
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
                .dropdown_menu(move |menu, _, _| Self::photo_menu(entity.clone(), key.clone(), actions, menu))
        };
        // 解説文などを横のパネルに出す
        let info_button = Button::new("info")
            .icon(Icon::new(IconName::Info).text_color(white()))
            .tooltip(format!("{}（Ctrl+I）", tr!("action.info")))
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
            .on_click(cx.listener(|this, _, window, cx| this.open_info(window, cx)));
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
                        // ボタンを押したときに、ウィンドウの移動が始まらないようにする
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .when(hero.can_apply, |el| el.child(info_button).child(favorite_button))
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
        actions: PhotoActions,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let PhotoActions { current, favorite, .. } = actions;
        let selected = self.shown.as_deref() == Some(key);
        let thumb = self.library.thumb_path(key);
        let key_owned = key.to_string();
        let entity = cx.entity().downgrade();
        let menu_key = key.to_string();
        v_flex()
            .id(id.into())
            .w(px(width))
            .flex_none()
            .gap_1p5()
            .on_click(cx.listener(move |this, _, _, cx| this.select(key_owned.clone(), cx)))
            .context_menu(move |menu, _, _| Self::photo_menu(entity.clone(), menu_key.clone(), actions, menu))
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
                            .when(self.has_thumb(key), |el| {
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
            .map(|(i, _)| bing::market_name(self.markets[i]))
            .collect();
        let tiles: Vec<AnyElement> = today
            .iter()
            .map(|photo| {
                let actions = self.photo_actions(&photo.key, today);
                let regions = photo.markets.iter().map(|&i| bing::market_name(self.markets[i])).collect::<Vec<_>>();
                self.tile(
                    SharedString::from(format!("today-{}", photo.key)),
                    300.,
                    &photo.key,
                    photo.info.display_title(),
                    regions_caption(&regions, photo.markets.len() == self.markets.len()),
                    actions,
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

    /// 写真 1 枚のタイル（キーから）。色で並べたときは日付を添える。
    fn stored_tile(&self, key: &str, date_caption: bool, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(s) = self.find_stored(key) else { return div().into_any_element() };
        let caption = if date_caption {
            bing::date_text(&s.startdate, true)
        } else {
            let regions = s.markets.iter().map(|m| bing::market_name(m)).collect::<Vec<_>>();
            regions_caption(&regions, self.in_all_markets(s))
        };
        self.tile(
            SharedString::from(format!("tile-{key}")),
            TILE_WIDTH,
            key,
            s.display_title(),
            caption,
            self.photo_actions(key, &self.today_photos()),
            p,
            cx,
        )
        .into_any_element()
    }

    /// 一覧の 1 項目を描く（gpui の list から、見えている項目についてだけ呼ばれる）。
    fn render_list_item(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::current(window);
        let Some(item) = self.list_items.get(ix).cloned() else { return div().into_any_element() };
        let only_favorites =
            self.query.favorites && self.query.color.is_none() && self.query.text.trim().is_empty();
        let (body, bottom): (AnyElement, f32) = match item {
            ListItem::Today => {
                let today = self.today_photos();
                (self.render_today(&today, p, cx).into_any_element(), 40.)
            }
            ListItem::PastHeading(total) => (
                v_flex()
                    .gap_4()
                    .child(
                        h_flex()
                            .gap_3()
                            .child(Self::section_heading(
                                tr!("past.title"),
                                (total > 0).then(|| tr!("past.count", count = total)),
                                p,
                            ))
                            .child(div().flex_1())
                            .children(self.render_month_jump(cx)),
                    )
                    .when(total == 0, |el| {
                        el.child(div().text_sm().text_color(p.slate).child(tr!("past.empty")))
                    })
                    .into_any_element(),
                16.,
            ),
            ListItem::ResultsHeading(total) => (
                v_flex()
                    .gap_4()
                    .child(if only_favorites {
                        // お気に入りだけを表示しているときは「お気に入り n 枚」にする
                        Self::section_heading(tr!("favorite.filter"), Some(tr!("past.count", count = total)), p)
                    } else {
                        Self::section_heading(tr!("search.results", count = total), Some(tr!("search.hint")), p)
                    })
                    .when(total == 0, |el| {
                        el.child(div().text_sm().text_color(p.slate).child(if only_favorites {
                            tr!("favorite.empty")
                        } else {
                            tr!("search.no_results")
                        }))
                    })
                    .into_any_element(),
                16.,
            ),
            ListItem::Groups(groups) => {
                let groups: Vec<AnyElement> = groups
                    .iter()
                    .map(|(date, keys)| {
                        let tiles: Vec<AnyElement> =
                            keys.iter().map(|k| self.stored_tile(k, false, p, cx)).collect();
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
                                    .child(bing::date_text(date, false)),
                            )
                            .child(h_flex().gap_4().children(tiles))
                            .into_any_element()
                    })
                    .collect();
                (h_flex().items_start().gap_x_8().children(groups).into_any_element(), 24.)
            }
            ListItem::Tiles(keys) => {
                let tiles: Vec<AnyElement> = keys.iter().map(|k| self.stored_tile(k, true, p, cx)).collect();
                (h_flex().gap_4().children(tiles).into_any_element(), 20.)
            }
            ListItem::LoadOlder => {
                // 保存済みの写真のあとに、まだダウンロードしていない古い写真を読み込む案内。
                // 自動で読み込む設定では、ボタンは失敗したあとのやり直しにだけ出す。
                let auto = self.config.older == OlderMode::Auto;
                let note = if self.older_exhausted {
                    tr!("past.no_older").to_string()
                } else if auto {
                    tr!("past.auto_note", source = archive::SOURCE_NAME).to_string()
                } else {
                    tr!("past.load_older_note", source = archive::SOURCE_NAME).to_string()
                };
                let body = v_flex()
                    .gap_2()
                    .items_start()
                    .when(!auto || self.older_failed, |el| {
                        el.child(
                            Button::new("load-older")
                                .small()
                                .label(tr!("past.load_older"))
                                .loading(self.loading_older)
                                .disabled(self.loading_older || self.older_exhausted)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.older_failed = false;
                                    this.load_older(cx)
                                })),
                        )
                    })
                    .when(auto && self.loading_older && !self.older_failed, |el| {
                        el.child(
                            h_flex()
                                .gap_2()
                                .child(Spinner::new().small())
                                .child(div().text_sm().text_color(p.ink).child(tr!("past.auto_loading"))),
                        )
                    })
                    .child(div().text_xs().line_height(px(18.)).text_color(p.slate).child(note))
                    .into_any_element();
                (body, 28.)
            }
        };
        div()
            .px_8()
            .when(ix == 0, |el| el.pt_7())
            .pb(px(bottom))
            .child(body)
            .into_any_element()
    }

    /// 「年月へ移動」のボタン。過去の写真が 2 か月分以上あるときだけ出す。
    /// メニューは年ごとにまとめ、月を選ぶとその月の最初の写真までスクロールする。
    fn render_month_jump(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let months = self.past_months();
        if months.len() < 2 {
            return None;
        }
        let entity = cx.entity().downgrade();
        let month_item = move |month: &str| {
            let (year, number) = (month[..4].parse().unwrap_or(0), month[4..].parse().unwrap_or(1));
            let entity = entity.clone();
            let month = month.to_string();
            PopupMenuItem::new(i18n::format_month(year, number)).on_click(move |_, _, cx| {
                entity.update(cx, |this, cx| this.jump_to_month(&month, cx)).ok();
            })
        };
        // 年ごとにまとめる（1 年分しか無ければ、月をそのまま並べる）
        let mut years: Vec<(String, Vec<String>)> = Vec::new();
        for month in months {
            match years.last_mut() {
                Some((year, list)) if month.starts_with(year.as_str()) => list.push(month),
                _ => years.push((month[..4].to_string(), vec![month])),
            }
        }
        Some(
            Button::new("jump-to-month")
                .small()
                .ghost()
                .icon(IconName::Calendar)
                .label(tr!("past.jump"))
                .dropdown_menu(move |mut menu, window, cx| {
                    if let [(_, list)] = years.as_slice() {
                        for month in list {
                            menu = menu.item(month_item(month));
                        }
                        return menu;
                    }
                    for (year, list) in &years {
                        let (list, month_item) = (list.clone(), month_item.clone());
                        menu = menu.submenu(i18n::format_year(year), window, cx, move |mut sub, _, _| {
                            for month in &list {
                                sub = sub.item(month_item(month));
                            }
                            sub
                        });
                    }
                    menu.scrollable(true).max_h(px(360.))
                }),
        )
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
            .child(
                div().w(px(320.)).child(
                    Input::new(&self.search_input)
                        .small()
                        .prefix(Icon::new(IconName::Search).small().text_color(p.slate))
                        .cleanable(true),
                ),
            )
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

    fn render_footer(&self, p: Palette, radii: Corners<Pixels>) -> impl IntoElement {
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
            .rounded_bl(radii.bottom_left)
            .rounded_br(radii.bottom_right)
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
        // 一覧の項目を組み立て、変わったときだけリストを作り直す（スクロール位置は保つ）
        let width = f32::from(window.viewport_size().width) - LIST_SIDE_PADDING;
        let items = self.build_list_items(&today, width);
        let signature = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            items.hash(&mut h);
            h.finish()
        };
        if signature != self.list_signature {
            let top = self.list_state.logical_scroll_top();
            self.list_state.reset(items.len());
            if top.item_ix < items.len() {
                self.list_state.scroll_to(top);
            }
            self.list_signature = signature;
        }
        self.list_items = items;
        // 高さ: スクロール中は細い帯、それ以外は広げた高さ。変わったらアニメーションをやり直す
        let target_height =
            if self.hero_collapsed { HERO_COLLAPSED_HEIGHT } else { Self::hero_full_height(window) };
        if self.hero_height.1 != target_height {
            let from = if self.hero_height.2 == 0 { target_height } else { self.hero_height.1 };
            self.hero_height = (from, target_height, self.hero_height.2 + 1);
            self.hero_height_changed = Some(std::time::Instant::now());
        }
        if let Some(key) = self.shown.clone() {
            self.request_preview(&key, cx);
        }
        self.preload_neighbors(&today, window, cx);
        // タイトルバーは、写真にマウスが乗っているとき（写真がまだ無いときは常に）だけ出す
        let bar_visible = self.hero_hovered || hero.as_ref().is_none_or(|h| h.image.is_none());
        if self.window_bar.0 != bar_visible {
            self.window_bar = (bar_visible, self.window_bar.1 + 1);
        }
        let radii = window_corner_radii(window, cx);
        // ダイアログを開いている間は、窓の形（角の丸み）に合わせて後ろを暗くする
        let scrim = (window.has_active_dialog(cx) || window.has_active_sheet(cx)).then(|| {
            div()
                .absolute()
                .inset_0()
                .rounded_tl(radii.top_left)
                .rounded_tr(radii.top_right)
                .rounded_bl(radii.bottom_left)
                .rounded_br(radii.bottom_right)
                .bg(p.scrim)
        });
        v_flex()
            .relative()
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
            .on_action(|_: &ToggleFullscreen, window, _| window.toggle_fullscreen())
            .on_action(cx.listener(|this, _: &ShowInfo, window, cx| this.open_info(window, cx)))
            .on_action(cx.listener(|this, _: &ScrollHome, _, cx| this.scroll_to_edge(false, cx)))
            .on_action(cx.listener(|this, _: &ScrollEnd, _, cx| this.scroll_to_edge(true, cx)))
            .on_action(cx.listener(|this, _: &ScrollPageUp, _, cx| this.scroll_page(-1., cx)))
            .on_action(cx.listener(|this, _: &ScrollPageDown, _, cx| this.scroll_page(1., cx)))
            .on_action(cx.listener(|this, _: &DeleteSelected, window, cx| {
                if let Some(key) = this.shown.clone() {
                    this.confirm_delete(&key, window, cx);
                }
            }))
            .font_family(theme::FONT)
            .rounded_tl(radii.top_left)
            .rounded_tr(radii.top_right)
            .rounded_bl(radii.bottom_left)
            .rounded_br(radii.bottom_right)
            .bg(p.fog)
            .text_color(p.ink)
            // 大きな写真は上部に固定し、一覧だけをスクロールさせる。
            // 下の方の写真を選んでも、選んだ写真が常に見える。
            // タイトルバーは写真の上に重ね、写真を窓の上端まで広げる。
            .child(
                div()
                    .id("hero-area")
                    .relative()
                    .flex_none()
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.hero_hovered = *hovered;
                        cx.notify();
                    }))
                    // 大きな写真（とタイトルバー）を押したまま動かすと、ウィンドウを移動する。
                    // ボタンの上で押したときは、各ボタンの側で伝播を止めているので移動しない。
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, _| this.window_bar_pressed = true),
                    )
                    // 動かさずに離したら（＝クリック）、細い帯になっている大きな写真を元の高さに戻す
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if std::mem::take(&mut this.window_bar_pressed) && this.hero_collapsed {
                                this.hero_collapsed = false;
                                cx.notify();
                            }
                        }),
                    )
                    .when(self.hero_collapsed, |el| el.cursor_pointer())
                    .on_mouse_down_out(cx.listener(|this, _, _, _| this.window_bar_pressed = false))
                    .on_mouse_move(cx.listener(|this, _, window, _| {
                        if this.window_bar_pressed {
                            this.window_bar_pressed = false;
                            window.start_window_move();
                        }
                    }))
                    .child(self.render_hero(hero, loading, p, radii, cx))
                    .child(self.render_window_bar(p, radii, window, cx)),
            )
            .when(self.search_open || self.query.is_active(), |el| el.child(self.render_search_bar(p, cx)))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    // 見えている項目だけを描く仮想リスト
                    .child(
                        list(
                            self.list_state.clone(),
                            cx.processor(|this, ix: usize, window, cx| this.render_list_item(ix, window, cx)),
                        )
                        .size_full(),
                    )
                    .vertical_scrollbar(&self.list_state)
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
            .child(self.render_footer(p, radii))
            .children(scrim)
    }
}

/// ネットワークがまだつながっていないとき（ログイン直後など）に備えて、間を置いてやり直す。
fn with_retry<T>(mut run: impl FnMut() -> anyhow::Result<T>) -> anyhow::Result<T> {
    const WAITS: [u64; 3] = [5, 15, 30];
    let mut attempt = 0;
    loop {
        match run() {
            Ok(value) => return Ok(value),
            Err(err) if attempt < WAITS.len() => {
                eprintln!("{}: {err:#}", tr!("cli.retrying", seconds = WAITS[attempt]));
                std::thread::sleep(Duration::from_secs(WAITS[attempt]));
                attempt += 1;
            }
            Err(err) => return Err(err),
        }
    }
}

/// 0 以上 `n` 未満の無作為な数（乱数のライブラリを足さず、標準ライブラリの無作為なハッシュの種を使う）。
fn random_below(n: usize) -> usize {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    (hasher.finish() % n.max(1) as u64) as usize
}

/// `--apply --random [--favorites]` で壁紙にする写真の候補。今の壁紙は、ほかに候補があれば外す。
fn random_candidates(stored: Vec<StoredImage>, favorites: bool, current: Option<&str>) -> Vec<StoredImage> {
    let pool: Vec<StoredImage> = stored.into_iter().filter(|s| !favorites || s.favorite).collect();
    let others: Vec<StoredImage> = pool
        .iter()
        .filter(|s| current.is_none_or(|file| s.image_file.as_deref() != Some(file)))
        .cloned()
        .collect();
    if others.is_empty() { pool } else { others }
}

/// `--apply`: GUI を開かずに壁紙を変える（systemd timer 用）。
/// - `--mkt ja-JP`: 今日の写真の地域
/// - `--if-new`: 前回の自動更新と同じ写真なら何もしない
/// - `--random [--favorites]`: 保存した写真（お気に入り）から無作為に選ぶ
fn run_cli(args: &[String]) -> anyhow::Result<()> {
    let has = |flag: &str| args.iter().any(|a| a == flag);
    let config = Config::load();
    let library = Library::new(config.save_dir());

    if has("--random") {
        let favorites = has("--favorites");
        // 今の壁紙のファイル名（保存先からの相対パス）
        let prefix = format!("{}/", wallpaper::file_uri(library.dir()));
        let current_uri = wallpaper::current_uri();
        let current = current_uri.as_deref().and_then(|uri| uri.strip_prefix(prefix.as_str()));
        let candidates = random_candidates(library.load_all(), favorites, current);
        if candidates.is_empty() {
            anyhow::bail!(if favorites { tr!("cli.no_favorites") } else { tr!("cli.no_photos") });
        }
        let image = &candidates[random_below(candidates.len())];
        // 古い写真は、ここで壁紙用の画像をダウンロードする
        let (_, path) = with_retry(|| library.ensure_image(image))?;
        wallpaper::set_wallpaper(&path, config.fit)?;
        println!("{} {}", bing::date_text(&image.startdate, true), image.display_title());
        println!("{}: {}", tr!("cli.wallpaper_set"), path.display());
        return Ok(());
    }

    let mkt = match args.iter().position(|a| a == "--mkt").and_then(|i| args.get(i + 1)) {
        Some(code) => code.as_str(),
        None => config.auto_market(),
    };
    let image = with_retry(|| bing::fetch_archive(mkt, 0, 1))?
        .into_iter()
        .next()
        .expect("fetch_archive returns at least one image");
    let key = image.key();
    if has("--if-new") && config::State::load().last_auto.as_deref() == Some(key.as_str()) {
        println!("{}: {}", tr!("cli.already_applied"), image.display_title());
        return Ok(());
    }
    let stored = with_retry(|| library.store(&PhotoSource::from_bing(&image, mkt), &[mkt], true))?;
    let path = library
        .image_path(&stored)
        .ok_or_else(|| anyhow::anyhow!(tr!("error.saved_image_missing")))?;
    wallpaper::set_wallpaper(&path, config.fit)?;
    config::State::update(|state| state.last_auto = Some(key))?;
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

    // すでに起動していたら、そのウィンドウを前に出してもらって終了する
    let wake = match single::acquire() {
        single::Instance::AlreadyRunning => return,
        single::Instance::First(wake) => wake,
    };

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.bind_keys(key_bindings());
            let options = WindowOptions {
                // 前回の大きさで開く（初回は 1280x900）
                window_bounds: Some(WindowBounds::centered(
                    match config::State::load().window {
                        Some((w, h)) => size(px(w.max(820.)), px(h.max(560.))),
                        None => size(px(1280.), px(900.)),
                    },
                    cx,
                )),
                window_min_size: Some(size(px(820.), px(560.))),
                app_id: Some("bing-wallpaper-manager".into()),
                // 枠はこのアプリが描く（角を丸くし、タイトルバーを大きな写真に重ねるため）
                window_decorations: Some(WindowDecorations::Client),
                ..gpui_kit::component::TitleBar::window_options()
            };
            let (window, _) = gpui_kit::open_window(options, cx, |window, cx| {
                theme::apply(window, cx);
                cx.new(|cx| Dashboard::new(window, cx))
            })
            .expect("failed to open window");
            cx.activate(true);
            // もう一度起動されたら、新しいウィンドウを開かずに、このウィンドウを前に出す
            if let Some(wake) = wake {
                cx.spawn(async move |cx| {
                    loop {
                        cx.background_executor().timer(Duration::from_millis(300)).await;
                        if wake.try_iter().count() == 0 {
                            continue;
                        }
                        let shown = cx.update(|cx| {
                            cx.activate(true);
                            window.update(cx, |_, window, _| window.activate_window()).is_ok()
                        });
                        // ウィンドウを閉じたあとは、待ち受けをやめる
                        if !shown {
                            break;
                        }
                    }
                })
                .detach();
            }
        });
}
