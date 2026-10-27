//! A welcome screen over an ambient backdrop: aurora, silk dunes, clouds of colour or caustics
//! drifting slowly behind a card that holds the words and the two ways to start. The title bar
//! picks the scene.
//!
//! ```text
//! cargo run --example ambient -- [--ambience aurora|dunes|bloom|caustics]
//!     [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use atelier::{
    ActiveTheme, Ambience, AmbientBackdrop, Appearance, Button, Card, ControlSize, Direction,
    Label, Segment, SegmentedControl, TextColor, TextSize, TitleBar, WindowFrame,
};
use common::Themed;
use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px};

struct Welcome {
    choice: (Direction, Appearance),
    ambience: Ambience,
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
        let index = Ambience::ALL
            .iter()
            .position(|ambience| *ambience == self.ambience)
            .unwrap_or(0);
        let scenes = SegmentedControl::new(
            "ambience",
            Ambience::ALL.map(|ambience| Segment::label(ambience.label())),
            index,
        )
        .size(ControlSize::Small)
        .on_select(cx.listener(|this, index: &usize, _, cx| {
            this.ambience = Ambience::ALL[*index];
            cx.notify();
        }));
        // The words sit on a card above the scene, never on it.
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
                    .title("Ambient")
                    .subtitle("ウェルカム画面の背景")
                    .trailing(
                        div()
                            .flex()
                            .gap(theme.spacing.sm)
                            .child(scenes)
                            .child(switch),
                    ),
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
                            .child(AmbientBackdrop::new("backdrop", self.ambience)),
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
    let ambience = common::argument("--ambience")
        .and_then(|name| {
            Ambience::ALL
                .into_iter()
                .find(|ambience| ambience.label().eq_ignore_ascii_case(&name))
        })
        .unwrap_or(Ambience::Aurora);
    common::run("Ambient", 1100., 680., move |choice, _| Welcome {
        choice,
        ambience,
    });
}
