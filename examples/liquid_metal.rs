//! A welcome screen over a liquid-metal backdrop: pools of liquid chrome flowing slowly behind a
//! card that holds the words and the two ways to start.
//!
//! ```text
//! cargo run --example liquid_metal -- [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use atelier::{
    ActiveTheme, Appearance, Button, Card, ControlSize, Direction, Label, MetalBackdrop, TextColor,
    TextSize, TitleBar, WindowFrame,
};
use common::Themed;
use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px};

struct Welcome {
    choice: (Direction, Appearance),
}

impl Themed for Welcome {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

impl Render for Welcome {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let switch = common::theme_switch(self, cx);
        // The words sit on a card above the metal, never on it.
        let card = Card::new()
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
                    .gap(theme.spacing.sm)
                    .child(Button::new("open", "開く…").size(ControlSize::Large))
                    .child(
                        Button::new("new", "新しいプロジェクト")
                            .primary()
                            .size(ControlSize::Large),
                    ),
            );
        WindowFrame::new()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .size_full()
            .flex()
            .flex_col()
            .child(
                TitleBar::new()
                    .title("Liquid metal")
                    .subtitle("ウェルカム画面の背景")
                    .trailing(switch),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .overflow_hidden()
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
                            .child(card),
                    ),
            )
    }
}

fn main() {
    common::run("Liquid metal", 1100., 680., |choice, _| Welcome { choice });
}
