//! Liquid metal as ornament: a welcome screen over a drifting metal backdrop, and metal rings on
//! the controls that deserve one — the call to action, the selected tool, an add button.
//!
//! ```text
//! cargo run --example liquid_metal -- [--screen welcome|parts]
//!                                      [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use atelier::{
    ActiveTheme, Appearance, Button, Card, ControlSize, Direction, Divider, IconName, Label,
    LiquidMetal, MetalBackdrop, MetalFrame, SectionHeader, Segment, SegmentedControl, TextColor,
    TextSize, TitleBar, WindowFrame,
};
use common::Themed;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Render, Styled, Window, div, px, relative,
};

struct LiquidMetalDemo {
    choice: (Direction, Appearance),
    /// 0 the welcome screen, 1 the parts.
    screen: usize,
    tool: usize,
}

impl Themed for LiquidMetalDemo {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

impl LiquidMetalDemo {
    /// The welcome screen: metal drifts behind, the words sit on a card above it.
    fn welcome(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        div()
            .flex_1()
            .min_h_0()
            .relative()
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .child(MetalBackdrop::new("backdrop")),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Card::new()
                            .w(px(440.))
                            .items_center()
                            .gap(theme.spacing.xl)
                            .p(theme.spacing.xxl)
                            .bg(theme.colors.overlay)
                            .shadow(theme.elevation.high.clone())
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
                                    .items_center()
                                    .gap(theme.spacing.md)
                                    .child(Button::new("open", "開く…").size(ControlSize::Large))
                                    .child(
                                        MetalFrame::new("new-frame")
                                            .fill(theme.colors.overlay)
                                            .child(
                                                Button::new("new", "新しいプロジェクト")
                                                    .primary()
                                                    .size(ControlSize::Large),
                                            ),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// Metal on controls, and the rules for where it does not go.
    fn parts(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let tools = [
            (IconName::Sliders, "ミキサー"),
            (IconName::Waveform, "波形"),
            (IconName::Music, "楽譜"),
            (IconName::Folder, "素材"),
        ];
        let palette = div()
            .flex()
            .items_center()
            .gap(theme.spacing.sm)
            .p(theme.spacing.sm)
            .rounded(theme.shape.radius_lg + px(4.))
            .bg(theme.colors.elevated)
            .border_1()
            .border_color(theme.colors.border)
            .children(tools.iter().enumerate().map(|(index, (icon, name))| {
                let button = Button::icon_only(("tool", index), *icon)
                    .size(ControlSize::Large)
                    .selected(self.tool == index)
                    .tooltip(*name)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tool = index;
                        cx.notify();
                    }));
                if self.tool == index {
                    MetalFrame::new(("tool-frame", index))
                        .fill(theme.colors.elevated)
                        .radius(theme.shape.radius_md + px(4.))
                        .child(button)
                        .into_any_element()
                } else {
                    div().p(px(4.)).child(button).into_any_element()
                }
            }));
        let add = div()
            .flex()
            .items_center()
            .gap(theme.spacing.md)
            .child(
                MetalFrame::new("add-frame")
                    .width(px(5.))
                    .radius(px(99.))
                    .fill(theme.colors.elevated)
                    .child(
                        Button::icon_only("add", IconName::Plus)
                            .size(ControlSize::Large)
                            .tooltip("トラックを追加"),
                    ),
            )
            .child(
                MetalFrame::new("upload-frame")
                    .width(px(2.))
                    .child(Button::new("upload", "書き出す…").icon(IconName::ArrowUp)),
            );
        let pro = div()
            .flex()
            .items_center()
            .gap(theme.spacing.md)
            .child(
                div()
                    .size(px(30.))
                    .flex_none()
                    .child(LiquidMetal::new("pro-mark")),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.xxs)
                    .child(Label::new("Auris Pro"))
                    .child(
                        Label::new("高品質な書き出しと、長い生成が使えます。")
                            .size(TextSize::Small)
                            .color(TextColor::Muted),
                    ),
            );
        let rule = |text: &'static str| {
            Label::new(text)
                .size(TextSize::Small)
                .color(TextColor::Muted)
        };
        let section = |title: &'static str, body: AnyElement| {
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.md)
                .child(SectionHeader::caption(title))
                .child(body)
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .gap(theme.spacing.xxl)
            .p(theme.spacing.xxl)
            .bg(theme.colors.background)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.xxl)
                    .w(relative(0.55))
                    .child(section("選択中のツール", palette.into_any_element()))
                    .child(section("主役のボタン", add.into_any_element()))
                    .child(section("プランの印", pro.into_any_element())),
            )
            .child(
                div().flex_1().min_w_0().child(section(
                    "使わないところ",
                    Card::new()
                        .p(theme.spacing.lg)
                        .gap(theme.spacing.sm)
                        .child(rule("文字を金属の上に置かない。反射の明暗で読めなくなる。"))
                        .child(rule(
                            "状態や選択の意味を金属だけに持たせない。選択は塗りでも示す。",
                        ))
                        .child(rule(
                            "1 画面に 1 つの用途まで。枠を並べすぎると主役が消える。",
                        ))
                        .child(rule(
                            "編集中ずっと見える場所では流さない。.phase(..) で止める。",
                        ))
                        .child(rule("視差効果を減らす設定では動かさない。"))
                        .into_any_element(),
                )),
            )
            .into_any_element()
    }
}

impl Render for LiquidMetalDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let switch = common::theme_switch(self, cx);
        let body = if self.screen == 0 {
            self.welcome(cx)
        } else {
            self.parts(cx)
        };
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
                            "screen",
                            [Segment::label("ようこそ"), Segment::label("部品")],
                            self.screen,
                        )
                        .size(ControlSize::Small)
                        .on_select(cx.listener(
                            |this, index: &usize, _, cx| {
                                this.screen = *index;
                                cx.notify();
                            },
                        )),
                    )
                    .trailing(Divider::vertical())
                    .trailing(switch),
            )
            .child(body)
    }
}

fn main() {
    let screen = match common::argument("--screen").as_deref() {
        Some("parts") => 1,
        _ => 0,
    };
    common::run("Liquid metal", 1100., 680., move |choice, _| {
        LiquidMetalDemo {
            choice,
            screen,
            tool: 1,
        }
    });
}
