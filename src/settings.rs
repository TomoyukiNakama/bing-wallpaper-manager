//! 右から開く設定パネル（壁紙の表示方法・保存先）。

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::Dashboard;
use crate::config::FitMode;
use crate::theme::Palette;

/// プレビューの枠の幅。
const PREVIEW_WIDTH: f32 = 372.;

pub struct SettingsPanel {
    dashboard: Entity<Dashboard>,
    _observe: Subscription,
}

impl SettingsPanel {
    pub fn new(dashboard: Entity<Dashboard>, cx: &mut Context<Self>) -> Self {
        // ダッシュボードの状態（表示中の写真・設定）が変わったら描き直す
        let observe = cx.observe(&dashboard, |_, _, cx| cx.notify());
        Self { dashboard, _observe: observe }
    }

    /// 選んだ表示方法で、表示中の写真がモニターにどう収まるかを描く。
    fn render_preview(&self, fit: FitMode, monitor: Size<f32>, p: Palette, cx: &App) -> impl IntoElement {
        let dashboard = self.dashboard.read(cx);
        let width = PREVIEW_WIDTH;
        let height = width * monitor.height / monitor.width;
        let photo = dashboard.preview_photo();

        let image = photo.map(|(path, photo_size)| match fit {
            FitMode::Zoom | FitMode::Spanned => {
                img(path).size_full().object_fit(ObjectFit::Cover).into_any_element()
            }
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
}

impl Render for SettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::current(window);
        let dashboard = self.dashboard.read(cx);
        let fit = dashboard.config.fit;
        let selected = FitMode::ALL.iter().position(|m| *m == fit);
        let save_dir = dashboard.library.dir().display().to_string();

        // モニターの実ピクセル（論理サイズ × 拡大率）
        let scale = window.scale_factor();
        // Wayland では primary_display() が常に None なので、ウィンドウのあるディスプレイを使う
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

        let heading = |text: &'static str| {
            div().text_sm().font_weight(FontWeight::BOLD).text_color(p.ink).child(text)
        };
        let note = |text: String| div().text_xs().line_height(px(18.)).text_color(p.slate).child(text);

        v_flex()
            .gap_8()
            .pb_6()
            .child(
                v_flex()
                    .gap_3()
                    .child(heading("壁紙の表示方法"))
                    .child(note(format!(
                        "写真とモニター（{}×{}）の縦横比が違うときの合わせ方です。",
                        monitor.width.round(),
                        monitor.height.round()
                    )))
                    .child(self.render_preview(fit, monitor, p, cx))
                    .child(
                        RadioGroup::vertical("fit")
                            .selected_index(selected)
                            .children(
                                FitMode::ALL
                                    .iter()
                                    .enumerate()
                                    .map(|(i, m)| Radio::new(("fit", i)).label(m.label())),
                            )
                            .on_change(set_fit),
                    )
                    .child(note(fit.description().to_string()))
                    .when(dashboard.wallpaper_is_ours(), |el| {
                        el.child(note("今の壁紙にもすぐ反映されます。".to_string()))
                    }),
            )
            .child(
                v_flex()
                    .gap_3()
                    .child(heading("保存先"))
                    .child(div().text_sm().text_color(p.ink).child(save_dir))
                    .child(note(
                        "取得した写真はここに保管され、日が変わると「過去の写真」に並びます。".to_string(),
                    ))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Button::new("choose-dir").small().label("変更…").on_click(choose_dir))
                            .child(
                                Button::new("open-dir")
                                    .small()
                                    .ghost()
                                    .label("フォルダを開く")
                                    .on_click(open_dir),
                            ),
                    ),
            )
    }
}
