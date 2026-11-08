//! 設定ダイアログ。左に項目の一覧、右にその内容を出す。
//!
//! - 一般: 言語・テーマ
//! - 壁紙: 写真とモニターの縦横比が違うときの表示方法（プレビュー付き）
//! - 保存先: 写真を保管するフォルダ
//! - このアプリについて: バージョン・ライセンス・使用しているライブラリ

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::scroll::ScrollableElement as _;
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

    fn label(self) -> &'static str {
        match self {
            Section::General => tr!("一般", "General"),
            Section::Wallpaper => tr!("壁紙", "Wallpaper"),
            Section::Storage => tr!("保存先", "Storage"),
            Section::About => tr!("このアプリについて", "About"),
        }
    }
}

pub struct SettingsView {
    dashboard: Entity<Dashboard>,
    section: Section,
    libraries: Vec<[&'static str; 4]>,
    _observe: Subscription,
}

impl SettingsView {
    pub fn new(dashboard: Entity<Dashboard>, cx: &mut Context<Self>) -> Self {
        // ダッシュボードの状態（表示中の写真・設定）が変わったら描き直す
        let observe = cx.observe(&dashboard, |_, _, cx| cx.notify());
        let libraries = THIRD_PARTY
            .lines()
            .filter_map(|l| {
                let mut f = l.split('\t');
                Some([f.next()?, f.next()?, f.next()?, f.next()?])
            })
            .collect();
        Self { dashboard, section: Section::General, libraries, _observe: observe }
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
    fn field(title: &str, description: String, control: impl IntoElement, p: Palette) -> impl IntoElement {
        v_flex()
            .gap_2()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(p.ink).child(title.to_string()))
                    .child(div().text_xs().line_height(px(18.)).text_color(p.slate).child(description)),
            )
            .child(control)
    }

    fn render_general(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let config = &self.dashboard.read(cx).config;
        let (language, theme) = (config.language, config.theme);
        let set_language = {
            let dashboard = self.dashboard.clone();
            move |ix: &usize, window: &mut Window, cx: &mut App| {
                let lang = Language::ALL[*ix];
                dashboard.update(cx, |this, cx| this.set_language(lang, window, cx));
            }
        };
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
                tr!("言語", "Language"),
                tr!("画面の表示言語。自動は OS の言語に合わせます。", "Display language. Automatic follows the OS.").into(),
                RadioGroup::vertical("language")
                    .selected_index(Language::ALL.iter().position(|l| *l == language))
                    .children(Language::ALL.iter().enumerate().map(|(i, l)| Radio::new(("language", i)).label(l.label())))
                    .on_change(set_language),
                p,
            ))
            .child(Self::field(
                tr!("テーマ", "Theme"),
                tr!(
                    "配色。システムに合わせると OS のライト／ダークに追従します。",
                    "Color scheme. Match the system follows the OS light or dark mode."
                )
                .into(),
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
            note.push_str(tr!(" 今の壁紙にもすぐ反映されます。", " The current wallpaper updates right away."));
        }
        Self::field(
            tr!("表示方法", "Fit"),
            tr!(
                format!(
                    "写真とモニター（{}×{}）の縦横比が違うときの合わせ方です。下は表示中の写真での見え方です。",
                    monitor.width.round(),
                    monitor.height.round()
                ),
                format!(
                    "How the photo fits when its shape differs from the monitor ({}×{}). Below is a preview with the shown photo.",
                    monitor.width.round(),
                    monitor.height.round()
                )
            ),
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
            tr!("保存先", "Storage folder"),
            tr!(
                "取得した写真はここに保管され、日が変わると「過去の写真」に並びます。",
                "Downloaded photos are kept here and appear under Earlier photos the next day."
            )
            .into(),
            v_flex()
                .gap_3()
                .child(div().text_sm().text_color(p.ink).child(save_dir))
                .child(
                    h_flex()
                        .gap_2()
                        .child(Button::new("choose-dir").small().label(tr!("変更…", "Change…")).on_click(choose_dir))
                        .child(
                            Button::new("open-dir")
                                .small()
                                .ghost()
                                .label(tr!("フォルダを開く", "Open folder"))
                                .on_click(open_dir),
                        ),
                ),
            p,
        )
    }

    fn render_about(&self, p: Palette) -> impl IntoElement {
        let row = |label: &str, value: String| {
            h_flex()
                .gap_4()
                .text_sm()
                .child(div().w(px(120.)).text_color(p.slate).child(label.to_string()))
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
                    .child(tr!("Bing 壁紙", "Bing Wallpaper")),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(row(tr!("バージョン", "Version"), env!("CARGO_PKG_VERSION").to_string()))
                    .child(row(tr!("ライセンス", "License"), env!("CARGO_PKG_LICENSE").to_string())),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(p.ink)
                    .child(format!("Copyright (c) 2026 {}", env!("CARGO_PKG_AUTHORS"))),
            )
            .child(div().text_xs().text_color(p.slate).child(tr!(
                "MIT License と Apache License 2.0 のどちらかを選んで利用できます。",
                "You may use it under either the MIT License or the Apache License 2.0."
            )))
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
                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(p.ink).child(tr!(
                        format!("使用しているライブラリ（{} 件）", self.libraries.len()),
                        format!("Libraries used ({})", self.libraries.len())
                    )))
                    .child(div().text_xs().line_height(px(18.)).text_color(p.slate).child(tr!(
                        "クリックするとそのライブラリのページを開きます。一覧は THIRD_PARTY_LICENSES.md にもあります。",
                        "Click a library to open its page. The list is also in THIRD_PARTY_LICENSES.md."
                    ))),
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
