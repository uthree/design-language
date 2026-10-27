//! Liquid metal as ornament: a splash hero, a finished-export moment and a plan mark, with the
//! rules for where it does not go. The title bar compares mercury with rigid chrome.
//!
//! ```text
//! cargo run --example liquid_metal -- [--finish liquid|chrome]
//!                                      [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use atelier::{
    ActiveTheme, Appearance, Button, Card, ControlSize, Direction, Divider, Label, LiquidMetal,
    MetalShape, SectionHeader, Segment, SegmentedControl, TextColor, TextSize, TitleBar,
    WindowFrame,
};
use common::Themed;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Render, Styled, Window, div, px};

struct LiquidMetalDemo {
    choice: (Direction, Appearance),
    /// 1 for mercury, 0 for rigid chrome.
    liquid: f32,
}

impl Themed for LiquidMetalDemo {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

impl LiquidMetalDemo {
    /// The splash: the one large piece of metal in the window, with the words beside it, not on it.
    fn hero(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        Card::new()
            .flex_1()
            .min_w_0()
            .items_center()
            .justify_center()
            .gap(theme.spacing.xl)
            .p(theme.spacing.xxl)
            .child(
                div()
                    .w(px(320.))
                    .h(px(210.))
                    .child(LiquidMetal::new("hero").liquid(self.liquid)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(theme.spacing.xs)
                    .child(Label::new("Auris Studio").size(TextSize::Display))
                    .child(
                        Label::new("思いついた音を、そのまま曲に。")
                            .size(TextSize::Large)
                            .color(TextColor::Muted),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(theme.spacing.sm)
                    .child(Button::new("open", "開く…"))
                    .child(Button::new("new", "新しいプロジェクト").primary()),
            )
            .into_any_element()
    }

    fn uses(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let row = |metal: AnyElement, title: &'static str, detail: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(theme.spacing.md)
                .child(metal)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .flex_1()
                        .gap(theme.spacing.xxs)
                        .child(Label::new(title))
                        .child(
                            Label::new(detail)
                                .size(TextSize::Small)
                                .color(TextColor::Muted),
                        ),
                )
        };
        let rule = |text: &'static str| {
            Label::new(text)
                .size(TextSize::Small)
                .color(TextColor::Muted)
        };
        div()
            .w(px(380.))
            .flex_none()
            .flex()
            .flex_col()
            .gap(theme.spacing.lg)
            .child(SectionHeader::caption("ほかの使いどころ"))
            .child(
                Card::new()
                    .p(theme.spacing.lg)
                    .gap(theme.spacing.lg)
                    .child(
                        row(
                            div()
                                .w(px(46.))
                                .h(px(34.))
                                .flex_none()
                                .child(LiquidMetal::new("done").liquid(self.liquid * 0.6))
                                .into_any_element(),
                            "書き出しが完了しました",
                            "Night Drive.wav · 3:42 · 48 kHz",
                        )
                        .child(Button::new("reveal", "表示").size(ControlSize::Small)),
                    )
                    .child(Divider::horizontal())
                    .child(row(
                        div()
                            .w(px(34.))
                            .h(px(14.))
                            .flex_none()
                            .child(
                                LiquidMetal::new("plan")
                                    .shape(MetalShape::Capsule)
                                    .liquid(0.),
                            )
                            .into_any_element(),
                        "Auris Pro",
                        "高品質な書き出しと、長い生成が使えます。",
                    )),
            )
            .child(SectionHeader::caption("使わないところ"))
            .child(
                Card::new()
                    .p(theme.spacing.lg)
                    .gap(theme.spacing.sm)
                    .child(rule("文字を金属の上に置かない。反射の明暗で読めなくなる。"))
                    .child(rule("状態・選択・AI の色にしない。金属は無彩色の飾り。"))
                    .child(rule("1 画面に 1 か所まで。並べると安っぽくなる。"))
                    .child(rule(
                        "編集中ずっと見える場所では流さない。止めるか、置かない。",
                    ))
                    .child(rule("視差効果を減らす設定では動かさない。")),
            )
            .into_any_element()
    }
}

impl Render for LiquidMetalDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let switch = common::theme_switch(self, cx);
        let hero = self.hero(cx);
        let uses = self.uses(cx);
        WindowFrame::new()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .size_full()
            .flex()
            .flex_col()
            .child(
                TitleBar::new()
                    .title("Liquid metal")
                    .subtitle("装飾用の素材")
                    .trailing(
                        SegmentedControl::new(
                            "finish",
                            [Segment::label("液体"), Segment::label("鏡面")],
                            if self.liquid > 0.5 { 0 } else { 1 },
                        )
                        .size(ControlSize::Small)
                        .on_select(cx.listener(
                            |this, index: &usize, _, cx| {
                                this.liquid = if *index == 0 { 1. } else { 0. };
                                cx.notify();
                            },
                        )),
                    )
                    .trailing(Divider::vertical())
                    .trailing(switch),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap(theme.spacing.lg)
                    .p(theme.spacing.lg)
                    .bg(theme.colors.background)
                    .child(hero)
                    .child(uses),
            )
    }
}

fn main() {
    let liquid = match common::argument("--finish").as_deref() {
        Some("chrome") => 0.,
        _ => 1.,
    };
    common::run("Liquid metal", 1100., 680., move |choice, _| {
        LiquidMetalDemo { choice, liquid }
    });
}
