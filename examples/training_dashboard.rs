//! A mock training dashboard in the spirit of TensorBoard, with a playground for trying the
//! model as it trains. Four runs of a small classifier train side by side on a synthetic
//! dataset of eight shapes; their scalars (training and validation loss, accuracy, learning rate,
//! gradient norm) are logged step by step and plotted as they arrive, smoothed the way
//! TensorBoard smooths them. The Distributions page shows how each layer's weights spread as they
//! train, the Runs page compares the runs in a table, and the Inference page, after Gradio, runs
//! an example image through any run's latest checkpoint. One run's learning rate is too high and
//! it diverges part way through. The title bar pauses the clock, restarts it and sets its speed.
//!
//! ```text
//! cargo run --example training_dashboard -- [--page scalars|distributions|runs|inference] [--step N] [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use std::f32::consts::TAU;
use std::time::Instant;

use atelier::plot::{FieldPlot, Histogram, LinePlot, Series, SeriesStyle};
use atelier::viz::Heatmap;
use atelier::{
    ActiveTheme, Appearance, Badge, Button, Card, Checkbox, ControlSize, Direction, Disclosure,
    Edge, IconName, Label, ProgressBar, SectionHeader, Segment, SegmentedControl, Sidebar,
    StatusBar, StatusDot, Switch, Tabs, TextColor, TextSize, TitleBar, Tone, WindowFrame,
};
use common::Themed;
use gpui::{
    App, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::*, px,
};

/// How long every run trains, and how its steps divide into epochs.
const TOTAL_STEPS: usize = 4000;
const STEPS_PER_EPOCH: usize = 400;
/// How often the runs are validated and their weights summarised, and how often a checkpoint
/// is written for the playground.
const EVAL_EVERY: usize = 100;
const CHECKPOINT_EVERY: usize = 250;
/// Training steps per second of the clock at 1×, and the speeds the title bar offers.
const STEPS_PER_SECOND: f32 = 40.;
const SPEEDS: [f32; 3] = [1., 4., 16.];
const BATCH: usize = 128;
/// Steps the learning rate warms up over before its cosine decay.
const WARMUP: usize = 200;

/// The dataset: eight shapes, drawn small and jittered.
const CLASSES: [&str; 8] = [
    "circle", "ring", "square", "frame", "cross", "diagonal", "stripes", "checker",
];
/// The class each one is most easily mistaken for.
const LOOKALIKE: [usize; 8] = [1, 0, 3, 2, 5, 4, 7, 6];
/// Cross-entropy of a uniform guess over the classes: where every run starts.
const INITIAL_LOSS: f32 = 2.079;
/// The side of an input image, in pixels.
const SIDE: usize = 24;

/// The smoothing weights offered, TensorBoard's exponential moving average.
const SMOOTHING: [f32; 4] = [0., 0.6, 0.9, 0.99];
const TEMPERATURES: [f32; 3] = [0.5, 1., 2.];
const NOISE: [f32; 3] = [0., 0.25, 0.5];

/// A small deterministic generator, so every run draws the same samples.
#[derive(Clone)]
struct Random(u64);

impl Random {
    fn uniform(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 40) as f32 + 0.5) / (1u64 << 24) as f32
    }

    /// A standard normal sample, by Box–Muller.
    fn normal(&mut self) -> f32 {
        let (a, b) = (self.uniform(), self.uniform());
        (-2. * a.ln()).sqrt() * (TAU * b).cos()
    }
}

/// How one run is set up, and how its loss behaves.
struct RunSpec {
    name: &'static str,
    optimizer: &'static str,
    learning_rate: f32,
    /// Steps over which the loss falls, and the loss it falls towards.
    tau: f32,
    floor: f32,
    /// Step-to-step noise in the training loss, as a log-normal spread.
    noise: f32,
    /// How far the validation loss climbs away from the training loss by the end.
    overfit: f32,
    /// The step after which the loss runs away.
    diverges_after: Option<usize>,
}

const RUNS: [RunSpec; 4] = [
    RunSpec {
        name: "adamw-3e-4",
        optimizer: "AdamW",
        learning_rate: 3e-4,
        tau: 260.,
        floor: 0.11,
        noise: 0.07,
        overfit: 0.03,
        diverges_after: None,
    },
    RunSpec {
        name: "adamw-1e-3",
        optimizer: "AdamW",
        learning_rate: 1e-3,
        tau: 120.,
        floor: 0.05,
        noise: 0.1,
        overfit: 0.42,
        diverges_after: None,
    },
    RunSpec {
        name: "sgd-5e-2",
        optimizer: "SGD + momentum",
        learning_rate: 5e-2,
        tau: 720.,
        floor: 0.2,
        noise: 0.05,
        overfit: 0.,
        diverges_after: None,
    },
    RunSpec {
        name: "adamw-6e-3",
        optimizer: "AdamW",
        learning_rate: 6e-3,
        tau: 90.,
        floor: 0.32,
        noise: 0.2,
        overfit: 0.,
        diverges_after: Some(1640),
    },
];

/// The learning rate of `spec` at `step`: a linear warm-up, then a cosine decay to a tenth.
fn learning_rate(spec: &RunSpec, step: usize) -> f32 {
    if step < WARMUP {
        return spec.learning_rate * (step + 1) as f32 / WARMUP as f32;
    }
    let progress = (step - WARMUP) as f32 / (TOTAL_STEPS - WARMUP) as f32;
    spec.learning_rate * (0.1 + 0.45 * (1. + (std::f32::consts::PI * progress).cos()))
}

/// The loss `spec` would reach at `step` without noise: a power-law fall, a little lower again
/// as the learning rate decays.
fn ideal_loss(spec: &RunSpec, step: usize) -> f32 {
    let fall = (1. + step as f32 / spec.tau).powf(-1.1);
    let anneal = 1. - 0.3 * (step as f32 / TOTAL_STEPS as f32).powi(2);
    (spec.floor + (INITIAL_LOSS - spec.floor) * fall) * anneal
}

/// The validation loss `spec` would reach at `step`: a little above the training loss, and
/// climbing away from it as the run overfits.
fn ideal_val_loss(spec: &RunSpec, step: usize) -> f32 {
    ideal_loss(spec, step) * 1.06 + spec.overfit * (step as f32 / TOTAL_STEPS as f32).powi(2)
}

/// Accuracy at a validation loss: chance at the start, near perfect as the loss nears zero.
fn accuracy(loss: f32) -> f32 {
    let chance = 1. / CLASSES.len() as f32;
    chance + (1. - chance) * (1. - loss / INITIAL_LOSS).clamp(0., 1.)
}

/// One run as it trains: everything it has logged so far.
struct Run {
    spec: &'static RunSpec,
    visible: bool,
    random: Random,
    /// The training loss and gradient norm at every step taken, from step 1.
    loss: Vec<f32>,
    grad_norm: Vec<f32>,
    /// `(step, loss, accuracy)` at each validation.
    validation: Vec<(usize, f32, f32)>,
    /// The step at which the loss became NaN.
    diverged: Option<usize>,
}

impl Run {
    fn new(index: usize) -> Self {
        Self {
            spec: &RUNS[index],
            visible: true,
            random: Random(17 + index as u64 * 7919),
            loss: Vec::new(),
            grad_norm: Vec::new(),
            validation: Vec::new(),
            diverged: None,
        }
    }

    fn step(&self) -> usize {
        self.loss.len()
    }

    fn running(&self) -> bool {
        self.diverged.is_none() && self.step() < TOTAL_STEPS
    }

    /// Take one training step and log it.
    fn advance(&mut self) {
        if !self.running() {
            return;
        }
        let spec = self.spec;
        let step = self.step() + 1;
        let random = &mut self.random;
        let mut loss = ideal_loss(spec, step)
            * (spec.noise * random.normal() - spec.noise * spec.noise / 2.).exp();
        let mut grad =
            (0.45 + 2.4 * (-(step as f32) / 260.).exp()) * (0.22 * random.normal()).exp();
        // Now and then a hard batch.
        if random.uniform() < 0.004 {
            let spike = 1.4 + random.uniform();
            loss *= spike;
            grad *= spike * 1.5;
        }
        if let Some(after) = spec.diverges_after
            && step > after
        {
            let growth = ((step - after) as f32 / 36.).exp();
            loss *= growth;
            grad *= growth * growth;
            if loss > 14. {
                self.diverged = Some(step);
                return;
            }
        }
        self.loss.push(loss);
        self.grad_norm.push(grad);
        if step.is_multiple_of(EVAL_EVERY) {
            let loss = ideal_val_loss(spec, step) * (0.02 * random.normal()).exp();
            let loss = match spec.diverges_after {
                Some(after) if step > after => loss.max(self.loss[step - 1]),
                _ => loss,
            };
            self.validation.push((step, loss, accuracy(loss)));
        }
    }

    /// The latest checkpoint written: every [`CHECKPOINT_EVERY`] steps, none after a NaN.
    fn checkpoint(&self) -> usize {
        let last = match self.diverged {
            // The last checkpoint before the loss began to run away.
            Some(_) => self.spec.diverges_after.unwrap_or(0).min(self.step()),
            None => self.step(),
        };
        last / CHECKPOINT_EVERY * CHECKPOINT_EVERY
    }

    /// The training loss smoothed by `weight`, at the latest step.
    fn smoothed_loss(&self, weight: f32) -> Option<f32> {
        let points: Vec<_> = self
            .loss
            .iter()
            .enumerate()
            .map(|(index, loss)| ((index + 1) as f32, *loss))
            .collect();
        smooth(&points, weight).last().map(|(_, loss)| *loss)
    }

    fn best_accuracy(&self) -> Option<f32> {
        self.validation
            .iter()
            .map(|(_, _, accuracy)| *accuracy)
            .max_by(f32::total_cmp)
    }
}

/// TensorBoard's smoothing: an exponential moving average, debiased so it starts at the first
/// value instead of rising from zero.
fn smooth(points: &[(f32, f32)], weight: f32) -> Vec<(f32, f32)> {
    let mut average = 0.;
    let mut debias = 1.;
    points
        .iter()
        .map(|(x, y)| {
            average = average * weight + (1. - weight) * y;
            debias *= weight;
            (*x, average / (1. - debias).max(f32::EPSILON))
        })
        .collect()
}

/// At most `limit` of `points`, evenly picked, always keeping the last.
fn thin(points: Vec<(f32, f32)>, limit: usize) -> Vec<(f32, f32)> {
    if points.len() <= limit {
        return points;
    }
    let every = points.len().div_ceil(limit);
    let last = points.last().copied();
    let mut thinned: Vec<_> = points.into_iter().step_by(every).collect();
    if thinned.last().copied() != last {
        thinned.extend(last);
    }
    thinned
}

/// `count` with thousands separators.
fn grouped(count: usize) -> String {
    let digits = count.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A learning rate as people write it: 3e-4.
fn rate_label(rate: f32) -> String {
    let exponent = rate.log10().floor();
    let mantissa = rate / 10f32.powf(exponent);
    format!("{mantissa:.0}e{exponent:.0}").replace('-', "−")
}

/// The layers whose weights the Distributions page summarises, with the range their values
/// stay in.
const LAYERS: [(&str, &str, f32); 3] = [
    ("conv1/kernel", "3×3×1×16", 0.6),
    ("dense/kernel", "576×64", 0.4),
    ("logits/kernel", "64×8", 0.8),
];
const WEIGHT_SAMPLES: usize = 900;
const WEIGHT_BINS: usize = 48;

/// A sample of layer `layer`'s weights in `run` at `step`. They start as a narrow normal; the
/// first layer widens, the middle one grows heavy tails, and the output layer splits into two
/// modes as the classes pull apart. A higher learning rate spreads them faster.
fn weights(run: usize, layer: usize, step: usize) -> Vec<f32> {
    let spec = &RUNS[run];
    let mut random = Random(((run * 3 + layer) * 100_003 + step) as u64);
    let learned = 1. - (1. + step as f32 / spec.tau).powf(-0.8);
    let spread = (spec.learning_rate / 1e-3).powf(0.12);
    (0..WEIGHT_SAMPLES)
        .map(|_| {
            let z = random.normal();
            match layer {
                0 => z * 0.08 * (1. + 1.1 * learned * spread),
                1 => {
                    let tail = if random.uniform() < 0.12 * learned {
                        2.6
                    } else {
                        1.
                    };
                    z * 0.05 * (1. + 0.6 * learned * spread) * tail
                }
                _ => {
                    let side = if random.uniform() < 0.5 { -1. } else { 1. };
                    side * 0.32 * learned * spread + z * (0.09 - 0.03 * learned)
                }
            }
        })
        .collect()
}

/// The share of `samples` in each of [`WEIGHT_BINS`] bins across `-range..range`.
fn binned(samples: &[f32], range: f32) -> Vec<f32> {
    let mut bins = vec![0f32; WEIGHT_BINS];
    for value in samples {
        let at = ((value + range) / (2. * range) * WEIGHT_BINS as f32).floor();
        if (0. ..WEIGHT_BINS as f32).contains(&at) {
            bins[at as usize] += 1.;
        }
    }
    bins
}

/// An example image of `class`, drawn with jitter from `seed`, with noise of strength `noise`
/// added, in row-major order from the top.
fn image(class: usize, seed: u64, noise: f32) -> Vec<f32> {
    let mut random = Random(seed * 2_654_435_761 + class as u64);
    let angle = 0.35 * (random.uniform() - 0.5);
    let scale = 0.85 + 0.3 * random.uniform();
    let (dx, dy) = (
        0.2 * (random.uniform() - 0.5),
        0.2 * (random.uniform() - 0.5),
    );
    (0..SIDE * SIDE)
        .map(|index| {
            let at = |i: usize| (i as f32 + 0.5) / SIDE as f32 * 2. - 1.;
            let (u, v) = (at(index % SIDE) - dx, at(index / SIDE) - dy);
            let (x, y) = (
                (u * angle.cos() + v * angle.sin()) / scale,
                (v * angle.cos() - u * angle.sin()) / scale,
            );
            let radius = (x * x + y * y).sqrt();
            let square = x.abs().max(y.abs());
            let ink = match class {
                0 => radius < 0.55,
                1 => (0.38..0.62).contains(&radius),
                2 => square < 0.5,
                3 => (0.34..0.56).contains(&square),
                4 => (x.abs() < 0.14 && y.abs() < 0.62) || (y.abs() < 0.14 && x.abs() < 0.62),
                5 => (x - y).abs() < 0.22 && square < 0.75,
                6 => (x * 7.).sin() > 0.2 && square < 0.7,
                _ => {
                    ((x * 2.2 + 3.).floor() + (y * 2.2 + 3.).floor()) as i32 % 2 == 0
                        && square < 0.68
                }
            };
            let value = if ink { 0.9 } else { 0.08 };
            (value + noise * random.normal()).clamp(0., 1.)
        })
        .collect()
}

/// One input from the dataset: its class and the seed it was drawn with.
#[derive(Clone, Copy, PartialEq)]
struct Example {
    class: usize,
    seed: u64,
}

/// The examples under the playground's input, one of each class.
const EXAMPLES: [Example; 8] = [
    Example { class: 0, seed: 3 },
    Example { class: 1, seed: 5 },
    Example { class: 2, seed: 8 },
    Example { class: 3, seed: 13 },
    Example { class: 4, seed: 21 },
    Example { class: 5, seed: 34 },
    Example { class: 6, seed: 55 },
    Example { class: 7, seed: 89 },
];

/// What a run's checkpoint at `step` makes of `example` with input noise `noise`: a
/// probability for each class. The model is as good as its validation accuracy at that step
/// says; it confuses a class with its lookalike first, and noise in the input costs it
/// confidence.
fn predict(run: usize, step: usize, example: Example, noise: f32, temperature: f32) -> [f32; 8] {
    let spec = &RUNS[run];
    let quality = accuracy(ideal_val_loss(spec, step));
    let strength = 7. * (quality - 1. / 8.) / (7. / 8.) * (1. - 0.75 * noise);
    let mut random = Random(example.seed * 977 + example.class as u64 * 31 + step as u64);
    let logits: [f32; 8] = std::array::from_fn(|class| {
        let fit = if class == example.class {
            1.
        } else if class == LOOKALIKE[example.class] {
            0.5
        } else {
            0.
        };
        strength * fit + (1.3 - quality + noise) * random.normal()
    });
    let top = logits.iter().copied().fold(f32::MIN, f32::max);
    let weights = logits.map(|logit| ((logit - top) / temperature).exp());
    let total: f32 = weights.iter().sum();
    weights.map(|weight| weight / total)
}

/// One call to the model, as the playground's log keeps it.
struct Inference {
    run: usize,
    checkpoint: usize,
    example: Example,
    noise: f32,
    probabilities: [f32; 8],
    latency_ms: f32,
}

impl Inference {
    fn top(&self) -> usize {
        (0..CLASSES.len())
            .max_by(|a, b| self.probabilities[*a].total_cmp(&self.probabilities[*b]))
            .unwrap_or(0)
    }
}

/// The tags on the Scalars page, in its groups.
const GROUPS: [(&str, &str); 3] = [
    (
        "loss",
        "Cross-entropy on the training batches and on the held-out set",
    ),
    ("metrics", "Accuracy on the held-out set, every 100 steps"),
    (
        "optimizer",
        "What the optimiser did: its schedule and the gradients it saw",
    ),
];

struct TrainingDashboard {
    choice: (Direction, Appearance),
    page: usize,
    runs: Vec<Run>,
    playing: bool,
    speed: usize,
    last_frame: Option<Instant>,
    /// Steps owed to the runs but not yet taken.
    carry: f32,
    smoothing: usize,
    log_scale: bool,
    show_raw: bool,
    grid: bool,
    open: [bool; 3],
    /// The run the Distributions page shows.
    distribution_run: usize,
    /// The playground: its input, the run it asks, its settings and its log.
    example: Example,
    noise: usize,
    temperature: usize,
    model: usize,
    live: bool,
    result: Option<Inference>,
    log: Vec<Inference>,
    calls: u64,
}

impl Themed for TrainingDashboard {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

impl TrainingDashboard {
    fn new(choice: (Direction, Appearance), page: usize, start: usize) -> Self {
        let mut dashboard = Self {
            choice,
            page,
            runs: (0..RUNS.len()).map(Run::new).collect(),
            playing: true,
            speed: 0,
            last_frame: None,
            carry: 0.,
            smoothing: 1,
            log_scale: false,
            show_raw: true,
            grid: true,
            open: [true; 3],
            distribution_run: 0,
            example: EXAMPLES[0],
            noise: 0,
            temperature: 1,
            model: 0,
            live: true,
            result: None,
            log: Vec::new(),
            calls: 0,
        };
        for _ in 0..start {
            dashboard.advance_all();
        }
        if dashboard.result.is_none() {
            dashboard.submit();
        }
        dashboard
    }

    fn restart(&mut self) {
        self.runs = (0..RUNS.len())
            .map(|index| {
                let mut run = Run::new(index);
                run.visible = self.runs[index].visible;
                run
            })
            .collect();
        self.carry = 0.;
        self.log.clear();
        self.submit();
    }

    fn advance_all(&mut self) {
        let before = self.runs[self.model].checkpoint();
        for run in &mut self.runs {
            run.advance();
        }
        // Live, the playground asks again whenever its run writes a new checkpoint.
        if self.live && self.runs[self.model].checkpoint() != before {
            self.submit();
        }
    }

    /// The step the furthest run has reached.
    fn step(&self) -> usize {
        self.runs.iter().map(Run::step).max().unwrap_or(0)
    }

    fn finished(&self) -> bool {
        self.runs.iter().all(|run| !run.running())
    }

    /// Ask the playground's run about its input, at the run's latest checkpoint.
    fn submit(&mut self) {
        let run = self.model;
        let checkpoint = self.runs[run].checkpoint();
        let noise = NOISE[self.noise];
        self.calls += 1;
        let mut random = Random(self.calls);
        let inference = Inference {
            run,
            checkpoint,
            example: self.example,
            noise,
            probabilities: predict(
                run,
                checkpoint,
                self.example,
                noise,
                TEMPERATURES[self.temperature],
            ),
            latency_ms: 2.6 + 1.4 * random.uniform(),
        };
        if let Some(previous) = self.result.replace(inference) {
            self.log.insert(0, previous);
            self.log.truncate(7);
        }
    }

    fn panel(title: impl Into<SharedString>, description: &'static str, cx: &App) -> gpui::Div {
        let theme = cx.theme();
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

    fn well(cx: &App) -> gpui::Div {
        let theme = cx.theme();
        div()
            .rounded(theme.shape.radius_sm)
            .bg(theme.colors.sunken)
            .overflow_hidden()
    }

    /// A row of panels. Too narrow for them side by side, the panels move onto lines of their
    /// own.
    fn row(cx: &App) -> gpui::Div {
        div().flex().flex_wrap().gap(cx.theme().spacing.lg)
    }

    /// A run's colour, the same on every page: runs are categories.
    fn color(&self, run: usize, cx: &App) -> gpui::Hsla {
        cx.theme().categorical(run)
    }

    fn runs_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme().clone();
        let group = |title: &'static str| {
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.md)
                .child(SectionHeader::caption(title))
        };
        let runs = self.runs.iter().enumerate().map(|(index, run)| {
            let (state, tone) = if run.diverged.is_some() {
                ("NaN", Tone::Danger)
            } else if run.running() {
                ("Running", Tone::Accent)
            } else {
                ("Done", Tone::Success)
            };
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.xxs)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme.spacing.sm)
                        .child(
                            Checkbox::new(SharedString::from(format!("run-{index}")), run.visible)
                                .label(run.spec.name)
                                .on_change(cx.listener(move |this, visible: &bool, _, cx| {
                                    this.runs[index].visible = *visible;
                                    cx.notify();
                                })),
                        )
                        .child(StatusDot::new(self.color(index, cx)))
                        .child(div().flex_1())
                        .child(Badge::new(state).tone(tone)),
                )
                .child(
                    div().pl(px(26.)).child(
                        Label::new(format!(
                            "{} · lr {} · bs {BATCH}",
                            run.spec.optimizer,
                            rate_label(run.spec.learning_rate)
                        ))
                        .size(TextSize::Small)
                        .color(TextColor::Faint)
                        .truncate(),
                    ),
                )
        });
        Sidebar::new(Edge::Left)
            .id("runs")
            .w(px(272.))
            .p(theme.spacing.lg)
            .gap(theme.spacing.xl)
            .overflow_y_scroll()
            .child(group("Runs").children(runs))
            .child(
                group("Smoothing")
                    .child(
                        SegmentedControl::new(
                            "smoothing",
                            SMOOTHING.map(|weight| {
                                Segment::label(if weight == 0. {
                                    "Off".to_string()
                                } else {
                                    format!("{weight}")
                                })
                            }),
                            self.smoothing,
                        )
                        .size(ControlSize::Small)
                        .on_select(cx.listener(
                            |this, index: &usize, _, cx| {
                                this.smoothing = *index;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        Switch::new("raw", self.show_raw)
                            .label("Show raw values")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_raw = !this.show_raw;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                group("Axes")
                    .child(
                        Switch::new("log", self.log_scale)
                            .label("Log scale for loss")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.log_scale = !this.log_scale;
                                cx.notify();
                            })),
                    )
                    .child(
                        Switch::new("grid", self.grid)
                            .label("Grid")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.grid = !this.grid;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                Label::new("logdir  runs/shapes-8")
                    .size(TextSize::Small)
                    .mono()
                    .color(TextColor::Faint),
            )
    }

    /// A number with its caption, for the row over the plots.
    fn stat(caption: &'static str, value: String, detail: Option<String>, cx: &App) -> gpui::Div {
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(140.))
            .gap(theme.spacing.xs)
            .p(theme.spacing.md)
            .rounded(theme.shape.radius_md)
            .bg(theme.colors.elevated)
            .border_1()
            .border_color(theme.colors.border)
            .child(Label::caption(caption))
            .child(Label::new(value).size(TextSize::XLarge).mono())
            .children(detail.map(|detail| {
                Label::new(detail)
                    .size(TextSize::Small)
                    .color(TextColor::Muted)
                    .truncate()
            }))
    }

    fn stats(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let step = self.step();
        let running = self.runs.iter().filter(|run| run.running()).count();
        let rate = STEPS_PER_SECOND * SPEEDS[self.speed];
        let seconds = ((TOTAL_STEPS - step) as f32 / rate).ceil() as usize;
        let best = self
            .runs
            .iter()
            .filter_map(|run| Some((run.spec.name, run.best_accuracy()?)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        Self::row(cx)
            .gap(cx.theme().spacing.md)
            .child(
                Self::stat(
                    "Step",
                    grouped(step),
                    Some(format!("of {}", grouped(TOTAL_STEPS))),
                    cx,
                )
                .child(ProgressBar::new(step as f32 / TOTAL_STEPS as f32).id("progress")),
            )
            .child(Self::stat(
                "Epoch",
                format!(
                    "{}",
                    (step / STEPS_PER_EPOCH + 1).min(TOTAL_STEPS / STEPS_PER_EPOCH)
                ),
                Some(format!("of {}", TOTAL_STEPS / STEPS_PER_EPOCH)),
                cx,
            ))
            .child(Self::stat(
                "Throughput",
                if running == 0 || !self.playing {
                    "—".into()
                } else {
                    format!("{:.1}k", rate * (BATCH * running) as f32 / 1000.)
                },
                Some("samples per second".into()),
                cx,
            ))
            .child(Self::stat(
                "ETA",
                if running == 0 {
                    "done".into()
                } else {
                    format!("{}:{:02}", seconds / 60, seconds % 60)
                },
                Some(format!("{running} of {} runs training", RUNS.len())),
                cx,
            ))
            .child(Self::stat(
                "Best val accuracy",
                best.map_or("—".into(), |(_, accuracy)| {
                    format!("{:.1}%", accuracy * 100.)
                }),
                best.map(|(name, _)| name.to_string()),
                cx,
            ))
    }

    /// A tag's plot: each visible run's points in its colour, smoothed when the page smooths,
    /// with the raw values faint behind.
    fn scalar(
        &self,
        (tag, description): (&'static str, &'static str),
        y_label: &'static str,
        smooth_it: bool,
        y_range: Option<(f32, f32)>,
        values: impl Fn(&Run) -> Vec<(f32, f32)>,
        cx: &App,
    ) -> gpui::Div {
        let weight = SMOOTHING[self.smoothing];
        let mut plot = LinePlot::new()
            .x_range(0., self.step().max(EVAL_EVERY) as f32)
            .x_label("step")
            .y_label(y_label)
            .grid(self.grid)
            .legend(true);
        if let Some((min, max)) = y_range {
            plot = plot.y_range(min, max);
        }
        for (index, run) in self.runs.iter().enumerate() {
            if !run.visible {
                continue;
            }
            let color = self.color(index, cx);
            let points = values(run);
            if points.is_empty() {
                continue;
            }
            if smooth_it && weight > 0. {
                if self.show_raw {
                    plot =
                        plot.series(Series::line(thin(points.clone(), 900)).color(color).faint());
                }
                plot = plot.series(
                    Series::line(thin(smooth(&points, weight), 900))
                        .color(color)
                        .name(run.spec.name),
                );
            } else {
                let style = if points.len() < 60 {
                    SeriesStyle::LineMarkers
                } else {
                    SeriesStyle::Line
                };
                plot = plot.series(
                    Series::line(thin(points, 900))
                        .style(style)
                        .color(color)
                        .name(run.spec.name),
                );
            }
        }
        Self::panel(tag, description, cx)
            .flex_1()
            .min_w(px(MIN_PANEL_WIDTH))
            .child(Self::well(cx).h(px(240.)).child(plot))
    }

    fn scalars(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme().clone();
        let log = self.log_scale;
        let loss = move |value: f32| if log { value.log10() } else { value };
        let loss_label = if log { "loss (log₁₀)" } else { "loss" };
        // Fixed, so a run that blows up runs off the top instead of flattening the others.
        let loss_range = if log { (-1.5, 0.5) } else { (0., 2.5) };
        let group = |index: usize, content: gpui::Div, cx: &mut Context<Self>| {
            let (name, description) = GROUPS[index];
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.md)
                .child(
                    Disclosure::new(
                        SharedString::from(format!("group-{name}")),
                        name,
                        self.open[index],
                    )
                    .trailing(
                        Label::new(description)
                            .size(TextSize::Small)
                            .color(TextColor::Faint)
                            .truncate(),
                    )
                    .on_toggle(cx.listener(move |this, _, _, cx| {
                        this.open[index] = !this.open[index];
                        cx.notify();
                    })),
                )
                .when(self.open[index], |group| group.child(content))
        };
        let train = self.scalar(
            ("loss/train", "Per step, on each batch of 128"),
            loss_label,
            true,
            Some(loss_range),
            |run| {
                run.loss
                    .iter()
                    .enumerate()
                    .map(|(index, value)| ((index + 1) as f32, loss(*value)))
                    .collect()
            },
            cx,
        );
        let val = self.scalar(
            ("loss/val", "On 2,000 held-out images"),
            loss_label,
            false,
            Some(loss_range),
            |run| {
                run.validation
                    .iter()
                    .map(|(step, value, _)| (*step as f32, loss(*value)))
                    .collect()
            },
            cx,
        );
        let accuracy = self.scalar(
            ("accuracy/val", "Share of held-out images classified right"),
            "accuracy",
            false,
            Some((0., 1.)),
            |run| {
                run.validation
                    .iter()
                    .map(|(step, _, accuracy)| (*step as f32, *accuracy))
                    .collect()
            },
            cx,
        );
        let rate = self.scalar(
            (
                "learning_rate",
                "Linear warm-up over 200 steps, then cosine decay",
            ),
            "lr (log₁₀)",
            false,
            None,
            |run| {
                (0..run.step())
                    .step_by(10)
                    .map(|step| (step as f32, learning_rate(run.spec, step).log10()))
                    .collect()
            },
            cx,
        );
        let grad = self.scalar(
            (
                "grad_norm",
                "Global L2 norm of the gradients before clipping",
            ),
            "‖g‖",
            true,
            Some((0., 4.)),
            |run| {
                run.grad_norm
                    .iter()
                    .enumerate()
                    .map(|(index, value)| ((index + 1) as f32, value.min(12.)))
                    .collect()
            },
            cx,
        );
        div()
            .flex()
            .flex_col()
            .gap(theme.spacing.lg)
            .child(self.stats(cx))
            .child(group(0, Self::row(cx).child(train).child(val), cx))
            .child(group(1, Self::row(cx).child(accuracy), cx))
            .child(group(2, Self::row(cx).child(rate).child(grad), cx))
    }

    fn distributions(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme().clone();
        let run_index = self.distribution_run;
        let run = &self.runs[run_index];
        let last = run.step() / EVAL_EVERY * EVAL_EVERY;
        let snapshots: Vec<usize> = (0..=last).step_by(EVAL_EVERY).collect();
        let layers = LAYERS
            .iter()
            .enumerate()
            .map(|(layer, (name, shape, range))| {
                let range = *range;
                let now = weights(run_index, layer, last);
                // Each row of the field is one summary, its bins scaled to the fullest.
                let mut rows: Vec<Vec<f32>> = snapshots
                    .iter()
                    .map(|step| binned(&weights(run_index, layer, *step), range))
                    .collect();
                if rows.len() == 1 {
                    rows.push(rows[0].clone());
                }
                let peak = rows.iter().flatten().copied().fold(1f32, f32::max);
                let count = rows.len();
                let field: Vec<f32> = rows
                    .into_iter()
                    .flatten()
                    .map(|value| value / peak)
                    .collect();
                Self::panel(
                    SharedString::from(*name),
                    "This step's weights, and every summary since the start, 100 steps apart",
                    cx,
                )
                .flex_1()
                .min_w(px(MIN_PANEL_WIDTH))
                .child(
                    Label::new(format!("{shape} · step {}", grouped(last)))
                        .size(TextSize::Small)
                        .mono()
                        .color(TextColor::Muted),
                )
                .child(
                    Self::well(cx).h(px(170.)).child(
                        Histogram::new(now)
                            .bins(WEIGHT_BINS)
                            .range(-range, range)
                            .color(self.color(run_index, cx))
                            .x_label("weight"),
                    ),
                )
                .child(
                    Self::well(cx).h(px(200.)).child(
                        FieldPlot::new(WEIGHT_BINS, count, field)
                            .x_range(-range, range)
                            .y_range(0., last.max(EVAL_EVERY) as f32)
                            .value_range(0., 1.)
                            .x_label("weight")
                            .y_label("step"),
                    ),
                )
            });
        div()
            .flex()
            .flex_col()
            .gap(theme.spacing.lg)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.md)
                    .child(Label::new("Run").color(TextColor::Muted))
                    .child(
                        SegmentedControl::new(
                            "distribution-run",
                            RUNS.iter().map(|spec| Segment::label(spec.name)),
                            run_index,
                        )
                        .size(ControlSize::Small)
                        .on_select(cx.listener(
                            |this, index: &usize, _, cx| {
                                this.distribution_run = *index;
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(Self::row(cx).children(layers))
    }

    fn runs_table(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme().clone();
        let weight = SMOOTHING[self.smoothing];
        let best = self
            .runs
            .iter()
            .enumerate()
            .filter_map(|(index, run)| Some((index, run.best_accuracy()?)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index);
        const COLUMNS: [(&str, f32); 8] = [
            ("Run", 0.),
            ("Optimiser", 150.),
            ("lr", 70.),
            ("Step", 80.),
            ("Train loss", 100.),
            ("Val loss", 90.),
            ("Best val acc", 120.),
            ("State", 110.),
        ];
        let cell = |index: usize, content: gpui::AnyElement| {
            let width = COLUMNS[index].1;
            let cell = div().flex().items_center().gap(theme.spacing.sm).min_w_0();
            if width == 0. {
                cell.flex_1().child(content)
            } else {
                cell.w(px(width)).flex_none().child(content)
            }
        };
        let mono = |text: String| {
            Label::new(text)
                .size(TextSize::Small)
                .mono()
                .into_any_element()
        };
        let header =
            div()
                .flex()
                .gap(theme.spacing.md)
                .px(theme.spacing.md)
                .pb(theme.spacing.sm)
                .border_b_1()
                .border_color(theme.colors.separator)
                .children(COLUMNS.iter().enumerate().map(|(index, (name, _))| {
                    cell(index, Label::caption(*name).into_any_element())
                }));
        let rows = self.runs.iter().enumerate().map(|(index, run)| {
            let validation = run.validation.last();
            let state = if let Some(step) = run.diverged {
                Badge::new(format!("NaN at {}", grouped(step))).tone(Tone::Danger)
            } else if run.running() {
                Badge::new("Running").tone(Tone::Accent).dot()
            } else {
                Badge::new("Done").tone(Tone::Success)
            };
            div()
                .flex()
                .items_center()
                .gap(theme.spacing.md)
                .px(theme.spacing.md)
                .py(theme.spacing.sm)
                .border_b_1()
                .border_color(theme.colors.separator)
                .child(cell(
                    0,
                    div()
                        .flex()
                        .items_center()
                        .gap(theme.spacing.sm)
                        .child(StatusDot::new(self.color(index, cx)))
                        .child(Label::new(run.spec.name).mono().truncate())
                        .into_any_element(),
                ))
                .child(cell(
                    1,
                    Label::new(run.spec.optimizer)
                        .size(TextSize::Small)
                        .color(TextColor::Muted)
                        .into_any_element(),
                ))
                .child(cell(2, mono(rate_label(run.spec.learning_rate))))
                .child(cell(3, mono(grouped(run.step()))))
                .child(cell(
                    4,
                    mono(
                        run.smoothed_loss(weight)
                            .map_or("—".into(), |loss| format!("{loss:.4}")),
                    ),
                ))
                .child(cell(
                    5,
                    mono(validation.map_or("—".into(), |(_, loss, _)| format!("{loss:.4}"))),
                ))
                .child(cell(
                    6,
                    match run.best_accuracy() {
                        Some(accuracy) if best == Some(index) => {
                            Badge::new(format!("{:.1}%", accuracy * 100.))
                                .tone(Tone::Success)
                                .icon(IconName::Check)
                                .into_any_element()
                        }
                        Some(accuracy) => mono(format!("{:.1}%", accuracy * 100.)),
                        None => mono("—".into()),
                    },
                ))
                .child(cell(7, state.into_any_element()))
        });
        let hparams = [
            ("Dataset", "shapes-8 · 24×24 · 16,000 train / 2,000 val"),
            ("Model", "conv 3×3×16 → dense 64 → 8 logits"),
            ("Batch", "128"),
            ("Schedule", "200 warm-up steps, cosine to 10%"),
            ("Checkpoints", "every 250 steps"),
        ];
        div()
            .flex()
            .flex_col()
            .gap(theme.spacing.lg)
            .child(
                Self::panel(
                    "Runs",
                    "Smoothed training loss, the latest validation, and the best accuracy so far",
                    cx,
                )
                .child(div().flex().flex_col().child(header).children(rows)),
            )
            .child(
                Self::panel("Shared hyperparameters", "The same for every run", cx).child(
                    div().flex().flex_col().gap(theme.spacing.sm).children(
                        hparams.into_iter().map(|(name, value)| {
                            div()
                                .flex()
                                .gap(theme.spacing.md)
                                .child(
                                    div()
                                        .w(px(120.))
                                        .child(Label::new(name).color(TextColor::Muted)),
                                )
                                .child(Label::new(value).mono())
                        }),
                    ),
                ),
            )
    }

    fn input_panel(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = cx.theme().clone();
        let noise = NOISE[self.noise];
        let thumbnails = EXAMPLES.iter().enumerate().map(|(index, example)| {
            let selected = self.example == *example;
            Card::new()
                .id(SharedString::from(format!("example-{index}")))
                .w(px(68.))
                .p(theme.spacing.xs)
                .gap(theme.spacing.xs)
                .items_center()
                .cursor_pointer()
                .when(selected, |card| card.border_color(theme.colors.accent))
                .child(Self::well(cx).size(px(64.)).child(Heatmap::new(
                    SIDE,
                    SIDE,
                    image(example.class, example.seed, 0.),
                )))
                .child(
                    Label::new(CLASSES[example.class])
                        .size(TextSize::XSmall)
                        .color(if selected {
                            TextColor::Default
                        } else {
                            TextColor::Muted
                        }),
                )
                .on_click(cx.listener({
                    let example = *example;
                    move |this, _, _, cx| {
                        this.example = example;
                        if this.live {
                            this.submit();
                        }
                        cx.notify();
                    }
                }))
        });
        Self::panel("Input", "A 24×24 image from the validation set", cx)
            .flex_1()
            .min_w(px(MIN_PANEL_WIDTH))
            .child(
                div()
                    .flex()
                    .justify_center()
                    .child(Self::well(cx).size(px(240.)).child(Heatmap::new(
                        SIDE,
                        SIDE,
                        image(self.example.class, self.example.seed, noise),
                    ))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.md)
                    .child(Label::new("Noise").color(TextColor::Muted))
                    .child(
                        SegmentedControl::new(
                            "noise",
                            NOISE.map(|noise| {
                                Segment::label(if noise == 0. {
                                    "Clean".to_string()
                                } else {
                                    format!("σ {noise}")
                                })
                            }),
                            self.noise,
                        )
                        .size(ControlSize::Small)
                        .on_select(cx.listener(
                            |this, index: &usize, _, cx| {
                                this.noise = *index;
                                if this.live {
                                    this.submit();
                                }
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(theme.spacing.sm)
                    .child(
                        Button::new("shuffle", "Random image")
                            .icon(IconName::Refresh)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let mut random = Random(this.calls * 7 + 1);
                                this.example = Example {
                                    class: (random.uniform() * CLASSES.len() as f32) as usize
                                        % CLASSES.len(),
                                    seed: 1000 + this.calls,
                                };
                                this.calls += 1;
                                if this.live {
                                    this.submit();
                                }
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("submit", "Submit")
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.submit();
                                cx.notify();
                            })),
                    ),
            )
            .child(SectionHeader::caption("Examples"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(theme.spacing.sm)
                    .children(thumbnails),
            )
    }

    fn output_panel(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = cx.theme().clone();
        let model = &self.runs[self.model];
        let latest = model.checkpoint();
        let mut panel = Self::panel(
            "Output",
            "The run's latest checkpoint, asked for a label",
            cx,
        )
        .flex_1()
        .min_w(px(MIN_PANEL_WIDTH))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.sm)
                .child(
                    SegmentedControl::new(
                        "model",
                        RUNS.iter().map(|spec| Segment::label(spec.name)),
                        self.model,
                    )
                    .size(ControlSize::Small)
                    .on_select(cx.listener(|this, index: &usize, _, cx| {
                        this.model = *index;
                        this.submit();
                        cx.notify();
                    })),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme.spacing.lg)
                        .child(
                            Switch::new("live", self.live)
                                .label("Live")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.live = !this.live;
                                    if this.live {
                                        this.submit();
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(Label::new("Temperature").color(TextColor::Muted))
                        .child(
                            SegmentedControl::new(
                                "temperature",
                                TEMPERATURES.map(|t| Segment::label(format!("{t}"))),
                                self.temperature,
                            )
                            .size(ControlSize::Small)
                            .on_select(cx.listener(
                                |this, index: &usize, _, cx| {
                                    this.temperature = *index;
                                    this.submit();
                                    cx.notify();
                                },
                            )),
                        ),
                ),
        );
        let Some(result) = &self.result else {
            return panel;
        };
        let top = result.top();
        let right = top == result.example.class;
        let mut ranked: Vec<usize> = (0..CLASSES.len()).collect();
        ranked.sort_by(|a, b| result.probabilities[*b].total_cmp(&result.probabilities[*a]));
        let stale = result.checkpoint != latest || result.run != self.model;
        panel =
            panel
                .child(
                    div()
                        .flex()
                        .items_end()
                        .gap(theme.spacing.md)
                        .child(Label::new(CLASSES[top]).size(TextSize::Display))
                        .child(
                            div().pb(theme.spacing.xs).child(
                                Label::new(format!("{:.1}%", result.probabilities[top] * 100.))
                                    .size(TextSize::Large)
                                    .mono()
                                    .color(TextColor::Muted),
                            ),
                        )
                        .child(div().flex_1())
                        .child(div().pb(theme.spacing.xs).child(if right {
                            Badge::new("Correct")
                                .tone(Tone::Success)
                                .icon(IconName::Check)
                        } else {
                            Badge::new(format!("Wrong · is {}", CLASSES[result.example.class]))
                                .tone(Tone::Danger)
                                .icon(IconName::Close)
                        })),
                )
                .child(div().flex().flex_col().gap(theme.spacing.sm).children(
                    ranked.into_iter().take(5).enumerate().map(|(rank, class)| {
                        let probability = result.probabilities[class];
                        div()
                            .flex()
                            .items_center()
                            .gap(theme.spacing.md)
                            .child(div().w(px(72.)).child(
                                Label::new(CLASSES[class]).size(TextSize::Small).color(
                                    if rank == 0 {
                                        TextColor::Default
                                    } else {
                                        TextColor::Muted
                                    },
                                ),
                            ))
                            .child(
                                div().flex_1().child(
                                    ProgressBar::new(probability)
                                        .id(SharedString::from(format!("p-{rank}")))
                                        .tone(if rank == 0 {
                                            Tone::Accent
                                        } else {
                                            Tone::Neutral
                                        })
                                        .height(px(8.)),
                                ),
                            )
                            .child(
                                div().w(px(56.)).flex().justify_end().child(
                                    Label::new(format!("{:.1}%", probability * 100.))
                                        .size(TextSize::Small)
                                        .mono()
                                        .color(TextColor::Muted),
                                ),
                            )
                    }),
                ))
                .child(
                    Label::new(format!(
                        "{} · ckpt-{} · {:.1} ms{}",
                        RUNS[result.run].name,
                        result.checkpoint,
                        result.latency_ms,
                        if stale {
                            format!(" · newer checkpoint: ckpt-{latest}")
                        } else {
                            String::new()
                        }
                    ))
                    .size(TextSize::Small)
                    .mono()
                    .color(if stale {
                        TextColor::Warning
                    } else {
                        TextColor::Faint
                    }),
                );
        panel
    }

    /// How sure every checkpoint so far is of the input's true class, so the model can be seen
    /// learning it.
    fn confidence_panel(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = cx.theme().clone();
        let noise = NOISE[self.noise];
        let temperature = TEMPERATURES[self.temperature];
        let class = self.example.class;
        let mut plot = LinePlot::new()
            .x_range(0., TOTAL_STEPS as f32)
            .y_range(0., 1.)
            .x_label("checkpoint step")
            .y_label(SharedString::from(format!("P({})", CLASSES[class])))
            .grid(self.grid)
            .legend(true);
        for (index, run) in self.runs.iter().enumerate() {
            if !run.visible && index != self.model {
                continue;
            }
            let points: Vec<_> = (0..=run.checkpoint())
                .step_by(CHECKPOINT_EVERY)
                .map(|step| {
                    let p = predict(index, step, self.example, noise, temperature)[class];
                    (step as f32, p)
                })
                .collect();
            let series = Series::line(points)
                .style(SeriesStyle::LineMarkers)
                .color(self.color(index, cx))
                .name(run.spec.name);
            plot = plot.series(if index == self.model {
                series
            } else {
                series.faint()
            });
        }
        Self::panel(
            "Across checkpoints",
            "The true class's probability for this input, at every checkpoint written so far",
            cx,
        )
        .flex_1()
        .min_w(px(MIN_PANEL_WIDTH))
        .child(Self::well(cx).h(px(220.)).child(plot))
        .child(
            Label::new(format!(
                "Checkpoints are written every {CHECKPOINT_EVERY} steps; with Live on, the \
                 output follows each new one."
            ))
            .size(TextSize::Small)
            .color(TextColor::Faint),
        )
        .gap(theme.spacing.md)
    }

    fn history_panel(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = cx.theme().clone();
        let rows = self.log.iter().map(|call| {
            let top = call.top();
            let right = top == call.example.class;
            div()
                .flex()
                .items_center()
                .gap(theme.spacing.md)
                .py(theme.spacing.xs)
                .border_b_1()
                .border_color(theme.colors.separator)
                .child(Self::well(cx).size(px(28.)).flex_none().child(Heatmap::new(
                    SIDE,
                    SIDE,
                    image(call.example.class, call.example.seed, call.noise),
                )))
                .child(
                    div().flex_1().min_w_0().child(
                        Label::new(format!(
                            "{} → {} {:.0}%",
                            CLASSES[call.example.class],
                            CLASSES[top],
                            call.probabilities[top] * 100.
                        ))
                        .size(TextSize::Small)
                        .truncate(),
                    ),
                )
                .child(
                    Label::new(format!(
                        "{} · ckpt-{}",
                        RUNS[call.run].name, call.checkpoint
                    ))
                    .size(TextSize::Small)
                    .mono()
                    .color(TextColor::Faint),
                )
                .child(
                    Badge::new(if right { "right" } else { "wrong" })
                        .tone(if right { Tone::Success } else { Tone::Danger })
                        .dot(),
                )
        });
        Self::panel("History", "Earlier calls, newest first", cx)
            .flex_1()
            .min_w(px(MIN_PANEL_WIDTH))
            .when(self.log.is_empty(), |panel| {
                panel.child(
                    Label::new("Nothing yet. Submit an image, or leave Live on while it trains.")
                        .size(TextSize::Small)
                        .color(TextColor::Faint),
                )
            })
            .child(div().flex().flex_col().children(rows))
    }

    fn inference(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .flex_col()
            .gap(cx.theme().spacing.lg)
            .child(
                Self::row(cx)
                    .child(self.input_panel(cx))
                    .child(self.output_panel(cx)),
            )
            .child(
                Self::row(cx)
                    .child(self.confidence_panel(cx))
                    .child(self.history_panel(cx)),
            )
    }

    fn status(&self) -> StatusBar {
        let step = self.step();
        let epoch = (step / STEPS_PER_EPOCH + 1).min(TOTAL_STEPS / STEPS_PER_EPOCH);
        let running = self.runs.iter().filter(|run| run.running()).count();
        let diverged = self
            .runs
            .iter()
            .find_map(|run| Some((run.spec.name, run.diverged?)));
        let (message, tone) = if self.finished() {
            ("Training finished".to_string(), Tone::Success)
        } else if let Some((name, at)) = diverged {
            (
                format!(
                    "{name} diverged at step {} (loss NaN) · {running} runs still training",
                    grouped(at)
                ),
                Tone::Warning,
            )
        } else if self.playing {
            (
                format!(
                    "Training {running} runs · epoch {epoch} of {}",
                    TOTAL_STEPS / STEPS_PER_EPOCH
                ),
                Tone::Neutral,
            )
        } else {
            ("Paused".to_string(), Tone::Neutral)
        };
        StatusBar::new()
            .message(message)
            .tone(tone)
            .busy(self.playing && running > 0)
            .trailing(
                Label::new(format!("{} steps/s", STEPS_PER_SECOND * SPEEDS[self.speed]))
                    .size(TextSize::Small)
                    .mono()
                    .color(TextColor::Muted),
            )
            .trailing(
                Label::new("GPU 0 · RTX 4090 (simulated)")
                    .size(TextSize::Small)
                    .color(TextColor::Faint),
            )
    }
}

impl Render for TrainingDashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The runs sidebar goes first when the window narrows, then the speed and step readout.
        let width = window.viewport_size().width;
        let sidebar = width >= px(1180.);
        let wide = width >= px(1240.);
        if self.playing && !self.finished() {
            let now = Instant::now();
            if let Some(last) = self.last_frame {
                // A long gap (the window was hidden) counts as one frame, not a jump.
                let step = (now - last).as_secs_f32().min(0.1);
                self.carry += step * STEPS_PER_SECOND * SPEEDS[self.speed];
                while self.carry >= 1. {
                    self.carry -= 1.;
                    self.advance_all();
                }
            }
            self.last_frame = Some(now);
            window.request_animation_frame();
        } else {
            self.last_frame = None;
        }
        let theme = cx.theme().clone();
        let switch = common::theme_switch(self, cx);
        let page = match self.page {
            0 => self.scalars(cx).into_any_element(),
            1 => self.distributions(cx).into_any_element(),
            2 => self.runs_table(cx).into_any_element(),
            _ => self.inference(cx).into_any_element(),
        };
        WindowFrame::new()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .size_full()
            .flex()
            .flex_col()
            .child(
                TitleBar::new()
                    .title("Training")
                    .child(
                        Tabs::new(
                            "page",
                            ["Scalars", "Distributions", "Runs", "Inference"],
                            self.page,
                        )
                        .on_select(cx.listener(
                            |this, index: &usize, _, cx| {
                                this.page = *index;
                                cx.notify();
                            },
                        )),
                    )
                    .child({
                        let (label, icon) = if self.playing {
                            ("Pause", IconName::Pause)
                        } else {
                            ("Resume", IconName::Play)
                        };
                        let button = if wide {
                            Button::new("play", label)
                                .size(ControlSize::Small)
                                .icon(icon)
                        } else {
                            Button::icon_only("play", icon).tooltip(label)
                        };
                        button.on_click(cx.listener(|this, _, _, cx| {
                            this.playing = !this.playing;
                            cx.notify();
                        }))
                    })
                    .child(
                        Button::icon_only("restart", IconName::Refresh)
                            .tooltip("Restart every run from step 0")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.restart();
                                cx.notify();
                            })),
                    )
                    .when(wide, |bar| {
                        bar.child(
                            SegmentedControl::new(
                                "speed",
                                SPEEDS.map(|speed| Segment::label(format!("{speed}×"))),
                                self.speed,
                            )
                            .size(ControlSize::Small)
                            .on_select(cx.listener(
                                |this, index: &usize, _, cx| {
                                    this.speed = *index;
                                    cx.notify();
                                },
                            )),
                        )
                    })
                    .trailing(switch),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(sidebar, |row| row.child(self.runs_sidebar(cx)))
                    .child(
                        div()
                            .id("page")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(theme.spacing.lg)
                            .p(theme.spacing.lg)
                            .overflow_y_scroll()
                            .bg(theme.colors.background)
                            .child(page),
                    ),
            )
            .child(self.status())
    }
}

/// Narrowest a panel gets before its row wraps.
const MIN_PANEL_WIDTH: f32 = 360.;

fn main() {
    let page = match common::argument("--page").as_deref() {
        Some("distributions") => 1,
        Some("runs") => 2,
        Some("inference") => 3,
        _ => 0,
    };
    // `--step N` starts with every run trained that far, for screenshots.
    let start = common::argument("--step")
        .and_then(|step| step.parse().ok())
        .unwrap_or(0)
        .min(TOTAL_STEPS);
    common::run("Training", 1600., 1000., move |choice, _| {
        TrainingDashboard::new(choice, page, start)
    });
}
