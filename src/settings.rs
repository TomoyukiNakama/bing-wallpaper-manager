//! 設定ダイアログ。左に項目の一覧、右にその内容を出す。
//!
//! - 一般: 言語・テーマ
//! - 地域: ダッシュボードに出す Bing の地域
//! - 壁紙: 写真とモニターの縦横比が違うときの表示方法（プレビュー付き）
//! - 自動更新: 壁紙を自動で変える（systemd のユーザータイマー）
//! - 保存先: 写真を保管するフォルダと使用量
//! - このアプリについて: バージョン・ライセンス・使用しているライブラリ

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::IndexPath;
use gpui_kit::component::{Disableable as _, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::Dashboard;
use crate::bing::{self, MARKETS};
use crate::config::{AutoMode, AutoUpdate, FitMode, Interval, Language, ThemePref};
use crate::theme::Palette;

/// プレビューの枠の幅。
const PREVIEW_WIDTH: f32 = 400.;

/// 使用しているライブラリ（scripts/gen-third-party.py で生成）。名前・バージョン・ライセンス・URL。
const THIRD_PARTY: &str = include_str!("third_party.tsv");

const MIT_URL: &str = "https://opensource.org/license/mit";
const APACHE_URL: &str = "https://www.apache.org/licenses/LICENSE-2.0";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    General,
    Regions,
    Wallpaper,
    Auto,
    Storage,
    About,
}

/// 設定画面を開くときの、今の設定（SettingsView::new では Dashboard を読めないので渡す）。
pub struct Init {
    pub language: Language,
    pub auto: AutoUpdate,
    pub markets: Vec<&'static str>,
}

impl Section {
    const ALL: [Section; 6] =
        [Section::General, Section::Regions, Section::Wallpaper, Section::Auto, Section::Storage, Section::About];

    fn label(self) -> String {
        match self {
            Section::General => tr!("settings.general"),
            Section::Regions => tr!("settings.regions"),
            Section::Wallpaper => tr!("settings.wallpaper"),
            Section::Auto => tr!("settings.auto"),
            Section::Storage => tr!("settings.storage"),
            Section::About => tr!("settings.about"),
        }
    }
}

/// 言語のドロップダウン（項目は Language::ALL の表示名）。
type LanguageSelect = SelectState<Vec<String>>;
/// 文字列の項目を並べたドロップダウン（自動更新の地域と間隔）。
type TextSelect = SelectState<Vec<String>>;

pub struct SettingsView {
    dashboard: Entity<Dashboard>,
    section: Section,
    libraries: Vec<[&'static str; 4]>,
    /// 言語は 12 個あるので、ラジオボタンではなくドロップダウンにする（デザインガイド: 長い選択肢は Select）
    language_select: Entity<LanguageSelect>,
    /// 自動更新の「今日の写真」で使う地域（表示している地域から選ぶ）
    market_select: Entity<TextSelect>,
    /// 自動更新で壁紙を切り替える間隔
    interval_select: Entity<TextSelect>,
    _observe: Subscription,
    _language_events: Subscription,
    _market_events: Subscription,
    _interval_events: Subscription,
}

impl SettingsView {
    /// `language` は今の設定。ダイアログは Dashboard の更新中に作られるので、ここで Dashboard を読まない。
    pub fn new(dashboard: Entity<Dashboard>, init: Init, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let language = init.language;
        let market_select = Self::new_market_select(&init.markets, &init.auto, window, cx);
        let market_events = Self::subscribe_market(&market_select, window, cx);
        let interval_select = Self::new_interval_select(init.auto.interval, window, cx);
        let interval_events = Self::subscribe_interval(&interval_select, window, cx);
        // ダッシュボードの状態（表示中の写真・設定）が変わったら描き直す
        let observe = cx.observe(&dashboard, |_, _, cx| cx.notify());
        let language_select = Self::new_language_select(language, window, cx);
        let language_events = Self::subscribe_language(&language_select, window, cx);
        let libraries = THIRD_PARTY
            .lines()
            .filter_map(|l| {
                let mut f = l.split('\t');
                Some([f.next()?, f.next()?, f.next()?, f.next()?])
            })
            .collect();
        Self {
            dashboard,
            section: Section::General,
            libraries,
            language_select,
            market_select,
            interval_select,
            _observe: observe,
            _language_events: language_events,
            _market_events: market_events,
            _interval_events: interval_events,
        }
    }

    fn new_market_select(
        markets: &[&'static str],
        auto: &AutoUpdate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TextSelect> {
        let labels: Vec<String> = markets.iter().map(|m| bing::market_name(m)).collect();
        // 未設定なら、表示している地域の最初のもの
        let selected = auto.market.as_deref().and_then(|code| markets.iter().position(|m| *m == code)).unwrap_or(0);
        cx.new(|cx| SelectState::new(labels, Some(IndexPath::new(selected)), window, cx))
    }

    fn subscribe_market(select: &Entity<TextSelect>, window: &mut Window, cx: &mut Context<Self>) -> Subscription {
        cx.subscribe_in(select, window, |this, select, event: &SelectEvent<Vec<String>>, _, cx| {
            let SelectEvent::Confirm(Some(_)) = event else { return };
            let Some(ix) = select.read(cx).selected_index(cx).map(|ix| ix.row) else { return };
            this.dashboard.update(cx, |d, cx| {
                let Some(market) = d.markets.get(ix) else { return };
                let auto = AutoUpdate { market: Some(market.to_string()), ..d.config.auto.clone() };
                d.set_auto(auto, cx);
            });
        })
    }

    fn new_interval_select(interval: Interval, window: &mut Window, cx: &mut Context<Self>) -> Entity<TextSelect> {
        let labels: Vec<String> = Interval::ALL.iter().map(|i| i.label()).collect();
        let selected = Interval::ALL.iter().position(|i| *i == interval).map(IndexPath::new);
        cx.new(|cx| SelectState::new(labels, selected, window, cx))
    }

    fn subscribe_interval(select: &Entity<TextSelect>, window: &mut Window, cx: &mut Context<Self>) -> Subscription {
        cx.subscribe_in(select, window, |this, select, event: &SelectEvent<Vec<String>>, _, cx| {
            let SelectEvent::Confirm(Some(_)) = event else { return };
            let Some(ix) = select.read(cx).selected_index(cx).map(|ix| ix.row) else { return };
            this.dashboard.update(cx, |d, cx| {
                let auto = AutoUpdate { interval: Interval::ALL[ix], ..d.config.auto.clone() };
                d.set_auto(auto, cx);
            });
        })
    }

    /// 地域と間隔のドロップダウンを作り直す（表示言語や、表示する地域が変わったとき）。
    fn rebuild_auto_selects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (markets, auto) = {
            let d = self.dashboard.read(cx);
            (d.markets.clone(), d.config.auto.clone())
        };
        self.market_select = Self::new_market_select(&markets, &auto, window, cx);
        self._market_events = Self::subscribe_market(&self.market_select, window, cx);
        self.interval_select = Self::new_interval_select(auto.interval, window, cx);
        self._interval_events = Self::subscribe_interval(&self.interval_select, window, cx);
    }

    fn new_language_select(language: Language, window: &mut Window, cx: &mut Context<Self>) -> Entity<LanguageSelect> {
        let labels: Vec<String> = Language::ALL.iter().map(|l| l.label()).collect();
        let selected = Language::ALL.iter().position(|l| *l == language).map(IndexPath::new);
        cx.new(|cx| SelectState::new(labels, selected, window, cx))
    }

    fn subscribe_language(select: &Entity<LanguageSelect>, window: &mut Window, cx: &mut Context<Self>) -> Subscription {
        cx.subscribe_in(select, window, |this, select, event: &SelectEvent<Vec<String>>, window, cx| {
            let SelectEvent::Confirm(Some(_)) = event else { return };
            let Some(ix) = select.read(cx).selected_index(cx).map(|ix| ix.row) else { return };
            let language = Language::ALL[ix];
            this.dashboard.update(cx, |d, cx| d.set_language(language, window, cx));
            // 「自動」の表示名も新しい言語にするため、項目を作り直す
            this.language_select = Self::new_language_select(language, window, cx);
            this._language_events = Self::subscribe_language(&this.language_select, window, cx);
            this.rebuild_auto_selects(window, cx);
            cx.notify();
        })
    }

    fn render_nav(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w(px(210.))
            .h_full()
            .flex_none()
            .gap_1()
            .pr_3()
            .border_r_1()
            .border_color(p.line)
            .children(Section::ALL.iter().enumerate().map(|(i, &section)| {
                Button::new(("section", i))
                    .ghost()
                    .w_full()
                    // Button は中身を中央に置くので、全幅の子要素で項目名を左に揃える
                    .child(div().w_full().child(section.label()))
                    .selected(self.section == section)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.section = section;
                        cx.notify();
                    }))
            }))
    }

    /// 項目名・説明・操作部品からなる 1 行（例: 「言語 / 画面の表示言語…」）。
    fn field(
        title: impl Into<SharedString>,
        description: impl Into<SharedString>,
        control: impl IntoElement,
        p: Palette,
    ) -> impl IntoElement {
        let (title, description): (SharedString, SharedString) = (title.into(), description.into());
        v_flex()
            .gap_2()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(p.ink).child(title))
                    .child(div().text_xs().line_height(px(18.)).text_color(p.slate).child(description)),
            )
            .child(control)
    }

    fn render_general(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let config = &self.dashboard.read(cx).config;
        let theme = config.theme;
        let set_theme = {
            let dashboard = self.dashboard.clone();
            move |ix: &usize, window: &mut Window, cx: &mut App| {
                let pref = ThemePref::ALL[*ix];
                dashboard.update(cx, |this, cx| this.set_theme(pref, window, cx));
            }
        };
        v_flex()
            .gap_6()
            .child(Self::field(
                tr!("settings.language"),
                tr!("settings.language_desc"),
                div().w(px(300.)).child(Select::new(&self.language_select)),
                p,
            ))
            .child(Self::field(
                tr!("settings.theme"),
                tr!("settings.theme_desc"),
                RadioGroup::vertical("theme")
                    .selected_index(ThemePref::ALL.iter().position(|t| *t == theme))
                    .children(ThemePref::ALL.iter().enumerate().map(|(i, t)| Radio::new(("theme", i)).label(t.label())))
                    .on_change(set_theme),
                p,
            ))
    }

    /// 選んだ表示方法で、表示中の写真がモニターにどう収まるかを描く。
    fn render_preview(&self, fit: FitMode, monitor: Size<f32>, p: Palette, cx: &App) -> impl IntoElement {
        let dashboard = self.dashboard.read(cx);
        let width = PREVIEW_WIDTH;
        let height = width * monitor.height / monitor.width;
        let image = dashboard.preview_photo().map(|(path, photo_size)| match fit {
            FitMode::Zoom | FitMode::Spanned => img(path).size_full().object_fit(ObjectFit::Cover).into_any_element(),
            FitMode::Scaled => img(path).size_full().object_fit(ObjectFit::Contain).into_any_element(),
            FitMode::Stretched => img(path).size_full().object_fit(ObjectFit::Fill).into_any_element(),
            FitMode::Centered => {
                // 元の大きさ: モニターの実ピクセルに対する写真の大きさの比で描く
                let scale = width / monitor.width;
                img(path)
                    .flex_none()
                    .w(px(photo_size.width * scale))
                    .h(px(photo_size.height * scale))
                    .into_any_element()
            }
        });
        div()
            .w(px(width))
            .h(px(height))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .rounded(px(6.))
            .border_4()
            .border_color(p.ink)
            .bg(dashboard.margin_color)
            .children(image)
    }

    fn render_wallpaper(&self, p: Palette, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dashboard = self.dashboard.read(cx);
        let fit = dashboard.config.fit;
        let ours = dashboard.wallpaper_is_ours();
        // モニターの実ピクセル（Wayland では primary_display() が常に None なので、ウィンドウのあるディスプレイ）
        let scale = window.scale_factor();
        let monitor = window
            .display(cx)
            .or_else(|| cx.displays().into_iter().next())
            .map(|d| d.bounds().size)
            .map(|s| size(f32::from(s.width) * scale, f32::from(s.height) * scale))
            .unwrap_or(size(1920., 1080.));
        let set_fit = {
            let dashboard = self.dashboard.clone();
            move |ix: &usize, _: &mut Window, cx: &mut App| {
                let fit = FitMode::ALL[*ix];
                dashboard.update(cx, |this, cx| this.set_fit(fit, cx));
            }
        };
        let mut note = fit.description().to_string();
        if ours {
            note.push_str(&tr!("settings.fit_applies_now"));
        }
        Self::field(
            tr!("settings.fit"),
            tr!("settings.fit_desc", size = format!("{}×{}", monitor.width.round(), monitor.height.round())),
            v_flex()
                .gap_3()
                .child(self.render_preview(fit, monitor, p, cx))
                .child(
                    RadioGroup::vertical("fit")
                        .selected_index(FitMode::ALL.iter().position(|m| *m == fit))
                        .children(FitMode::ALL.iter().enumerate().map(|(i, m)| Radio::new(("fit", i)).label(m.label())))
                        .on_change(set_fit),
                )
                .child(div().text_xs().line_height(px(18.)).text_color(p.slate).child(note)),
            p,
        )
    }

    /// ダッシュボードに出す地域。少なくとも 1 つは残すので、最後の 1 つは外せない。
    fn render_regions(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = self.dashboard.read(cx).markets.clone();
        let view = cx.entity();
        let boxes = MARKETS.iter().enumerate().map(|(i, (code, _, _))| {
            let code: &'static str = code;
            let checked = enabled.contains(&code);
            let (dashboard, view) = (self.dashboard.clone(), view.clone());
            Checkbox::new(("region", i))
                .label(bing::market_name(code))
                .checked(checked)
                .disabled(checked && enabled.len() == 1)
                .on_click(move |checked: &bool, window, cx| {
                    dashboard.update(cx, |d, cx| d.set_market_enabled(code, *checked, cx));
                    view.update(cx, |this, cx| {
                        this.rebuild_auto_selects(window, cx);
                        cx.notify();
                    });
                })
        });
        Self::field(
            tr!("settings.regions_shown"),
            tr!("settings.regions_desc"),
            v_flex().gap_2p5().children(boxes),
            p,
        )
    }

    /// 壁紙の自動更新。スイッチを入れると systemd のユーザータイマーを作り、切ると消す。
    fn render_auto(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let dashboard = self.dashboard.read(cx);
        let auto = dashboard.config.auto.clone();
        let error = dashboard.auto_error.clone();
        let no_favorites = !dashboard.stored.iter().any(|s| s.favorite);
        let toggle = {
            let dashboard = self.dashboard.clone();
            move |enabled: &bool, _: &mut Window, cx: &mut App| {
                dashboard.update(cx, |d, cx| {
                    let auto = AutoUpdate { enabled: *enabled, ..d.config.auto.clone() };
                    d.set_auto(auto, cx);
                });
            }
        };
        let set_mode = {
            let dashboard = self.dashboard.clone();
            move |ix: &usize, _: &mut Window, cx: &mut App| {
                dashboard.update(cx, |d, cx| {
                    let auto = AutoUpdate { mode: AutoMode::ALL[*ix], ..d.config.auto.clone() };
                    d.set_auto(auto, cx);
                });
            }
        };
        let mut mode_note = auto.mode.description();
        if auto.mode == AutoMode::Favorites && no_favorites {
            mode_note.push_str(&tr!("auto.no_favorites"));
        }
        v_flex()
            .gap_6()
            .child(Self::field(
                tr!("auto.switch"),
                tr!("auto.desc"),
                v_flex()
                    .gap_2()
                    .child(Switch::new("auto-update").accessibility_label(tr!("auto.switch")).checked(auto.enabled).on_click(toggle))
                    .children(error.map(|e| div().text_xs().line_height(px(18.)).text_color(p.danger).child(e))),
                p,
            ))
            .when(auto.enabled, |el| {
                el.child(Self::field(
                    tr!("auto.mode"),
                    mode_note,
                    RadioGroup::vertical("auto-mode")
                        .selected_index(AutoMode::ALL.iter().position(|m| *m == auto.mode))
                        .children(AutoMode::ALL.iter().enumerate().map(|(i, m)| Radio::new(("auto-mode", i)).label(m.label())))
                        .on_change(set_mode),
                    p,
                ))
                .child(if auto.mode == AutoMode::Today {
                    Self::field(
                        tr!("auto.market"),
                        tr!("auto.market_desc"),
                        div().w(px(300.)).child(Select::new(&self.market_select)),
                        p,
                    )
                } else {
                    Self::field(
                        tr!("auto.interval"),
                        tr!("auto.interval_desc"),
                        div().w(px(300.)).child(Select::new(&self.interval_select)),
                        p,
                    )
                })
            })
    }

    fn render_storage(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let dashboard = self.dashboard.read(cx);
        let save_dir = dashboard.library.dir().display().to_string();
        let count = dashboard.stored.len();
        let usage = match dashboard.usage {
            Some(bytes) => tr!("settings.usage", size = format_size(bytes), count = count),
            None => tr!("settings.usage_counting"),
        };
        let choose_dir = {
            let dashboard = self.dashboard.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                dashboard.update(cx, |this, cx| this.choose_save_dir(cx));
            }
        };
        let open_dir = {
            let dashboard = self.dashboard.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                dashboard.update(cx, |this, cx| this.open_save_dir(cx));
            }
        };
        Self::field(
            tr!("settings.storage_folder"),
            tr!("settings.storage_desc"),
            v_flex()
                .gap_3()
                .child(
                    v_flex()
                        .gap_1()
                        .child(div().text_sm().text_color(p.ink).child(save_dir))
                        .child(div().text_xs().text_color(p.slate).child(usage)),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(Button::new("choose-dir").small().label(tr!("settings.change_dir")).on_click(choose_dir))
                        .child(
                            Button::new("open-dir")
                                .small()
                                .ghost()
                                .label(tr!("settings.open_dir"))
                                .on_click(open_dir),
                        ),
                ),
            p,
        )
    }

    fn render_about(&self, p: Palette) -> impl IntoElement {
        let row = |label: String, value: String| {
            h_flex()
                .gap_4()
                .text_sm()
                .child(div().w(px(120.)).text_color(p.slate).child(label))
                .child(div().text_color(p.ink).child(value))
        };
        let libraries = self.libraries.iter().enumerate().map(|(i, [name, version, license, url])| {
            let url = url.to_string();
            h_flex()
                .id(("library", i))
                .px_2()
                .py_1()
                .gap_4()
                .rounded(px(4.))
                .text_sm()
                .hover(|el| el.bg(p.line))
                .on_click(move |_, _, cx| cx.open_url(&url))
                .child(div().flex_1().min_w_0().truncate().text_color(p.ink).child(name.to_string()))
                .child(div().w(px(90.)).flex_none().text_color(p.slate).child(version.to_string()))
                .child(div().w(px(170.)).flex_none().truncate().text_color(p.slate).child(license.to_string()))
        });
        v_flex()
            .gap_4()
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.ink)
                    .child(tr!("app.name")),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(row(tr!("about.version"), env!("CARGO_PKG_VERSION").to_string()))
                    .child(row(tr!("about.license"), env!("CARGO_PKG_LICENSE").to_string())),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(p.ink)
                    .child(format!("Copyright (c) 2026 {}", env!("CARGO_PKG_AUTHORS"))),
            )
            .child(div().text_xs().text_color(p.slate).child(tr!("about.license_note")))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("mit")
                            .small()
                            .outline()
                            .label("MIT License")
                            .on_click(|_, _, cx| cx.open_url(MIT_URL)),
                    )
                    .child(
                        Button::new("apache")
                            .small()
                            .outline()
                            .label("Apache License 2.0")
                            .on_click(|_, _, cx| cx.open_url(APACHE_URL)),
                    ),
            )
            .child(
                v_flex()
                    .pt_2()
                    .gap_1()
                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(p.ink).child(tr!("about.libraries", count = self.libraries.len())))
                    .child(div().text_xs().line_height(px(18.)).text_color(p.slate).child(tr!("about.libraries_note"))),
            )
            .child(
                div()
                    .h(px(220.))
                    .flex_none()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(p.line)
                    .child(v_flex().size_full().p_1().children(libraries).overflow_y_scrollbar()),
            )
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::current(window);
        let content = match self.section {
            Section::General => self.render_general(p, cx).into_any_element(),
            Section::Regions => self.render_regions(p, cx).into_any_element(),
            Section::Wallpaper => self.render_wallpaper(p, window, cx).into_any_element(),
            Section::Auto => self.render_auto(p, cx).into_any_element(),
            Section::Storage => self.render_storage(p, cx).into_any_element(),
            Section::About => self.render_about(p).into_any_element(),
        };
        h_flex()
            .h(px(540.))
            .items_start()
            .child(self.render_nav(p, cx))
            .child(
                div().flex_1().min_w_0().h_full().pl_6().child(
                    v_flex()
                        .size_full()
                        .gap_4()
                        .pb_4()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.ink)
                                .child(self.section.label()),
                        )
                        .child(content)
                        .overflow_y_scrollbar(),
                ),
            )
    }
}

/// バイト数を読みやすい単位にする（例: `1.2 GB`、`340 MB`）。
fn format_size(bytes: u64) -> String {
    const MB: f64 = 1024. * 1024.;
    let mb = bytes as f64 / MB;
    if mb >= 1024. { format!("{:.1} GB", mb / 1024.) } else { format!("{:.0} MB", mb) }
}

#[cfg(test)]
mod tests {
    // gpui_kit の `test` マクロを取り込まないよう、使う関数だけを取り込む
    use super::format_size;

    #[test]
    fn formats_sizes() {
        assert_eq!(format_size(340 * 1024 * 1024), "340 MB");
        assert_eq!(format_size(1288490189), "1.2 GB");
    }
}
