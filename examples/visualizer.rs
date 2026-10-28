//! A mock master-bus analyser: spectrum, spectrogram, oscilloscope, vectorscope and loudness.
//! The signal is synthesised from a few sines and some deterministic noise; nothing is audio.
//!
//! ```text
//! cargo run --example visualizer -- [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use std::f32::consts::TAU;
use std::time::Duration;

use atelier::viz::{Heatmap, Spectrum, Vectorscope, Waveform};
use atelier::{
    ActiveTheme, Appearance, Badge, Button, ControlSize, Direction, Divider, IconName, Label,
    LevelMeter, SectionHeader, Segment, SegmentedControl, TextColor, TextSize, TitleBar, Tone,
    WindowFrame,
};
use common::Themed;
use gpui::{
    Animation, AnimationExt, App, Context, IntoElement, ParentElement, Render, RenderOnce,
    SharedString, Styled, Window, div, prelude::*, px, relative,
};

/// One loop of the synthetic programme.
const LOOP: Duration = Duration::from_secs(8);
const BANDS: usize = 72;
const HISTORY: usize = 120;
const SPECTROGRAM_ROWS: usize = 56;

/// Deterministic noise in 0..1, so every frame of the loop is reproducible.
fn noise(seed: f32) -> f32 {
    let x = (seed * 12.9898).sin() * 43_758.547;
    x - x.floor()
}

/// The programme's energy at band position `x` (0 low, 1 high) and loop time `t` (0 to 1): a
/// kick every quarter, a bass line, a chord pad and hats, under a pink-ish floor.
fn energy(x: f32, t: f32) -> f32 {
    let beat = (t * 16.).fract();
    let kick = (-beat * 9.).exp() * (-((x - 0.08) / 0.05).powi(2)).exp();
    let bass_note = [0.16, 0.16, 0.19, 0.14][(t * 4.) as usize % 4];
    let bass = 0.75 * (-((x - bass_note) / 0.025).powi(2)).exp();
    let pad: f32 = [0.36, 0.42, 0.47, 0.55]
        .iter()
        .map(|at| 0.45 * (-((x - at) / 0.018).powi(2)).exp())
        .sum::<f32>()
        * (0.75 + 0.25 * (TAU * t * 2.).sin());
    let hat_beat = (t * 32. + 0.5).fract();
    let hats = (-hat_beat * 14.).exp() * 0.5 * ((x - 0.78) / 0.12).clamp(0., 1.).min(1.);
    let floor = 0.42 * (1. - x).powf(0.7) + 0.05;
    let jitter = 0.06 * noise(x * 97. + (t * 240.).floor());
    (floor + kick + bass + pad + hats + jitter).clamp(0., 1.)
}

fn spectrum_at(t: f32) -> Vec<f32> {
    (0..BANDS)
        .map(|band| energy(band as f32 / (BANDS - 1) as f32, t))
        .collect()
}

/// Peak holds: the highest level over the last quarter second of the loop.
fn peaks_at(t: f32) -> Vec<f32> {
    (0..BANDS)
        .map(|band| {
            let x = band as f32 / (BANDS - 1) as f32;
            (0..8)
                .map(|step| energy(x, (t - step as f32 * 0.004).rem_euclid(1.)))
                .fold(0., f32::max)
        })
        .collect()
}

/// The spectrogram: `HISTORY` columns of past spectra, newest on the right, high bands on top.
fn spectrogram_at(t: f32) -> Vec<f32> {
    let mut values = vec![0.; HISTORY * SPECTROGRAM_ROWS];
    for column in 0..HISTORY {
        let time = (t - (HISTORY - 1 - column) as f32 * 0.003).rem_euclid(1.);
        for row in 0..SPECTROGRAM_ROWS {
            let x = 1. - row as f32 / (SPECTROGRAM_ROWS - 1) as f32;
            values[row * HISTORY + column] = (energy(x, time) - 0.1).max(0.) * 1.15;
        }
    }
    values
}

/// Stereo samples: a bass and a chord, with the chord spread slightly wider on the right.
fn stereo_at(t: f32, count: usize, span: f32) -> Vec<(f32, f32)> {
    (0..count)
        .map(|index| {
            let s = t * 40. + index as f32 / count as f32 * span;
            let bass = 0.45 * (TAU * s * 1.).sin();
            let chord = 0.22 * (TAU * s * 5.).sin() + 0.14 * (TAU * s * 7.5).sin();
            let noise_l = 0.05 * (noise(s * 13.) - 0.5);
            let noise_r = 0.05 * (noise(s * 17. + 3.) - 0.5);
            (
                bass + chord + noise_l,
                bass + 0.8 * chord + 0.18 * (TAU * s * 5. + 0.6).sin() + noise_r,
            )
        })
        .collect()
}

/// A number that animates: a loudness readout.
#[derive(IntoElement)]
struct Readout {
    value: f32,
    unit: &'static str,
    label: &'static str,
    large: bool,
}

impl Readout {
    fn value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }
}

impl RenderOnce for Readout {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .gap(theme.spacing.xxs)
            .child(Label::caption(self.label))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(theme.spacing.xs)
                    .child(
                        Label::new(SharedString::from(format!("{:.1}", self.value)))
                            .size(if self.large {
                                TextSize::Display
                            } else {
                                TextSize::XLarge
                            })
                            .mono()
                            .weight(theme.typography.label_weight),
                    )
                    .child(
                        Label::new(self.unit)
                            .size(TextSize::Small)
                            .color(TextColor::Muted),
                    ),
            )
    }
}

struct Visualizer {
    choice: (Direction, Appearance),
    tap: usize,
}

impl Themed for Visualizer {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

impl Visualizer {
    fn panel(title: &'static str, description: &'static str, cx: &App) -> gpui::Div {
        let theme = cx.theme();
        // The card surface as a plain div, so callers can keep chaining any children.
        div()
            .flex()
            .flex_col()
            .gap(theme.spacing.md)
            .p(theme.spacing.lg)
            .rounded(theme.shape.radius_md)
            .bg(theme.colors.elevated)
            .border_1()
            .border_color(theme.colors.border)
            .child(SectionHeader::new(title).description(description))
    }

    fn plot(cx: &App) -> gpui::Div {
        let theme = cx.theme();
        div()
            .rounded(theme.shape.radius_sm)
            .bg(theme.colors.sunken)
            .overflow_hidden()
    }

    fn spectrum(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let frequencies = [
            "20", "50", "100", "200", "500", "1k", "2k", "5k", "10k", "20k",
        ];
        let axis = div()
            .flex()
            .justify_between()
            .px(theme.spacing.xs)
            .children(frequencies.into_iter().map(|label| {
                Label::new(label)
                    .size(TextSize::XSmall)
                    .mono()
                    .color(TextColor::Faint)
            }));
        let scale = div()
            .flex()
            .flex_col()
            .justify_between()
            .h_full()
            .py(theme.spacing.xs)
            .children(
                ["0", "−12", "−24", "−36", "−48", "−60"]
                    .into_iter()
                    .map(|label| {
                        Label::new(label)
                            .size(TextSize::XSmall)
                            .mono()
                            .color(TextColor::Faint)
                    }),
            );
        Self::panel("Spectrum", "1/6-octave bands with peak hold", cx)
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .flex_1()
                    .gap(theme.spacing.sm)
                    .min_h(px(200.))
                    .child(scale)
                    .child(
                        Self::plot(cx).flex_1().h_full().p(theme.spacing.xs).child(
                            Spectrum::new(spectrum_at(0.))
                                .peaks(peaks_at(0.))
                                .with_animation(
                                    "spectrum",
                                    Animation::new(LOOP).repeat(),
                                    |spectrum, t| {
                                        spectrum.levels(spectrum_at(t)).peaks(peaks_at(t))
                                    },
                                ),
                        ),
                    ),
            )
            .child(div().pl(px(28.)).child(axis))
    }

    fn loudness(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let loudness = |t: f32| -14.2 + 2.4 * (TAU * t * 2.).sin() + 1.2 * (energy(0.1, t) - 0.5);
        let meter = |id: &'static str, offset: f32| {
            div().h_full().child(
                LevelMeter::new(0.7)
                    .peak(0.86)
                    .thickness(px(8.))
                    .with_animation(id, Animation::new(LOOP).repeat(), move |meter, t| {
                        let level = 0.62 + 0.18 * energy(0.1, (t + offset).rem_euclid(1.));
                        meter.level(level)
                    }),
            )
        };
        Self::panel("Loudness", "EBU R128", cx)
            .w(px(300.))
            .flex_none()
            .child(
                div()
                    .flex()
                    .flex_1()
                    .gap(theme.spacing.lg)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(theme.spacing.md)
                            .child(
                                Readout {
                                    value: -14.,
                                    unit: "LUFS",
                                    label: "Momentary",
                                    large: true,
                                }
                                .with_animation(
                                    "momentary",
                                    Animation::new(LOOP).repeat(),
                                    move |readout, t| readout.value(loudness(t)),
                                ),
                            )
                            .child(Readout {
                                value: -13.8,
                                unit: "LUFS",
                                label: "Short-term",
                                large: false,
                            })
                            .child(Readout {
                                value: -14.1,
                                unit: "LUFS",
                                label: "Integrated",
                                large: false,
                            })
                            .child(
                                div()
                                    .flex()
                                    .gap(theme.spacing.lg)
                                    .child(Readout {
                                        value: -1.2,
                                        unit: "dBTP",
                                        label: "True peak",
                                        large: false,
                                    })
                                    .child(Readout {
                                        value: 6.4,
                                        unit: "LU",
                                        label: "Range",
                                        large: false,
                                    }),
                            )
                            .child(
                                Badge::new("On target for streaming")
                                    .tone(Tone::Success)
                                    .dot(),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(theme.spacing.xs)
                            .min_h(px(230.))
                            .child(meter("meter-l", 0.))
                            .child(meter("meter-r", 0.013)),
                    ),
            )
    }

    fn spectrogram(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        Self::panel("Spectrogram", "Last 3 seconds, high frequencies on top", cx)
            .flex_1()
            .min_w_0()
            .child(Self::plot(cx).flex_1().min_h(px(160.)).child(
                Heatmap::new(HISTORY, SPECTROGRAM_ROWS, spectrogram_at(0.)).with_animation(
                    "spectrogram",
                    Animation::new(LOOP).repeat(),
                    |heatmap, t| heatmap.values(spectrogram_at(t)),
                ),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.sm)
                    .child(
                        Label::new("Quiet")
                            .size(TextSize::XSmall)
                            .color(TextColor::Faint),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .h(px(6.))
                            .rounded(px(3.))
                            .overflow_hidden()
                            .children(
                                theme
                                    .colors
                                    .heat
                                    .iter()
                                    .map(|color| div().flex_1().h_full().bg(*color)),
                            ),
                    )
                    .child(
                        Label::new("Loud")
                            .size(TextSize::XSmall)
                            .color(TextColor::Faint),
                    ),
            )
    }

    fn stereo(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let correlation = 0.72_f32;
        Self::panel("Stereo", "Goniometer and phase correlation", cx)
            .w(px(300.))
            .flex_none()
            .child(Self::plot(cx).flex_1().min_h(px(160.)).child(
                Vectorscope::new(stereo_at(0., 600, 3.)).with_animation(
                    "vectorscope",
                    Animation::new(LOOP).repeat(),
                    |scope, t| scope.points(stereo_at(t, 600, 3.)),
                ),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.sm)
                    .child(
                        Label::new("−1")
                            .size(TextSize::XSmall)
                            .mono()
                            .color(TextColor::Faint),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .h(px(6.))
                            .rounded(px(3.))
                            .bg(theme.colors.sunken)
                            .child(
                                div()
                                    .absolute()
                                    .left(relative(0.5))
                                    .top_0()
                                    .bottom_0()
                                    .w(px(1.))
                                    .bg(theme.colors.grid),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(relative(0.5))
                                    .w(relative(correlation / 2.))
                                    .top_0()
                                    .bottom_0()
                                    .rounded(px(3.))
                                    .bg(theme.colors.success),
                            ),
                    )
                    .child(
                        Label::new("+1")
                            .size(TextSize::XSmall)
                            .mono()
                            .color(TextColor::Faint),
                    )
                    .child(
                        Label::new(format!("{correlation:+.2}"))
                            .size(TextSize::Small)
                            .mono(),
                    ),
            )
    }

    fn oscilloscope(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let channel = |left: bool| {
            let samples = move |t: f32| {
                stereo_at(t, 512, 2.)
                    .into_iter()
                    .map(|(l, r)| if left { l } else { r })
                    .collect::<Vec<_>>()
            };
            // The right channel draws over the left without a second grid.
            let wave = if left {
                Waveform::new(samples(0.))
            } else {
                Waveform::new(samples(0.))
                    .no_grid()
                    .color(theme.colors.series[1])
            };
            wave.with_animation(
                if left { "scope-l" } else { "scope-r" },
                Animation::new(LOOP).repeat(),
                move |wave, t| wave.samples(samples(t)),
            )
        };
        let legend = |label: &'static str, color| {
            div()
                .flex()
                .items_center()
                .gap(theme.spacing.xs)
                .child(div().w(px(12.)).h(px(2.)).bg(color))
                .child(
                    Label::new(label)
                        .size(TextSize::XSmall)
                        .color(TextColor::Muted),
                )
        };
        Self::panel(
            "Oscilloscope",
            "Two cycles of the bass, triggered on its rising edge",
            cx,
        )
        .flex_grow()
        .child(
            Self::plot(cx)
                .relative()
                .flex_1()
                .min_h(px(120.))
                .child(div().absolute().inset_0().child(channel(true)))
                .child(div().absolute().inset_0().child(channel(false))),
        )
        .child(
            div()
                .flex()
                .gap(theme.spacing.lg)
                .child(legend("Left", theme.colors.series[0]))
                .child(legend("Right", theme.colors.series[1])),
        )
    }
}

impl Render for Visualizer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let switch = common::theme_switch(self, cx);
        WindowFrame::new()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .size_full()
            .flex()
            .flex_col()
            .child(
                TitleBar::new()
                    .title("Analyzer")
                    .child(
                        SegmentedControl::new(
                            "tap",
                            [Segment::label("Pre-fader"), Segment::label("Post-fader")],
                            self.tap,
                        )
                        .size(ControlSize::Small)
                        .on_select(cx.listener(
                            |this, index: &usize, _, cx| {
                                this.tap = *index;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        Button::new("freeze", "Freeze")
                            .size(ControlSize::Small)
                            .icon(IconName::Pause),
                    )
                    .trailing(switch)
                    .trailing(Divider::vertical())
                    .trailing(
                        Button::icon_only("settings", IconName::Settings)
                            .tooltip("Analyzer settings…"),
                    ),
            )
            .child(
                div()
                    .id("analyzer")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap(theme.spacing.lg)
                    .p(theme.spacing.lg)
                    .bg(theme.colors.background)
                    // Spare height goes to the plots; a short window scrolls instead.
                    .child(
                        div()
                            .flex()
                            .flex_grow()
                            .gap(theme.spacing.lg)
                            .child(self.spectrum(cx))
                            .child(self.loudness(cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_grow()
                            .gap(theme.spacing.lg)
                            .child(self.spectrogram(cx))
                            .child(self.stereo(cx)),
                    )
                    .child(self.oscilloscope(cx)),
            )
    }
}

fn main() {
    common::run("Analyzer", 1400., 980., |choice, _| Visualizer {
        choice,
        tap: 1,
    });
}
