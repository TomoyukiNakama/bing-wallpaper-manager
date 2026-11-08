mod bing;
mod config;
mod library;
mod settings;
mod theme;
mod wallpaper;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{
    Disableable as _, Icon, IconName, Placement, Sizable as _, TitleBar, WindowExt as _, h_flex,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use bing::{BingImage, MARKETS};
use config::{Config, FitMode};
use library::{Library, StoredImage};
use settings::SettingsPanel;
use theme::Palette;

gpui_kit::actions!(
    bing_wallpaper,
    [SelectPrevious, SelectNext, ApplySelected, Refresh, OpenSettings]
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
    ]
}

/// 「過去の写真」を一度に表示する枚数。
const PAST_PAGE: usize = 48;
/// タイトルバーとフッターの高さの合計（大きな写真の高さの計算用）。
const CHROME_HEIGHT: Pixels = px(64.);
/// 大きな写真を固定しても、一覧に必ず残す高さ。
const LIST_MIN_HEIGHT: Pixels = px(260.);

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
    focus_handle: FocusHandle,
    /// 一覧のスクロール位置（キーボードで選んだ写真を見える位置に動かすため自前で持つ）
    list_scroll: ScrollHandle,
    /// 前回描画したときの各タイルの位置
    tile_bounds: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    /// 「写真全体を表示」などで余った部分に GNOME が塗る色（設定画面のプレビュー用）
    margin_color: Hsla,
    _appearance: Subscription,
}

fn markets_label(markets: impl IntoIterator<Item = String>) -> String {
    let names: Vec<String> = markets.into_iter().collect();
    if names.len() == MARKETS.len() {
        format!("全 {} 地域で配信", names.len())
    } else {
        format!("{}で配信", names.join("、"))
    }
}

/// タイル用の短い地域表記。
fn regions_caption(names: &[&str]) -> String {
    match names.len() {
        n if n == MARKETS.len() => "全地域".to_string(),
        0..=3 => names.join("、"),
        n => format!("{}ほか {} 地域", names[..2].join("、"), n - 2),
    }
}

impl Dashboard {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = Config::load();
        let library = Library::new(config.save_dir());
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            theme::apply(window, cx);
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
            _appearance: appearance,
        };
        this.refresh(cx);
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
        photos
    }

    fn hero(&self, today: &[TodayPhoto]) -> Option<Hero> {
        let key = self
            .selected
            .clone()
            .or_else(|| today.first().map(|p| p.key.clone()))
            .or_else(|| self.stored.first().map(|s| s.key.clone()))?;
        let stored = self.find_stored(&key);
        let image = stored
            .and_then(|s| self.library.image_path(s))
            .or_else(|| Some(self.library.thumb_path(&key)).filter(|p| p.is_file()));
        let can_apply = stored.is_some_and(|s| self.library.image_path(s).is_some());
        let current = stored.is_some_and(|s| self.is_current(s));

        if let Some(photo) = today.iter().find(|p| p.key == key) {
            return Some(Hero {
                title: photo.info.display_title(),
                description: photo.info.copyright.clone(),
                startdate: photo.info.startdate.clone(),
                markets: markets_label(photo.markets.iter().map(|&i| MARKETS[i].1.to_string())),
                key,
                image,
                can_apply,
                current,
            });
        }
        let s = stored?;
        Some(Hero {
            title: s.display_title(),
            description: s.copyright.clone(),
            startdate: s.startdate.clone(),
            markets: markets_label(s.markets.iter().map(|m| bing::market_name(m).to_string())),
            key,
            image,
            can_apply,
            current,
        })
    }

    /// 全地域の画像一覧を取得し、最新のものから順に保存先へ保管する。
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        self.progress = "各地域の写真を確認しています…".into();
        for latest in &mut self.latest {
            latest.state = LatestState::Loading;
        }
        cx.notify();

        let generation = self.generation;
        let library = self.library.clone();
        cx.spawn(async move |this, cx| {
            let tasks: Vec<_> = MARKETS
                .iter()
                .map(|(code, _)| {
                    let code = code.to_string();
                    cx.background_executor()
                        .spawn(async move { bing::fetch_archive(&code, 8) })
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
                    .spawn(async move { lib.store(&image, market) })
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
                        this.progress = format!("写真を保存しています（{}/{total}）", n + 1).into();
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
                if failures == 0 {
                    this.set_status("最新の写真を取得しました", false);
                } else {
                    this.set_status(
                        format!("{failures} 件を取得できませんでした。ネットワークを確認して更新してください"),
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
        let Some(stored) = self.find_stored(&key) else { return };
        let title = stored.display_title();
        let Some(path) = self.library.image_path(stored) else {
            self.set_status("この写真はまだ保存されていません", true);
            cx.notify();
            return;
        };
        self.applying = true;
        let fit = self.config.fit;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let target = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move { wallpaper::set_wallpaper(&target, fit) })
                .await;
            this.update(cx, |this, cx| {
                this.applying = false;
                match result {
                    Ok(()) => {
                        this.current_uri = Some(wallpaper::file_uri(&path));
                        this.set_status(format!("「{title}」を壁紙にしました"), false);
                    }
                    Err(err) => this.set_status(format!("{err:#}"), true),
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
            prompt: Some("保存先にする".into()),
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
        self.generation += 1;
        self.refreshing = false;
        self.set_status(format!("保存先を {} に変更しました", self.library.dir().display()), false);
        self.refresh(cx);
    }

    fn select(&mut self, key: String, cx: &mut Context<Self>) {
        self.selected = Some(key);
        cx.notify();
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
        self.set_status(format!("表示方法を「{}」にしました", fit.label()), false);
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
        let panel = cx.new(|cx| SettingsPanel::new(dashboard, cx));
        window.open_sheet_at(Placement::Right, cx, move |sheet, _, _| {
            sheet.title("設定").size(px(440.)).child(panel.clone())
        });
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
        current: bool,
        menu: PopupMenu,
    ) -> PopupMenu {
        let item = |label: &'static str, f: fn(&mut Self, &str, &mut Context<Self>)| {
            let entity = entity.clone();
            let key = key.clone();
            PopupMenuItem::new(label).on_click(move |_, _, cx| {
                entity.update(cx, |this, cx| f(this, &key, cx)).ok();
            })
        };
        menu.item(
            item("壁紙にする", |this, key, cx| this.apply(key.to_string(), cx))
                .disabled(!can_apply || current),
        )
        .separator()
        .item(item("画像を開く", |this, key, cx| this.reveal(key, true, cx)).disabled(!can_apply))
        .item(item("フォルダで表示", |this, key, cx| this.reveal(key, false, cx)).disabled(!can_apply))
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
                        .child("Bing 壁紙"),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new("refresh")
                                .small()
                                .ghost()
                                .icon(IconName::RefreshCw)
                                .tooltip("最新の写真を取得（F5）")
                                .loading(self.refreshing)
                                .disabled(self.refreshing)
                                .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                        )
                        .child(
                            Button::new("settings")
                                .small()
                                .ghost()
                                .label("設定")
                                .tooltip("壁紙の表示方法と保存先（Ctrl+,）")
                                .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
                        ),
                ),
        )
    }

    /// 選んだ写真を全幅で表示する。この画面で唯一の大きな要素。
    fn render_hero(&self, hero: Option<Hero>, p: Palette, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 上部に固定するので、一覧の領域（LIST_MIN_HEIGHT）を必ず残す高さにする
        let viewport = window.viewport_size().height;
        let height = (viewport * 0.5)
            .min(viewport - CHROME_HEIGHT - LIST_MIN_HEIGHT)
            .clamp(px(200.), px(600.));
        let compact = height < px(380.);
        let frame = div().relative().w_full().h(height).flex_none().overflow_hidden().bg(p.surface);

        let Some(hero) = hero else {
            let failed = !self.refreshing;
            return frame.child(
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .text_color(p.slate)
                    .when(!failed, |el| el.child(Spinner::new().large()).child("写真を取得しています…"))
                    .when(failed, |el| {
                        el.child("写真を取得できませんでした。ネットワークを確認して更新してください。")
                    }),
            );
        };

        let image = match &hero.image {
            Some(path) => div()
                .size_full()
                .child(img(path.clone()).size_full().object_fit(ObjectFit::Cover))
                .with_animation(
                    SharedString::from(format!("hero-{}", hero.key)),
                    Animation::new(Duration::from_millis(260)).with_easing(ease_out_quint()),
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

        let key = hero.key.clone();
        let menu_button = {
            let entity = cx.entity().downgrade();
            let (key, can_apply, current) = (hero.key.clone(), hero.can_apply, hero.current);
            // 写真の上に置くので、「壁紙に設定中」と同じ暗い下地と白い記号にする
            // （ButtonCustomVariant は背景を 20% ほどに薄めて使うため、ここでは直接指定する）
            Button::new("photo-menu")
                .icon(Icon::new(IconName::Ellipsis).text_color(white()))
                .tooltip("この写真の操作")
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
                    Self::photo_menu(entity.clone(), key.clone(), can_apply, current, menu)
                })
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
                .child("壁紙に設定中")
                .into_any_element()
        } else {
            Button::new("apply")
                .primary()
                .label("壁紙にする")
                .loading(self.applying)
                .disabled(!hero.can_apply || self.applying)
                .on_click(cx.listener(move |this, _, _, cx| this.apply(key.clone(), cx)))
                .into_any_element()
        };

        frame.child(image).child(
            h_flex()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .px_8()
                .pt(px(if compact { 56. } else { 120. }))
                .pb(px(if compact { 18. } else { 28. }))
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
                                .text_size(px(if compact { 26. } else { 34. }))
                                .line_height(px(if compact { 34. } else { 44. }))
                                .font_weight(FontWeight::LIGHT)
                                .child(hero.title),
                        )
                        .child(
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
                                    bing::date_ja(&hero.startdate, true),
                                    hero.markets
                                )),
                        ),
                )
                .child(h_flex().flex_none().gap_2().child(menu_button).child(button)),
        )
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
        let can_apply = self.find_stored(key).is_some_and(|s| self.library.image_path(s).is_some());
        let entity = cx.entity().downgrade();
        let menu_key = key.to_string();
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
                Self::photo_menu(entity.clone(), menu_key.clone(), can_apply, current, menu)
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
                            .bg(p.line)
                            .when(thumb.is_file(), |el| {
                                el.child(img(thumb).size_full().object_fit(ObjectFit::Cover))
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
                                el.child(div().flex_none().text_color(p.lake).child("壁紙に設定中"))
                            }),
                    ),
            )
    }

    fn section_heading(title: &str, note: Option<String>, p: Palette) -> impl IntoElement {
        h_flex()
            .gap_3()
            .items_baseline()
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.ink)
                    .child(title.to_string()),
            )
            .when_some(note, |el, note| el.child(div().text_xs().text_color(p.slate).child(note)))
    }

    fn render_today(&self, today: &[TodayPhoto], p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let failed: Vec<&str> = self
            .latest
            .iter()
            .enumerate()
            .filter(|(_, l)| l.info.is_none() && matches!(l.state, LatestState::Failed))
            .map(|(i, _)| MARKETS[i].1)
            .collect();
        let tiles: Vec<AnyElement> = today
            .iter()
            .map(|photo| {
                let current = self.find_stored(&photo.key).is_some_and(|s| self.is_current(s));
                let regions = photo.markets.iter().map(|&i| MARKETS[i].1).collect::<Vec<_>>();
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
                "今日の写真",
                (today.len() > 1).then(|| "地域によって写真が異なります".to_string()),
                p,
            ))
            .child(h_flex().flex_wrap().gap_5().children(tiles))
            .when(!failed.is_empty(), |el| {
                el.child(
                    div()
                        .text_xs()
                        .text_color(p.danger)
                        .child(format!("{}の写真を取得できませんでした", failed.join("、"))),
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
                            .child(bing::date_ja(&date, false)),
                    )
                    .child(h_flex().gap_4().children(tiles))
                    .into_any_element()
            })
            .collect();

        let remaining = total.saturating_sub(self.past_limit);
        v_flex()
            .gap_4()
            .child(Self::section_heading("過去の写真", (total > 0).then(|| format!("{total} 枚")), p))
            .when(total == 0, |el| {
                el.child(
                    div()
                        .text_sm()
                        .text_color(p.slate)
                        .child("まだありません。日付が変わると、それまでの写真がここに並びます。"),
                )
            })
            .child(h_flex().flex_wrap().items_start().gap_x_8().gap_y_6().children(group_elements))
            .when(remaining > 0, |el| {
                el.child(
                    h_flex().child(
                        Button::new("more")
                            .small()
                            .label(format!("さらに表示（残り {remaining} 枚）"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.past_limit += PAST_PAGE;
                                cx.notify();
                            })),
                    ),
                )
            })
    }

    fn render_footer(&self, p: Palette) -> impl IntoElement {
        let (text, color) = if self.refreshing {
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
            .when(self.refreshing, |el| el.child(Spinner::new().xsmall().color(p.slate)))
            .child(div().flex_1().min_w_0().truncate().text_color(color).child(text))
            .child(
                div()
                    .flex_none()
                    .text_color(p.slate)
                    .child("← → で写真を選ぶ　Enter で壁紙にする"),
            )
    }
}

impl Render for Dashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::current(window);
        let today = self.today_photos();
        let hero = self.hero(&today);
        self.shown = hero.as_ref().map(|h| h.key.clone());
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
            .font_family(theme::FONT)
            .bg(p.fog)
            .text_color(p.ink)
            .child(self.render_title_bar(p, cx))
            // 大きな写真は上部に固定し、一覧だけをスクロールさせる。
            // 下の方の写真を選んでも、選んだ写真が常に見える。
            .child(self.render_hero(hero, p, window, cx))
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
                            .child(
                                v_flex()
                                    .px_8()
                                    .py_7()
                                    .gap_10()
                                    .child(self.render_today(&today, p, cx))
                                    .child(self.render_past(&today, p, cx)),
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
    let image = bing::fetch_archive(mkt, 1)?
        .into_iter()
        .next()
        .expect("fetch_archive returns at least one image");
    let stored = library.store(&image, mkt)?;
    let path = library
        .image_path(&stored)
        .ok_or_else(|| anyhow::anyhow!("保存した画像が見つかりません"))?;
    wallpaper::set_wallpaper(&path, config.fit)?;
    println!("{} {}", bing::date_ja(&image.startdate, true), image.copyright);
    println!("壁紙にしました: {}", path.display());
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "使い方:\n  bing-wallpaper-manager                        ダッシュボードを開く\n  bing-wallpaper-manager --apply [--mkt ja-JP]  今日の写真を壁紙にして終了"
        );
        return;
    }
    if args.iter().any(|a| a == "--apply") {
        if let Err(err) = run_cli(&args) {
            eprintln!("エラー: {err:#}");
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
