//! Media controls: the preview player a generated take or a library sound plays in.

use gpui::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, prelude::FluentBuilder, px, relative,
};

use crate::button::{Button, ButtonVariant, ControlSize};
use crate::icon::{Icon, IconName};
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;
use crate::viz::PeakBars;

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A compact player: play/pause, a scrubber, the time, and an optional volume.
///
/// Give it the clip's peaks and the scrubber becomes a waveform, which shows where the loud parts
/// are and is easier to aim at than a bare line.
#[derive(IntoElement)]
pub struct AudioPlayer {
    id: ElementId,
    playing: bool,
    position: f32,
    elapsed: SharedString,
    duration: SharedString,
    peaks: Option<Vec<f32>>,
    volume: Option<f32>,
    disabled: bool,
    on_play: Option<ClickHandler>,
}

impl AudioPlayer {
    /// A player at `position` (0 to 1) with the elapsed and total time already formatted.
    pub fn new(
        id: impl Into<ElementId>,
        position: f32,
        elapsed: impl Into<SharedString>,
        duration: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            playing: false,
            position: position.clamp(0., 1.),
            elapsed: elapsed.into(),
            duration: duration.into(),
            peaks: None,
            volume: None,
            disabled: false,
            on_play: None,
        }
    }

    /// Show Pause instead of Play.
    pub fn playing(mut self, playing: bool) -> Self {
        self.playing = playing;
        self
    }

    /// The clip's peaks, each from 0 to 1, for a waveform scrubber.
    pub fn peaks(mut self, peaks: Vec<f32>) -> Self {
        self.peaks = Some(peaks);
        self
    }

    /// Show a volume control at this level.
    pub fn volume(mut self, volume: f32) -> Self {
        self.volume = Some(volume.clamp(0., 1.));
        self
    }

    /// Nothing to play yet, for example while the take is still being made.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called on play/pause.
    pub fn on_play(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_play = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for AudioPlayer {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let height = theme.density.control_lg;
        let mut play = Button::icon_only(
            (self.id.clone(), "play"),
            if self.playing {
                IconName::Pause
            } else {
                IconName::Play
            },
        )
        .variant(ButtonVariant::Secondary)
        .size(ControlSize::Small)
        .disabled(self.disabled)
        .tooltip(if self.playing { "Pause" } else { "Play" });
        if let Some(handler) = self.on_play {
            play = play.on_click(handler);
        }
        let scrubber = match self.peaks {
            Some(peaks) => div()
                .flex_1()
                .h(height - theme.spacing.md)
                .child(PeakBars::new(peaks).progress(self.position))
                .into_any_element(),
            None => div()
                .flex_1()
                .relative()
                .h(px(4.))
                .rounded(px(2.))
                .bg(colors.border_strong)
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(relative(self.position))
                        .rounded(px(2.))
                        .bg(colors.accent),
                )
                .into_any_element(),
        };
        div()
            .flex()
            .items_center()
            .gap(theme.spacing.sm)
            .h(height)
            .px(theme.spacing.sm)
            .rounded(theme.control_radius(height).min(theme.shape.radius_md))
            .bg(colors.sunken)
            .when(self.disabled, |player| player.opacity(0.55))
            .child(play)
            .child(scrubber)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(theme.spacing.xxs)
                    .child(Label::new(self.elapsed).size(TextSize::Small).mono())
                    .child(
                        Label::new(format!("/ {}", self.duration))
                            .size(TextSize::Small)
                            .mono()
                            .color(TextColor::Faint),
                    ),
            )
            .children(self.volume.map(|volume| {
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(theme.spacing.xs)
                    .child(Icon::new(IconName::Waveform).size(px(13.)))
                    .child(
                        div()
                            .relative()
                            .w(px(48.))
                            .h(px(4.))
                            .rounded(px(2.))
                            .bg(colors.border_strong)
                            .child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .top_0()
                                    .bottom_0()
                                    .w(relative(volume))
                                    .rounded(px(2.))
                                    .bg(colors.text_muted),
                            ),
                    )
            }))
    }
}
