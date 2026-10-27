//! A welcome screen over a backdrop: liquid metal, or an ambient scene — aurora, silk dunes,
//! clouds of colour or caustics — drifting slowly behind a card that holds the words and the two
//! ways to start. The title bar picks the backdrop.
//!
//! ```text
//! cargo run --example ambient -- [--backdrop metal|aurora|dunes|bloom|caustics]
//!     [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use atelier::{
    ActiveTheme, Ambience, AmbientBackdrop, Appearance, Button, Card, ControlSize, Direction,
    Label, MetalBackdrop, Segment, SegmentedControl, TextColor, TextSize, TitleBar, WindowFrame,
};
use common::Themed;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Render, Styled, Window, div, px};

/// What the welcome screen sits on.
#[derive(Clone, Copy, PartialEq)]
enum Backdrop {
    Metal,
    Ambient(Ambience),
}

impl Backdrop {
    /// Every backdrop, in the order the title bar offers them.
    const ALL: [Backdrop; 5] = [
        Backdrop::Metal,
        Backdrop::Ambient(Ambience::Aurora),
        Backdrop::Ambient(Ambience::Dunes),
        Backdrop::Ambient(Ambience::Bloom),
        Backdrop::Ambient(Ambience::Caustics),
    ];

    fn label(self) -> &'static str {
        match self {
            Backdrop::Metal => "Liquid metal",
            Backdrop::Ambient(ambience) => ambience.label(),
        }
    }

    /// The name `--backdrop` takes.
    fn id(self) -> &'static str {
        match self {
            Backdrop::Metal => "metal",
            Backdrop::Ambient(ambience) => ambience.label(),
        }
    }

    fn element(self) -> AnyElement {
        match self {
            Backdrop::Metal => MetalBackdrop::new("metal").into_any_element(),
            Backdrop::Ambient(ambience) => {
                AmbientBackdrop::new("ambient", ambience).into_any_element()
            }
        }
    }
}

struct Welcome {
    choice: (Direction, Appearance),
    backdrop: Backdrop,
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
        let index = Backdrop::ALL
            .iter()
            .position(|backdrop| *backdrop == self.backdrop)
            .unwrap_or(0);
        let backdrops = SegmentedControl::new(
            "backdrop",
            Backdrop::ALL.map(|backdrop| Segment::label(backdrop.label())),
            index,
        )
        .size(ControlSize::Small)
        .on_select(cx.listener(|this, index: &usize, _, cx| {
            this.backdrop = Backdrop::ALL[*index];
            cx.notify();
        }));
        // The words sit on a card above the backdrop, never on it.
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
                TitleBar::new().title("Ambient").trailing(
                    div()
                        .flex()
                        .gap(theme.spacing.sm)
                        .child(backdrops)
                        .child(switch),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .overflow_hidden()
                    .child(div().absolute().inset_0().child(self.backdrop.element()))
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
    let backdrop = common::argument("--backdrop")
        .and_then(|name| {
            Backdrop::ALL
                .into_iter()
                .find(|backdrop| backdrop.id().eq_ignore_ascii_case(&name))
        })
        .unwrap_or(Backdrop::Metal);
    common::run("Ambient", 1100., 680., move |choice, _| Welcome {
        choice,
        backdrop,
    });
}
