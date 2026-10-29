//! A welcome screen over a backdrop: liquid metal, or an ambient scene — aurora, silk dunes,
//! clouds of colour, caustics, a low sun, a galaxy, jellyfish or a shoal of fish — drifting
//! slowly behind a card that holds the words and the two ways to start. A bar below the card
//! picks the backdrop and its palette.
//!
//! The liquid metal can be played with: press on a drop and carry it, bring drops together to
//! fuse them into one (carry it too fast and it tears apart), and double-click to part them
//! again. So can the aurora's stars: pass over them to wake a constellation, press on one and
//! let go over another to draw a line between them, press on the open sky to send a shooting
//! star, and double-click to wipe the lines away.
//!
//! ```text
//! cargo run --example ambient -- [--backdrop metal|aurora|dunes|bloom|caustics|solar|galaxy|
//!     jellyfish|shoal]
//!     [--palette theme|dusk|ocean|ember|meadow|graphite]
//!     [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use atelier::{
    ActiveTheme, Ambience, AmbientBackdrop, Appearance, BackdropPalette, Button, Card, ControlSize,
    Direction, Label, MetalBackdrop, Segment, SegmentedControl, TextColor, TextSize, TitleBar,
    WindowFrame,
};
use common::Themed;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
    div, px,
};

/// What the welcome screen sits on.
#[derive(Clone, Copy, PartialEq)]
enum Backdrop {
    Metal,
    Ambient(Ambience),
}

impl Backdrop {
    /// Every backdrop, in the order the picker offers them.
    const ALL: [Backdrop; 9] = [
        Backdrop::Metal,
        Backdrop::Ambient(Ambience::Aurora),
        Backdrop::Ambient(Ambience::Dunes),
        Backdrop::Ambient(Ambience::Bloom),
        Backdrop::Ambient(Ambience::Caustics),
        Backdrop::Ambient(Ambience::Solar),
        Backdrop::Ambient(Ambience::Galaxy),
        Backdrop::Ambient(Ambience::Jellyfish),
        Backdrop::Ambient(Ambience::Shoal),
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

    fn element(self, palette: BackdropPalette) -> AnyElement {
        match self {
            Backdrop::Metal => MetalBackdrop::new("metal")
                .palette(palette)
                .into_any_element(),
            Backdrop::Ambient(ambience) => AmbientBackdrop::new("ambient", ambience)
                .palette(palette)
                .into_any_element(),
        }
    }
}

struct Welcome {
    choice: (Direction, Appearance),
    backdrop: Backdrop,
    palette: BackdropPalette,
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
        let index = BackdropPalette::ALL
            .iter()
            .position(|palette| *palette == self.palette)
            .unwrap_or(0);
        // Backdrops can take clicks, so the bar takes its own.
        let pickers = div()
            .occlude()
            .flex()
            .flex_col()
            .items_center()
            .gap(theme.spacing.xs)
            .p(theme.spacing.xs)
            .rounded(theme.shape.radius_lg)
            .bg(theme.colors.overlay)
            .shadow(theme.elevation.medium.clone())
            .child(backdrops)
            .child(
                SegmentedControl::new(
                    "palette",
                    BackdropPalette::ALL.map(|palette| Segment::label(palette.label())),
                    index,
                )
                .size(ControlSize::Small)
                .on_select(cx.listener(|this, index: &usize, _, cx| {
                    this.palette = BackdropPalette::ALL[*index];
                    cx.notify();
                })),
            );
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
            .child(TitleBar::new().title("Ambient").trailing(switch))
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
                            .child(self.backdrop.element(self.palette)),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            // The card takes its own clicks; the backdrop around it takes the
                            // rest.
                            .child(div().occlude().child(card)),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom(theme.spacing.xl)
                            .left_0()
                            .right_0()
                            .flex()
                            .justify_center()
                            .child(pickers),
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
    let palette = common::argument("--palette")
        .and_then(|name| {
            BackdropPalette::ALL
                .into_iter()
                .find(|palette| palette.label().eq_ignore_ascii_case(&name))
        })
        .unwrap_or_default();
    common::run("Ambient", 1100., 680., move |choice, _| Welcome {
        choice,
        backdrop,
        palette,
    });
}
