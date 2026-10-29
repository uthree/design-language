//! Media controls: the preview player a generated take or a library sound plays in.

use std::rc::Rc;

use gpui::{
    App, Bounds, ClickEvent, Div, ElementId, Entity, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, RenderOnce, SharedString,
    Stateful, Styled, Window, canvas, div, prelude::FluentBuilder, px, relative,
};

use crate::button::{Button, ButtonVariant, ControlSize};
use crate::display::Spinner;
use crate::focus::{
    CONTROL_CONTEXT, StepBackward, StepForward, StepToEnd, StepToStart, TRACK_CONTEXT, focusable,
};
use crate::icon::{Icon, IconName};
use crate::strings::ActiveStrings;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;
use crate::viz::PeakBars;

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type ValueHandler = Rc<dyn Fn(&f32, &mut Window, &mut App) + 'static>;

/// How far an arrow key moves the scrubber when the player is not told: a twentieth of the
/// clip. Pass [`AudioPlayer::seek_step`] five seconds' worth instead when the length is known.
const SEEK_STEP: f32 = 0.05;
/// How far an arrow key moves the volume.
const VOLUME_STEP: f32 = 0.05;

/// A compact player: play/pause, a scrubber, the time, and an optional volume and mute.
///
/// Give it the clip's peaks and the scrubber becomes a waveform, which shows where the loud parts
/// are and is easier to aim at than a bare line. With [`AudioPlayer::on_seek`] the scrubber
/// follows a click or a drag and, once Tab has reached it, the arrow keys (one
/// [`AudioPlayer::seek_step`]), Home and End; the volume does the same with
/// [`AudioPlayer::on_volume`]. The player only reports where the person asked to go: the position
/// belongs to the playback clock, so the application seeks and draws the next position itself.
///
/// The length may change while it plays — a draft that is still being written — since the
/// player is drawn afresh from the position and the times each frame.
#[derive(IntoElement)]
pub struct AudioPlayer {
    id: ElementId,
    playing: bool,
    position: f32,
    elapsed: SharedString,
    duration: SharedString,
    peaks: Option<Vec<f32>>,
    volume: Option<f32>,
    muted: bool,
    loading: bool,
    disabled: bool,
    seek_step: f32,
    on_play: Option<ClickHandler>,
    on_seek: Option<ValueHandler>,
    on_volume: Option<ValueHandler>,
    on_mute: Option<ClickHandler>,
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
            muted: false,
            loading: false,
            disabled: false,
            seek_step: SEEK_STEP,
            on_play: None,
            on_seek: None,
            on_volume: None,
            on_mute: None,
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

    /// Muted: the volume control greys out and its button shows the muted speaker.
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// The audio is still loading or decoding: a spinner takes the play button's place and
    /// the scrubber waits.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Nothing to play yet, for example while the take is still being made.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// How far, from 0 to 1, an arrow key moves the scrubber. Five seconds is the usual step:
    /// pass `5. / length_in_seconds`. A twentieth of the clip by default.
    pub fn seek_step(mut self, step: f32) -> Self {
        self.seek_step = step.clamp(0.001, 1.);
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

    /// Called with the position (0 to 1) the person asked for, by pointer or keyboard. Makes
    /// the scrubber a tab stop.
    pub fn on_seek(mut self, handler: impl Fn(&f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_seek = Some(Rc::new(handler));
        self
    }

    /// Called with the volume (0 to 1) the person asked for. Makes the volume a tab stop.
    pub fn on_volume(mut self, handler: impl Fn(&f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_volume = Some(Rc::new(handler));
        self
    }

    /// Called on the mute button, which shows beside the volume once this is set.
    pub fn on_mute(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_mute = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for AudioPlayer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let colors = &theme.colors;
        let height = theme.density.control_lg;
        let waiting = self.disabled || self.loading;
        let play = if self.loading {
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(theme.density.control_sm)
                .child(Spinner::new((self.id.clone(), "loading")).size(px(14.)))
                .into_any_element()
        } else {
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
            .tooltip(if self.playing {
                cx.strings().pause.clone()
            } else {
                cx.strings().play.clone()
            });
            if let Some(handler) = self.on_play {
                play = play.on_click(handler);
            }
            play.into_any_element()
        };
        let seekable = self.on_seek.is_some() && !waiting;
        let thumb = px(10.);
        let scrubber = track(
            (self.id.clone(), "seek").into(),
            self.position,
            self.seek_step,
            self.on_seek.filter(|_| !waiting),
            window,
            cx,
        )
        .flex_1()
        .min_w_0()
        .h(height - theme.spacing.md)
        .flex()
        .items_center()
        .child(match self.peaks {
            Some(peaks) => div()
                .size_full()
                .child(PeakBars::new(peaks).progress(self.position))
                .into_any_element(),
            None => div()
                .relative()
                .w_full()
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
                // A thumb says the line can be dragged.
                .when(seekable, |line| {
                    line.child(
                        div()
                            .absolute()
                            .left(relative(self.position))
                            .ml(-thumb * self.position)
                            .top(px(2.) - thumb / 2.)
                            .size(thumb)
                            .rounded(thumb / 2.)
                            .bg(colors.accent),
                    )
                })
                .into_any_element(),
        });
        let muted = self.muted;
        let volume = self.volume.map(|volume| {
            let mute = match self.on_mute {
                Some(handler) => Button::icon_only(
                    (self.id.clone(), "mute"),
                    if muted {
                        IconName::VolumeOff
                    } else {
                        IconName::Volume
                    },
                )
                .size(ControlSize::Small)
                .selected(muted)
                .tooltip(if muted {
                    cx.strings().unmute.clone()
                } else {
                    cx.strings().mute.clone()
                })
                .on_click(handler)
                .into_any_element(),
                None => Icon::new(if muted {
                    IconName::VolumeOff
                } else {
                    IconName::Volume
                })
                .size(px(13.))
                .into_any_element(),
            };
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(theme.spacing.xs)
                .child(mute)
                .child(
                    track(
                        (self.id.clone(), "volume").into(),
                        volume,
                        VOLUME_STEP,
                        self.on_volume,
                        window,
                        cx,
                    )
                    .w(px(56.))
                    .h(px(14.))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .relative()
                            .w_full()
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
                                    .bg(if muted {
                                        colors.text_disabled
                                    } else {
                                        colors.text_muted
                                    }),
                            ),
                    ),
                )
        });
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
            .children(volume)
    }
}

/// Where a track was last drawn, and whether the pointer is dragging along it.
#[derive(Default)]
struct Track {
    bounds: Bounds<Pixels>,
    dragging: bool,
}

/// A horizontal track that reports a value from 0 to 1: where a click or drag lands, and the
/// arrow keys, Home and End once Tab has reached it. Without a handler it is only drawn.
fn track(
    id: ElementId,
    value: f32,
    step: f32,
    on_change: Option<ValueHandler>,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let radius = cx.theme().shape.radius_xs;
    let element = focusable(
        div().id(id.clone()),
        &id,
        on_change.is_none(),
        radius,
        window,
        cx,
    )
    .relative();
    let Some(on_change) = on_change else {
        return element;
    };
    let state = window.use_keyed_state(
        ElementId::NamedChild(Box::new(id), "track".into()),
        cx,
        |_, _| Track::default(),
    );
    let dragging = state.read(cx).dragging;
    let at = |state: &Entity<Track>, x: Pixels, cx: &App| {
        let bounds = state.read(cx).bounds;
        (f32::from(x - bounds.left()) / f32::from(bounds.size.width).max(1.)).clamp(0., 1.)
    };
    let step_to = |to: f32| {
        let on_change = on_change.clone();
        move |window: &mut Window, cx: &mut App| on_change(&to.clamp(0., 1.), window, cx)
    };
    let (back, forward, start, end) = (
        step_to(value - step),
        step_to(value + step),
        step_to(0.),
        step_to(1.),
    );
    element
        .key_context(format!("{CONTROL_CONTEXT} {TRACK_CONTEXT}").as_str())
        .on_action(move |_: &StepBackward, window, cx| back(window, cx))
        .on_action(move |_: &StepForward, window, cx| forward(window, cx))
        .on_action(move |_: &StepToStart, window, cx| start(window, cx))
        .on_action(move |_: &StepToEnd, window, cx| end(window, cx))
        .on_mouse_down(MouseButton::Left, {
            let (state, on_change) = (state.clone(), on_change.clone());
            move |event: &MouseDownEvent, window, cx| {
                state.update(cx, |track, cx| {
                    track.dragging = true;
                    cx.notify();
                });
                on_change(&at(&state, event.position.x, cx), window, cx);
            }
        })
        .child(
            canvas(
                {
                    let state = state.clone();
                    move |bounds, _, cx| state.update(cx, |track, _| track.bounds = bounds)
                },
                move |_, _, window, _| {
                    if !dragging {
                        return;
                    }
                    // Follow the pointer anywhere in the window until the button comes up.
                    window.on_mouse_event({
                        let state = state.clone();
                        move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase.bubble() && event.pressed_button == Some(MouseButton::Left) {
                                on_change(&at(&state, event.position.x, cx), window, cx);
                            }
                        }
                    });
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                        if phase.bubble() {
                            state.update(cx, |track, cx| {
                                track.dragging = false;
                                cx.notify();
                            });
                        }
                    });
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
}
