mod bing;
mod config;
mod library;
mod theme;
mod wallpaper;

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable as _, IconName, Sizable as _, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use bing::{BingImage, MARKETS};
use config::Config;
use library::{Library, StoredImage};
use theme::Palette;

/// 「過去の写真」を一度に表示する枚数。
const PAST_PAGE: usize = 48;

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
        let mut this = Self {
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
        cx.notify();

        cx.spawn(async move |this, cx| {
            let target = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move { wallpaper::set_wallpaper(&target) })
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
                    Button::new("refresh")
                        .small()
                        .ghost()
                        .icon(IconName::RefreshCw)
                        .tooltip("最新の写真を取得")
                        .loading(self.refreshing)
                        .disabled(self.refreshing)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                ),
        )
    }

    /// 選んだ写真を全幅で表示する。この画面で唯一の大きな要素。
    fn render_hero(&self, hero: Option<Hero>, p: Palette, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = (window.viewport_size().height * 0.52).clamp(px(320.), px(640.));
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
        let button = if hero.current {
            Button::new("apply")
                .icon(IconName::Check)
                .label("壁紙に設定中")
                .disabled(true)
        } else {
            Button::new("apply")
                .primary()
                .label("壁紙にする")
                .loading(self.applying)
                .disabled(!hero.can_apply || self.applying)
                .on_click(cx.listener(move |this, _, _, cx| this.apply(key.clone(), cx)))
        };

        frame.child(image).child(
            h_flex()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .px_8()
                .pt(px(120.))
                .pb_7()
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
                                .text_size(px(34.))
                                .line_height(px(44.))
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
                .child(button),
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
        v_flex()
            .id(id.into())
            .w(px(width))
            .flex_none()
            .gap_1p5()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.select(key_owned.clone(), cx)))
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

    fn render_footer(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
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
            .child(div().text_color(p.slate).child("保存先"))
            .child(
                div()
                    .max_w(px(420.))
                    .truncate()
                    .text_color(p.ink)
                    .child(self.library.dir().display().to_string()),
            )
            .child(
                Button::new("choose-dir")
                    .xsmall()
                    .ghost()
                    .label("変更")
                    .on_click(cx.listener(|this, _, _, cx| this.choose_save_dir(cx))),
            )
            .child(
                Button::new("open-folder")
                    .xsmall()
                    .ghost()
                    .icon(IconName::ExternalLink)
                    .label("開く")
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Err(err) = wallpaper::open_folder(this.library.dir()) {
                            this.set_status(format!("{err:#}"), true);
                            cx.notify();
                        }
                    })),
            )
            .child(div().flex_1())
            .when(self.refreshing, |el| el.child(Spinner::new().xsmall().color(p.slate)))
            .child(div().max_w(px(560.)).truncate().text_color(color).child(text))
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
            .font_family(theme::FONT)
            .bg(p.fog)
            .text_color(p.ink)
            .child(self.render_title_bar(p, cx))
            .child(
                v_flex()
                    .id("dashboard")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(self.render_hero(hero, p, window, cx))
                    .child(
                        v_flex()
                            .px_8()
                            .py_7()
                            .gap_10()
                            .child(self.render_today(&today, p, cx))
                            .child(self.render_past(&today, p, cx)),
                    ),
            )
            .child(self.render_footer(p, cx))
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
    let library = Library::new(Config::load().save_dir());
    let image = bing::fetch_archive(mkt, 1)?
        .into_iter()
        .next()
        .expect("fetch_archive returns at least one image");
    let stored = library.store(&image, mkt)?;
    let path = library
        .image_path(&stored)
        .ok_or_else(|| anyhow::anyhow!("保存した画像が見つかりません"))?;
    wallpaper::set_wallpaper(&path)?;
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
