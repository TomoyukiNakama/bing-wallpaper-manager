//! 写真の情報（題名・撮影地・配信日・地域・解説文）。大きな写真の横に出すパネル（Sheet）の中身。
//!
//! 表示中の写真が変わったら、このパネルもその写真の内容に変わる。

use gpui_kit::component::button::Button;
use gpui_kit::component::link::Link;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;

use crate::theme::Palette;
use crate::{Dashboard, bing};

pub struct InfoView {
    dashboard: Entity<Dashboard>,
    _observe: Subscription,
}

impl InfoView {
    /// パネルは Dashboard の更新中に作られるので、ここで Dashboard を読まない。
    pub fn new(dashboard: Entity<Dashboard>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&dashboard, |_, _, cx| cx.notify());
        Self { dashboard, _observe: observe }
    }
}

impl Render for InfoView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::current(window);
        let dashboard = self.dashboard.read(cx);
        let Some(image) = dashboard.shown.as_deref().and_then(|key| dashboard.find_stored(key)) else {
            return div().into_any_element();
        };
        let key = image.key.clone();
        let regions = image
            .markets
            .iter()
            .map(|m| bing::market_name(m))
            .collect::<Vec<_>>()
            .join(tr!("list.separator").as_ref());
        let row = |label: String, value: String| {
            h_flex()
                .items_start()
                .gap_3()
                .text_sm()
                .line_height(px(22.))
                .child(div().w(px(72.)).flex_none().text_color(p.slate).child(label))
                .child(div().flex_1().min_w_0().text_color(p.ink).child(value))
        };
        let description = match image.display_description() {
            // 解説文は段落ごとに分ける
            Some(text) => v_flex()
                .gap_3()
                .children(text.split('\n').map(str::trim).filter(|l| !l.is_empty()).map(|paragraph| {
                    div().text_sm().line_height(px(24.)).text_color(p.ink).child(paragraph.to_string())
                }))
                .into_any_element(),
            None if dashboard.indexing => h_flex()
                .gap_2()
                .text_sm()
                .text_color(p.slate)
                .child(Spinner::new().small().color(p.slate))
                .child(tr!("info.loading"))
                .into_any_element(),
            None => div().text_sm().line_height(px(22.)).text_color(p.slate).child(tr!("info.none")).into_any_element(),
        };
        let copy = {
            let (dashboard, key) = (self.dashboard.clone(), key.clone());
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                dashboard.update(cx, |this, cx| this.copy_caption(&key, cx));
            }
        };
        v_flex()
            .size_full()
            .gap_5()
            .pb_4()
            .child(
                v_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .text_lg()
                            .line_height(px(28.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.ink)
                            .child(image.display_title()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .line_height(px(22.))
                            .text_color(p.slate)
                            .child(image.display_copyright().to_string()),
                    ),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(row(tr!("info.date"), bing::date_text(&image.startdate, true)))
                    .child(row(tr!("info.regions"), regions)),
            )
            .child(div().h(px(1.)).flex_none().bg(p.line))
            .child(description)
            .child(
                h_flex()
                    .gap_4()
                    .items_center()
                    .child(Button::new("copy-caption").small().outline().label(tr!("action.copy_caption")).on_click(copy))
                    // 外部のページなので、ボタンではなくリンクにする
                    .child(Link::new("open-bing").href(image.bing_link()).text_sm().child(tr!("action.open_bing"))),
            )
            .overflow_y_scrollbar()
            .into_any_element()
    }
}
