//! A mock of Auris Studio's physical-instrument view: the strings' motion, the partials they
//! produce, the body that filters them, and the parameters that shape them. The motion is a
//! sum of string modes computed on the fly, slowed down so it can be seen; nothing is synthesised.
//!
//! ```text
//! cargo run --example instrument_lab -- [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use std::f32::consts::PI;
use std::time::Duration;

use atelier::agent::AskBar;
use atelier::viz::{Heatmap, Spectrum, SpectrumStyle, StringLane};
use atelier::{
    ActiveTheme, Appearance, Badge, Button, Card, ControlSize, Direction, Divider, Edge, IconName,
    Knob, Label, LevelMeter, ListItem, SectionHeader, Segment, SegmentedControl, Sidebar,
    StatusDot, TaskProgress, TextColor, TextSize, TitleBar, Tone, WindowFrame,
};
use common::Themed;
use gpui::{
    Animation, AnimationExt, Context, Hsla, IntoElement, ParentElement, Render, Styled, Window,
    div, prelude::*, px,
};

/// One sounding voice: what it plays and how it is being excited.
struct Voice {
    note: &'static str,
    /// Fundamental, in visible cycles per animation loop.
    cycles: f32,
    /// Excitation position along the string, 0 at the nut.
    contact: f32,
    level: f32,
    held: bool,
}

const VOICES: [Voice; 4] = [
    Voice {
        note: "C3",
        cycles: 2.,
        contact: 0.12,
        level: 0.86,
        held: true,
    },
    Voice {
        note: "G3",
        cycles: 3.,
        contact: 0.14,
        level: 0.7,
        held: true,
    },
    Voice {
        note: "E4",
        cycles: 5.,
        contact: 0.1,
        level: 0.55,
        held: true,
    },
    Voice {
        note: "C5",
        cycles: 8.,
        contact: 0.18,
        level: 0.22,
        held: false,
    },
];

/// How long one loop of the slowed-down motion lasts.
const LOOP: Duration = Duration::from_secs(4);
const POINTS: usize = 64;
const MODES: usize = 8;

/// The amplitude of mode `n` (from 1) for a string excited at `contact`: modes with a node at
/// the contact point are not excited, and higher modes fall off as a bowed string's do.
fn mode_amplitude(n: usize, contact: f32) -> f32 {
    (n as f32 * PI * contact).sin() / n as f32
}

/// The string's displacement at loop phase `phase` (0 to 1).
fn string_shape(voice: &Voice, phase: f32) -> Vec<f32> {
    let norm: f32 = (1..=MODES)
        .map(|n| mode_amplitude(n, voice.contact).abs())
        .sum();
    (0..POINTS)
        .map(|index| {
            let x = index as f32 / (POINTS - 1) as f32;
            let sum: f32 = (1..=MODES)
                .map(|n| {
                    let shape = (n as f32 * PI * x).sin();
                    let time = (2. * PI * voice.cycles * n as f32 * phase).cos();
                    mode_amplitude(n, voice.contact) * shape * time
                })
                .sum();
            sum / norm * voice.level * 1.6
        })
        .collect()
}

/// Harmonic levels for the partials display, breathing slightly with the loop.
fn partials(phase: f32, contact: f32) -> Vec<f32> {
    (1..=24)
        .map(|n| {
            let base = mode_amplitude(n, contact).abs() * 1.4;
            let shimmer = 1. + 0.08 * (2. * PI * (phase * 2. + n as f32 * 0.13)).sin();
            (base * shimmer).clamp(0., 1.)
        })
        .collect()
}

/// A smooth resonance curve with a few body modes, as a cello's body would have.
fn body_response() -> Vec<f32> {
    let peaks = [
        (0.12, 0.75, 0.03),
        (0.22, 0.95, 0.025),
        (0.36, 0.7, 0.04),
        (0.58, 0.5, 0.06),
    ];
    (0..96)
        .map(|index| {
            let x = index as f32 / 95.;
            let floor = 0.35 * (1. - x * 0.6);
            let resonance: f32 = peaks
                .iter()
                .map(|(at, height, width)| height * (-((x - at) / width).powi(2)).exp())
                .sum();
            (floor + resonance * 0.6).clamp(0., 1.)
        })
        .collect()
}

/// A drum head's displacement: the (0,1) and (1,1) modes beating against each other.
fn membrane(phase: f32, size: usize) -> Vec<f32> {
    let mut values = Vec::with_capacity(size * size);
    for row in 0..size {
        for column in 0..size {
            let x = (column as f32 + 0.5) / size as f32 * 2. - 1.;
            let y = (row as f32 + 0.5) / size as f32 * 2. - 1.;
            let r = (x * x + y * y).sqrt().min(1.);
            let theta = y.atan2(x);
            let fundamental = (PI / 2. * r).cos() * (2. * PI * phase * 3.).cos();
            let dipole = r * (1. - r * r) * 2.6 * theta.cos() * (2. * PI * phase * 5.).cos();
            values.push((0.6 * fundamental + 0.5 * dipole).clamp(-1., 1.));
        }
    }
    values
}

struct InstrumentLab {
    choice: (Direction, Appearance),
    model: usize,
    exciter: usize,
    speed: usize,
}

impl Themed for InstrumentLab {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

impl InstrumentLab {
    fn models(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let models = [
            ("Cello", "Bowed string", IconName::Waveform),
            ("Nylon guitar", "Plucked string", IconName::Music),
            ("Grand piano", "Struck string", IconName::Music),
            ("Drum kit", "Membranes", IconName::Record),
        ];
        Sidebar::new(Edge::Left)
            .w(px(232.))
            .p(theme.spacing.sm)
            .gap(theme.spacing.xxs)
            .child(
                div()
                    .px(theme.spacing.sm)
                    .py(theme.spacing.xs)
                    .child(Label::caption("Models")),
            )
            .children(
                models
                    .into_iter()
                    .enumerate()
                    .map(|(index, (name, kind, icon))| {
                        ListItem::new(("model", index), name)
                            .icon(icon)
                            .description(kind)
                            .selected(index == self.model)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.model = index;
                                cx.notify();
                            }))
                    }),
            )
            .child(
                div()
                    .px(theme.spacing.sm)
                    .pt(theme.spacing.lg)
                    .pb(theme.spacing.xs)
                    .child(Label::caption("Presets")),
            )
            .child(
                ListItem::new("preset-1", "Warm legato")
                    .no_icon_lane()
                    .selected(false),
            )
            .child(ListItem::new("preset-2", "Sul ponticello").no_icon_lane())
            .child(
                ListItem::new("preset-3", "Calibrated · studio")
                    .no_icon_lane()
                    .trailing(Badge::new("Fit").tone(Tone::Agent)),
            )
    }

    fn string_motion(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let series = theme.colors.series;
        let lane_height = px(66.);
        let header = div()
            .flex()
            .items_center()
            .gap(theme.spacing.md)
            .child(
                SectionHeader::new("String motion")
                    .description("Slowed down; amplitude exaggerated"),
            )
            .child(div().flex_1())
            .child(
                SegmentedControl::new(
                    "speed",
                    [
                        Segment::label("×1/50"),
                        Segment::label("×1/200"),
                        Segment::icon(IconName::Pause),
                    ],
                    self.speed,
                )
                .size(ControlSize::Small)
                .on_select(cx.listener(|this, index: &usize, _, cx| {
                    this.speed = *index;
                    cx.notify();
                })),
            );
        let lanes = VOICES.iter().enumerate().map(|(index, voice)| {
            let color: Hsla = series[index % series.len()];
            let lane = StringLane::new(string_shape(voice, 0.))
                .contact(voice.contact)
                .color(color);
            let lane = if voice.held { lane } else { lane.inactive() };
            // Each voice's motion is rebuilt every frame from the loop phase, with fading traces
            // of the last few frames behind it.
            let voice_index = index;
            let animated = lane.with_animation(
                ("string", index),
                Animation::new(LOOP).repeat(),
                move |lane, phase| {
                    let voice = &VOICES[voice_index];
                    let trails = (1..=4)
                        .rev()
                        .map(|age| string_shape(voice, (phase - age as f32 * 0.006).rem_euclid(1.)))
                        .collect();
                    lane.points(string_shape(voice, phase)).trails(trails)
                },
            );
            div()
                .flex()
                .items_center()
                .gap(theme.spacing.md)
                .h(lane_height)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .w(px(64.))
                        .gap(theme.spacing.xxs)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(theme.spacing.xs)
                                .child(StatusDot::new(color))
                                .child(
                                    Label::new(voice.note)
                                        .mono()
                                        .weight(theme.typography.label_weight),
                                ),
                        )
                        .child(
                            Label::new(if voice.held { "Held" } else { "Released" })
                                .size(TextSize::XSmall)
                                .color(TextColor::Faint),
                        ),
                )
                .child(div().flex_1().h_full().child(animated))
                .child(
                    div()
                        .h(lane_height - px(16.))
                        .child(LevelMeter::new(voice.level).thickness(px(4.))),
                )
        });
        Card::new()
            .p(theme.spacing.lg)
            .gap(theme.spacing.md)
            .child(header)
            .child(div().flex().flex_col().children(lanes))
    }

    fn analysis_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let contact = VOICES[0].contact;
        let panel = |title: &'static str, description: &'static str| {
            Card::new()
                .flex_1()
                .min_w_0()
                .p(theme.spacing.lg)
                .gap(theme.spacing.md)
                .child(SectionHeader::new(title).description(description))
        };
        let plot = |body: gpui::AnyElement| {
            div()
                .h(px(150.))
                .rounded(theme.shape.radius_sm)
                .bg(theme.colors.sunken)
                .overflow_hidden()
                .p(theme.spacing.xs)
                .child(body)
        };
        div()
            .flex()
            .gap(theme.spacing.lg)
            .child(
                panel(
                    "Partials",
                    "Harmonics 1–24 of C3; the bow position notches every 8th",
                )
                .child(plot(
                    Spectrum::new(partials(0., contact))
                        .octaves(24)
                        .with_animation(
                            "partials",
                            Animation::new(LOOP).repeat(),
                            move |spectrum, phase| spectrum.levels(partials(phase, contact)),
                        )
                        .into_any_element(),
                )),
            )
            .child(
                panel("Body response", "Air and wood resonances below 1 kHz").child(plot(
                    Spectrum::new(body_response())
                        .style(SpectrumStyle::Curve)
                        .color(theme.colors.series[1])
                        .octaves(8)
                        .into_any_element(),
                )),
            )
            .child(
                Card::new()
                    .w(px(240.))
                    .flex_none()
                    .p(theme.spacing.lg)
                    .gap(theme.spacing.md)
                    .child(SectionHeader::new("Drum head").description("Membrane preview"))
                    .child(
                        div().h(px(150.)).child(
                            Heatmap::new(28, 28, membrane(0., 28))
                                .circular()
                                .signed()
                                .with_animation(
                                    "membrane",
                                    Animation::new(LOOP).repeat(),
                                    |heatmap, phase| heatmap.values(membrane(phase, 28)),
                                ),
                        ),
                    ),
            )
    }

    fn inspector(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let group = |title: &'static str| {
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.md)
                .child(SectionHeader::caption(title))
        };
        let knobs = || div().flex().flex_wrap().gap(theme.spacing.lg);
        Sidebar::new(Edge::Right)
            .w(px(300.))
            .p(theme.spacing.lg)
            .gap(theme.spacing.xl)
            .child(
                group("Exciter")
                    .child(
                        SegmentedControl::new(
                            "exciter",
                            [
                                Segment::label("Bow"),
                                Segment::label("Pluck"),
                                Segment::label("Strike"),
                            ],
                            self.exciter,
                        )
                        .on_select(cx.listener(
                            |this, index: &usize, _, cx| {
                                this.exciter = *index;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        knobs()
                            .child(Knob::new(0.62).label("Pressure").readout("0.62"))
                            .child(Knob::new(0.4).label("Speed").readout("0.40"))
                            .child(Knob::new(0.12).label("Position").readout("12 %")),
                    ),
            )
            .child(
                group("String").child(
                    knobs()
                        .child(Knob::new(0.7).label("Tension").readout("70 %"))
                        .child(Knob::new(0.18).label("Stiffness").readout("0.18"))
                        .child(Knob::new(0.35).label("Damping").readout("0.35")),
                ),
            )
            .child(
                group("Body").child(
                    knobs()
                        .child(
                            Knob::new(0.55)
                                .label("Size")
                                .readout("55 %")
                                .color(theme.colors.series[1]),
                        )
                        .child(
                            Knob::new(0.5)
                                .bipolar()
                                .label("Brightness")
                                .readout("0.0")
                                .color(theme.colors.series[1]),
                        ),
                ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.md)
                    .child(SectionHeader::caption("Assistant"))
                    .child(
                        TaskProgress::new("fit", "Fitting to reference")
                            .icon(IconName::Sparkles)
                            .detail("Partial decay · pass 37")
                            .progress(0.64)
                            .by_agent()
                            .on_cancel(|_, _, _| {}),
                    )
                    .child(AskBar::new("ask", "Ask about this tone…").shortcut(
                        if cfg!(target_os = "macos") {
                            "⌘K"
                        } else {
                            "Ctrl+K"
                        },
                    )),
            )
    }
}

impl Render for InstrumentLab {
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
                    .title("Cello")
                    .subtitle("Physical instrument · Strings track")
                    .child(div().w(theme.spacing.md))
                    .child(
                        Button::icon_only("play", IconName::Play)
                            .selected(true)
                            .tooltip("Preview"),
                    )
                    .child(
                        Button::new("freeze", "Freeze")
                            .size(ControlSize::Small)
                            .icon(IconName::Pause),
                    )
                    .child(Badge::new("3 of 4 voices"))
                    .trailing(switch)
                    .trailing(Divider::vertical())
                    .trailing(Button::icon_only("more", IconName::More).tooltip("More…")),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.models(cx))
                    .child(
                        div()
                            .id("stage")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(theme.spacing.lg)
                            .p(theme.spacing.lg)
                            .overflow_y_scroll()
                            .bg(theme.colors.surface)
                            .child(self.string_motion(cx))
                            .child(self.analysis_row(cx)),
                    )
                    .child(self.inspector(cx)),
            )
    }
}

fn main() {
    common::run("Instrument lab", 1480., 900., |choice, _| InstrumentLab {
        choice,
        model: 0,
        exciter: 0,
        speed: 0,
    });
}
