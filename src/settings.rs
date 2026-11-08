//! 設定ダイアログ。左に項目の一覧、右にその内容を出す。
//!
//! - 一般: 言語・テーマ
//! - 壁紙: 写真とモニターの縦横比が違うときの表示方法（プレビュー付き）
//! - 保存先: 写真を保管するフォルダ
//! - このアプリについて: バージョン・ライセンス・使用しているライブラリ

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::IndexPath;
use gpui_kit::component::{Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::*;

use crate::Dashboard;
use crate::config::{FitMode, Language, ThemePref};
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
    Wallpaper,
    Storage,
    About,
}

impl Section {
    const ALL: [Section; 4] = [Section::General, Section::Wallpaper, Section::Storage, Section::About];

    fn label(self) -> String {
        match self {
            Section::General => tr!("settings.general"),
            Section::Wallpaper => tr!("settings.wallpaper"),
            Section::Storage => tr!("settings.storage"),
            Section::About => tr!("settings.about"),
        }
    }
}

/// 言語のドロップダウン（項目は Language::ALL の表示名）。
type LanguageSelect = SelectState<Vec<String>>;

pub struct SettingsView {
    dashboard: Entity<Dashboard>,
    section: Section,
    libraries: Vec<[&'static str; 4]>,
    /// 言語は 12 個あるので、ラジオボタンではなくドロップダウンにする（デザインガイド: 長い選択肢は Select）
    language_select: Entity<LanguageSelect>,
    _observe: Subscription,
    _language_events: Subscription,
}

impl SettingsView {
    /// `language` は今の設定。ダイアログは Dashboard の更新中に作られるので、ここで Dashboard を読まない。
    pub fn new(dashboard: Entity<Dashboard>, language: Language, window: &mut Window, cx: &mut Context<Self>) -> Self {
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
            _observe: observe,
            _language_events: language_events,
        }
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
            cx.notify();
        })
    }

    fn render_nav(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w(px(180.))
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

    fn render_storage(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let save_dir = self.dashboard.read(cx).library.dir().display().to_string();
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
                .child(div().text_sm().text_color(p.ink).child(save_dir))
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
            Section::Wallpaper => self.render_wallpaper(p, window, cx).into_any_element(),
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
