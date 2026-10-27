//! A mock numerical workbench in the spirit of MATLAB: a workspace of variables, and two
//! figures of plots computed from textbook physics — a damped oscillator, thermal speeds, a
//! limit cycle, a Fourier series and a dipole's potential in 2-D; a surface, the Lorenz
//! attractor and a point cloud in 3-D. Drag a 3-D plot to turn it.
//!
//! ```text
//! cargo run --example plot_lab -- [--figure 2d|3d] [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use std::f32::consts::{PI, TAU};
use std::time::Duration;

use atelier::plot::{
    Colorbar, FieldPlot, Histogram, LinePlot, Plot3d, Series, SeriesStyle, Shading,
};
use atelier::{
    ActiveTheme, Appearance, Badge, Button, ControlSize, Direction, Divider, Edge, IconName, Knob,
    Label, ListItem, SectionHeader, Segment, SegmentedControl, Sidebar, Slider, Switch, Tabs,
    TextColor, TextSize, TitleBar, Tone, WindowFrame,
};
use common::Themed;
use gpui::{
    Animation, AnimationExt, App, Context, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement, Pixels, Point, Render, SharedString, Styled, Window, div,
    prelude::*, px,
};

/// One turn of the turntable.
const TURN: Duration = Duration::from_secs(24);

/// The views the inspector offers, as MATLAB's `view` arguments.
const VIEWS: [(&str, f32, f32); 4] = [
    ("3-D", -37.5, 30.),
    ("Top", 0., 90.),
    ("Front", 0., 0.),
    ("Side", 90., 0.),
];

/// A small deterministic generator, so every run draws the same samples.
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

/// Displacement of a unit oscillator released from 1 at rest, at damping ratio `zeta`.
fn oscillator(zeta: f32, omega: f32, t: f32) -> f32 {
    if zeta < 1. {
        let damped = omega * (1. - zeta * zeta).sqrt();
        (-zeta * omega * t).exp()
            * ((damped * t).cos() + zeta * omega / damped * (damped * t).sin())
    } else {
        (1. + omega * t) * (-omega * t).exp()
    }
}

/// Molecular speeds in a gas at thermal equilibrium: the length of a 3-D normal velocity.
/// Nitrogen at 300 K, where each component has σ = √(kT/m) ≈ 298 m/s.
const SIGMA: f32 = 298.;

fn thermal_speeds(count: usize) -> Vec<f32> {
    let mut random = Random(7);
    (0..count)
        .map(|_| {
            let (x, y, z) = (random.normal(), random.normal(), random.normal());
            SIGMA * (x * x + y * y + z * z).sqrt()
        })
        .collect()
}

/// The Maxwell–Boltzmann speed density those samples follow.
fn maxwell(v: f32) -> f32 {
    (2. / PI).sqrt() * v * v / SIGMA.powi(3) * (-v * v / (2. * SIGMA * SIGMA)).exp()
}

/// A fourth-order Runge–Kutta step of `f` from `y` by `h`.
fn rk4<const N: usize>(f: impl Fn([f32; N]) -> [f32; N], y: [f32; N], h: f32) -> [f32; N] {
    let add = |a: [f32; N], b: [f32; N], s: f32| std::array::from_fn(|i| a[i] + b[i] * s);
    let k1 = f(y);
    let k2 = f(add(y, k1, h / 2.));
    let k3 = f(add(y, k2, h / 2.));
    let k4 = f(add(y, k3, h));
    std::array::from_fn(|i| y[i] + h / 6. * (k1[i] + 2. * k2[i] + 2. * k3[i] + k4[i]))
}

/// A trajectory of the Van der Pol oscillator from `start`, in the phase plane.
fn van_der_pol(start: (f32, f32), mu: f32) -> Vec<(f32, f32)> {
    let field = |[x, v]: [f32; 2]| [v, mu * (1. - x * x) * v - x];
    let mut state = [start.0, start.1];
    let mut points = vec![start];
    for _ in 0..1600 {
        state = rk4(field, state, 0.01);
        points.push((state[0], state[1]));
    }
    points
}

/// The Lorenz attractor at its classic parameters, after the transient has died away.
fn lorenz() -> Vec<[f32; 3]> {
    let field = |[x, y, z]: [f32; 3]| [10. * (y - x), x * (28. - z) - y, x * y - 8. / 3. * z];
    let mut state = [1., 1., 1.];
    (0..6400)
        .map(|_| {
            state = rk4(field, state, 0.008);
            state
        })
        .skip(400)
        .collect()
}

/// MATLAB's `peaks`: three Gaussians, a sum of shapes everyone recognises.
fn peaks(x: f32, y: f32) -> f32 {
    3. * (1. - x).powi(2) * (-x * x - (y + 1.).powi(2)).exp()
        - 10. * (x / 5. - x.powi(3) - y.powi(5)) * (-x * x - y * y).exp()
        - 1. / 3. * (-(x + 1.).powi(2) - y * y).exp()
}

const PEAKS: usize = 41;

fn peaks_grid() -> Vec<f32> {
    (0..PEAKS * PEAKS)
        .map(|index| {
            let (column, row) = (index % PEAKS, index / PEAKS);
            let at = |i: usize| -3. + 6. * i as f32 / (PEAKS - 1) as f32;
            peaks(at(column), at(row))
        })
        .collect()
}

/// Three Gaussian clusters, as a classifier's training set might look.
fn clusters() -> [Vec<[f32; 3]>; 3] {
    let mut random = Random(11);
    [[-1.5, -1., 0.5], [1.2, 1.4, -0.4], [1.4, -1.2, 1.6]].map(|centre| {
        (0..180)
            .map(|_| {
                [
                    centre[0] + 0.55 * random.normal(),
                    centre[1] + 0.55 * random.normal(),
                    centre[2] + 0.45 * random.normal(),
                ]
            })
            .collect()
    })
}

const DIPOLE: (usize, usize) = (96, 72);

/// The potential of a +q and −q pair at x = ∓0.6, clamped near the charges.
fn dipole_potential(x: f32, y: f32) -> f32 {
    let r = |cx: f32| ((x - cx).powi(2) + y * y).sqrt().max(0.05);
    (1. / r(-0.6) - 1. / r(0.6)).clamp(-5., 5.)
}

fn dipole_grid() -> Vec<f32> {
    let (columns, rows) = DIPOLE;
    (0..columns * rows)
        .map(|index| {
            let x = -2. + 4. * (index % columns) as f32 / (columns - 1) as f32;
            let y = -1.5 + 3. * (index / columns) as f32 / (rows - 1) as f32;
            dipole_potential(x, y)
        })
        .collect()
}

/// The first `terms` odd harmonics of a unit square wave.
fn square_partial(terms: usize, t: f32) -> f32 {
    (0..terms)
        .map(|k| {
            let n = (2 * k + 1) as f32;
            4. / (PI * n) * (n * t).sin()
        })
        .sum()
}

struct PlotLab {
    choice: (Direction, Appearance),
    figure: usize,
    grid: bool,
    legend: bool,
    terms: usize,
    azimuth: f32,
    elevation: f32,
    shading: Shading,
    spin: bool,
    drag: Option<Point<Pixels>>,
    speeds: Vec<f32>,
    cycles: Vec<Vec<(f32, f32)>>,
    attractor: Vec<[f32; 3]>,
    surface: Vec<f32>,
    clusters: [Vec<[f32; 3]>; 3],
    dipole: Vec<f32>,
}

impl Themed for PlotLab {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

impl PlotLab {
    fn new(choice: (Direction, Appearance), figure: usize) -> Self {
        Self {
            choice,
            figure,
            grid: true,
            legend: true,
            terms: 5,
            azimuth: VIEWS[0].1,
            elevation: VIEWS[0].2,
            shading: Shading::Faceted,
            spin: false,
            drag: None,
            speeds: thermal_speeds(6000),
            cycles: [(0.1, 0.), (3.5, 2.5), (-3., -3.5)]
                .into_iter()
                .map(|start| van_der_pol(start, 1.2))
                .collect(),
            attractor: lorenz(),
            surface: peaks_grid(),
            clusters: clusters(),
            dipole: dipole_grid(),
        }
    }

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

    fn well(cx: &App) -> gpui::Div {
        let theme = cx.theme();
        div()
            .rounded(theme.shape.radius_sm)
            .bg(theme.colors.sunken)
            .overflow_hidden()
    }

    fn row(cx: &App) -> gpui::Div {
        div().flex().gap(cx.theme().spacing.lg)
    }

    fn workspace(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let variables = [
            ("t", "1×600 double"),
            ("x", "3×600 double"),
            ("v", "6000×1 double"),
            ("orbits", "3×1 cell"),
            ("V", "72×96 double"),
            ("Z", "41×41 double"),
            ("lorenz", "6000×3 double"),
        ];
        let history = [
            "plot(t, x)",
            "histogram(v, 40)",
            "stem(n, b)",
            "contour(X, Y, V, 10)",
            "surf(peaks(41))",
            "plot3(L(:,1), L(:,2), L(:,3))",
        ];
        Sidebar::new(Edge::Left)
            .w(px(232.))
            .p(theme.spacing.sm)
            .gap(theme.spacing.xxs)
            .child(
                div()
                    .px(theme.spacing.sm)
                    .py(theme.spacing.xs)
                    .child(Label::caption("Workspace")),
            )
            .children(variables.into_iter().map(|(name, kind)| {
                ListItem::new(SharedString::from(format!("var-{name}")), name)
                    .icon(IconName::Code)
                    .description(kind)
            }))
            .child(
                div()
                    .px(theme.spacing.sm)
                    .pt(theme.spacing.lg)
                    .pb(theme.spacing.xs)
                    .child(Label::caption("Command history")),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.xs)
                    .px(theme.spacing.sm)
                    .children(history.into_iter().map(|command| {
                        Label::new(format!(">> {command}"))
                            .size(TextSize::Small)
                            .mono()
                            .color(TextColor::Muted)
                            .truncate()
                    })),
            )
    }

    fn oscillator(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let omega = TAU;
        let curve = |zeta: f32| {
            (0..600)
                .map(|step| {
                    let t = step as f32 / 599. * 4.;
                    (t, oscillator(zeta, omega, t))
                })
                .collect::<Vec<_>>()
        };
        let mut random = Random(3);
        let measured: Vec<_> = (0..32)
            .map(|step| {
                let t = step as f32 / 31. * 4.;
                (t, oscillator(0.15, omega, t) + 0.04 * random.normal())
            })
            .collect();
        let envelope: Vec<_> = (0..120)
            .map(|step| {
                let t = step as f32 / 119. * 4.;
                (t, (-0.15 * omega * t).exp())
            })
            .collect();
        Self::panel(
            "Damped oscillator",
            "x(t) for three damping ratios, f₀ = 1 Hz, released from rest",
            cx,
        )
        .flex_1()
        .min_w_0()
        .child(
            Self::well(cx).h(px(260.)).child(
                LinePlot::new()
                    .series(Series::line(curve(0.04)).name("ζ = 0.04"))
                    .series(
                        Series::line(measured)
                            .style(SeriesStyle::LineMarkers)
                            .name("ζ = 0.15, measured"),
                    )
                    .series(Series::line(curve(1.)).name("ζ = 1, critical"))
                    .series(
                        Series::line(envelope)
                            .color(cx.theme().colors.series[1])
                            .faint(),
                    )
                    .x_label("t [s]")
                    .y_label("x [m]")
                    .grid(self.grid)
                    .legend(self.legend),
            ),
        )
    }

    fn speeds(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mean = self.speeds.iter().sum::<f32>() / self.speeds.len() as f32;
        let theory: Vec<_> = (0..200)
            .map(|step| {
                let v = step as f32 / 199. * 1400.;
                (v, maxwell(v))
            })
            .collect();
        Self::panel(
            "Thermal speeds",
            "6000 N₂ molecules at 300 K against the Maxwell–Boltzmann density",
            cx,
        )
        .w(px(440.))
        .flex_none()
        .child(
            Self::well(cx).h(px(260.)).child(
                Histogram::new(self.speeds.clone())
                    .bins(40)
                    .range(0., 1400.)
                    .density()
                    .name("Simulated")
                    .overlay(theory, "Maxwell–Boltzmann")
                    .x_label("Speed [m/s]"),
            ),
        )
        .child(
            div()
                .flex()
                .gap(cx.theme().spacing.sm)
                .child(Badge::new(format!("mean {mean:.0} m/s")))
                .child(Badge::new("theory 476 m/s"))
                .child(Badge::new("χ² p = 0.62").tone(Tone::Success).dot()),
        )
    }

    fn phase(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let starts: Vec<_> = self.cycles.iter().map(|cycle| cycle[0]).collect();
        let mut plot = LinePlot::new();
        for cycle in &self.cycles {
            plot = plot.series(Series::line(cycle.clone()));
        }
        Self::panel(
            "Limit cycle",
            "Van der Pol, μ = 1.2: three starts settle on one orbit",
            cx,
        )
        .flex_1()
        .min_w_0()
        .child(
            Self::well(cx).h(px(280.)).child(
                plot.series(Series::markers(starts).color(cx.theme().colors.text))
                    .x_label("x")
                    .y_label("dx/dt")
                    .grid(self.grid),
            ),
        )
    }

    fn fourier(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let terms = self.terms;
        let coefficients: Vec<_> = (1..=15)
            .map(|n| {
                let b = if n % 2 == 1 { 4. / (PI * n as f32) } else { 0. };
                (n as f32, b)
            })
            .collect();
        let square: Vec<_> = (0..=4)
            .map(|half| {
                let t = half as f32 * PI - PI;
                (t, if half % 2 == 0 { -1. } else { 1. })
            })
            .collect();
        let sum: Vec<_> = (0..500)
            .map(|step| {
                let t = -PI + TAU * 2. * step as f32 / 499.;
                (t, square_partial(terms, t))
            })
            .collect();
        Self::panel(
            "Fourier series",
            "A square wave's sine coefficients, and the sum of the first few",
            cx,
        )
        .w(px(440.))
        .flex_none()
        .child(
            Self::row(cx)
                .gap(theme.spacing.sm)
                .h(px(232.))
                .child(
                    Self::well(cx).w(px(160.)).h_full().child(
                        LinePlot::new()
                            .series(Series::stems(coefficients))
                            .x_range(0., 16.)
                            .x_label("n")
                            .y_label("bₙ")
                            .grid(self.grid),
                    ),
                )
                .child(
                    Self::well(cx).flex_1().h_full().child(
                        LinePlot::new()
                            .series(Series::line(square).style(SeriesStyle::Steps).faint())
                            .series(Series::line(sum).color(theme.colors.series[2]))
                            .x_range(-PI, 3. * PI)
                            .y_range(-1.5, 1.5)
                            .x_label("t [rad]")
                            .grid(self.grid),
                    ),
                ),
        )
        .child(
            SegmentedControl::new(
                "terms",
                [1, 3, 5, 15].map(|count| Segment::label(format!("{count} terms"))),
                [1, 3, 5, 15]
                    .iter()
                    .position(|count| *count == terms)
                    .unwrap_or(2),
            )
            .size(ControlSize::Small)
            .on_select(cx.listener(|this, index: &usize, _, cx| {
                this.terms = [1, 3, 5, 15][*index];
                cx.notify();
            })),
        )
    }

    fn dipole(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        Self::panel(
            "Electric dipole",
            "Potential of +q and −q, with nine equipotentials",
            cx,
        )
        .flex_1()
        .min_w_0()
        .child(
            Self::row(cx)
                .gap(theme.spacing.sm)
                .h(px(380.))
                .child(
                    Self::well(cx).flex_1().h_full().child(
                        FieldPlot::new(DIPOLE.0, DIPOLE.1, self.dipole.clone())
                            .x_range(-2., 2.)
                            .y_range(-1.5, 1.5)
                            .value_range(-3., 3.)
                            .signed()
                            .contours(9)
                            .x_label("x [m]")
                            .y_label("y [m]"),
                    ),
                )
                .child(
                    div()
                        .w(px(64.))
                        .h_full()
                        .child(Colorbar::new(-3., 3.).signed().label("V [kq/m]")),
                ),
        )
    }

    fn cuts(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let cut = |y: f32| {
            (0..400)
                .map(|step| {
                    let x = -2. + 4. * step as f32 / 399.;
                    (x, dipole_potential(x, y))
                })
                .collect::<Vec<_>>()
        };
        Self::panel(
            "Cuts through the field",
            "V along three lines parallel to the axis",
            cx,
        )
        .w(px(440.))
        .flex_none()
        .child(
            Self::well(cx).h(px(380.)).child(
                LinePlot::new()
                    .series(Series::line(cut(0.2)).name("y = 0.2 m"))
                    .series(Series::line(cut(0.5)).name("y = 0.5 m"))
                    .series(Series::line(cut(1.)).name("y = 1 m"))
                    .y_range(-5., 5.)
                    .x_label("x [m]")
                    .y_label("V [kq/m]")
                    .grid(self.grid)
                    .legend(self.legend),
            ),
        )
    }

    fn figure_2d(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .flex_col()
            .gap(cx.theme().spacing.lg)
            .child(
                Self::row(cx)
                    .child(self.oscillator(cx))
                    .child(self.speeds(cx)),
            )
            .child(Self::row(cx).child(self.phase(cx)).child(self.fourier(cx)))
            .child(Self::row(cx).child(self.dipole(cx)).child(self.cuts(cx)))
    }

    /// A 3-D plot in a well that turns when dragged, and on the turntable when it is on.
    fn turnable(
        &self,
        id: &'static str,
        plot: Plot3d,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let azimuth = self.azimuth;
        let plot = plot
            .view(self.azimuth, self.elevation)
            .shading(self.shading);
        let body = if self.spin {
            plot.with_animation(id, Animation::new(TURN).repeat(), move |plot, t| {
                plot.azimuth(azimuth + 360. * t)
            })
            .into_any_element()
        } else {
            plot.into_any_element()
        };
        Self::well(cx)
            .id(id)
            .cursor_grab()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, _| {
                    this.drag = Some(event.position);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = None),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                let Some(last) = this.drag.filter(|_| event.dragging()) else {
                    return;
                };
                let delta = event.position - last;
                this.azimuth =
                    (this.azimuth - f32::from(delta.x) * 0.4 + 180.).rem_euclid(360.) - 180.;
                this.elevation = (this.elevation + f32::from(delta.y) * 0.4).clamp(-90., 90.);
                this.drag = Some(event.position);
                cx.notify();
            }))
            .child(body)
    }

    fn figure_3d(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let (low, high) = self
            .surface
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), z| (lo.min(*z), hi.max(*z)));
        let surface = Plot3d::new()
            .surface(PEAKS, PEAKS, self.surface.clone(), (-3., 3.), (-3., 3.))
            .labels("x", "y", "z");
        let mut cloud = Plot3d::new().labels("x₁", "x₂", "x₃");
        for (index, cluster) in self.clusters.iter().enumerate() {
            cloud = cloud.scatter(cluster.clone(), Some(theme.colors.series[index]));
        }
        let attractor = Plot3d::new()
            .curve(self.attractor.clone(), None)
            .labels("x", "y", "z");
        div()
            .flex()
            .flex_col()
            .gap(theme.spacing.lg)
            .child(
                Self::panel("peaks(41)", "surf of three Gaussians on a 41 × 41 grid", cx).child(
                    Self::row(cx)
                        .gap(theme.spacing.sm)
                        .h(px(440.))
                        .child(self.turnable("surface", surface, cx).flex_1().h_full())
                        .child(
                            div()
                                .w(px(64.))
                                .h_full()
                                .child(Colorbar::new(low, high).label("z")),
                        ),
                ),
            )
            .child(
                Self::row(cx)
                    .child(
                        Self::panel(
                            "Lorenz attractor",
                            "σ = 10, ρ = 28, β = 8/3; colour runs with time",
                            cx,
                        )
                        .flex_1()
                        .min_w_0()
                        .child(self.turnable("lorenz", attractor, cx).h(px(340.))),
                    )
                    .child(
                        Self::panel("Clusters", "scatter3 of three labelled classes", cx)
                            .flex_1()
                            .min_w_0()
                            .child(self.turnable("cloud", cloud, cx).h(px(340.))),
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
        let sidebar = Sidebar::new(Edge::Right)
            .w(px(280.))
            .p(theme.spacing.lg)
            .gap(theme.spacing.xl);
        let axes = group("Axes")
            .child(
                Switch::new("grid", self.grid)
                    .label("Grid")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.grid = !this.grid;
                        cx.notify();
                    })),
            )
            .child(
                Switch::new("legend", self.legend)
                    .label("Legend")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.legend = !this.legend;
                        cx.notify();
                    })),
            );
        if self.figure == 0 {
            return sidebar
                .child(axes)
                .child(
                    group("Oscillator").child(
                        div()
                            .flex()
                            .gap(theme.spacing.lg)
                            .child(Knob::new(0.5).label("f₀").readout("1.00 Hz"))
                            .child(Knob::new(0.15).label("ζ").readout("0.15"))
                            .child(Knob::new(0.4).label("x₀").readout("1.0 m")),
                    ),
                )
                .child(
                    group("Gas")
                        .child(Slider::new(0.3).label("T").readout("300 K"))
                        .child(Slider::new(0.28).label("m").readout("28 u")),
                );
        }
        let view = VIEWS
            .iter()
            .position(|(_, azimuth, elevation)| {
                *azimuth == self.azimuth && *elevation == self.elevation
            })
            .unwrap_or(usize::MAX);
        sidebar
            .child(
                group("View")
                    .child(
                        SegmentedControl::new(
                            "view",
                            VIEWS.map(|(label, _, _)| Segment::label(label)),
                            view,
                        )
                        .size(ControlSize::Small)
                        .on_select(cx.listener(
                            |this, index: &usize, _, cx| {
                                (_, this.azimuth, this.elevation) = VIEWS[*index];
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        Slider::new((self.azimuth + 180.) / 360.)
                            .label("Azimuth")
                            .readout(format!("{:.1}°", self.azimuth).replace('-', "−")),
                    )
                    .child(
                        Slider::new((self.elevation + 90.) / 180.)
                            .label("Elevation")
                            .readout(format!("{:.1}°", self.elevation).replace('-', "−")),
                    )
                    .child(
                        Switch::new("spin", self.spin)
                            .label("Turntable")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.spin = !this.spin;
                                cx.notify();
                            })),
                    )
                    .child(
                        Label::new("Drag a plot to turn it. All three share one view.")
                            .size(TextSize::Small)
                            .color(TextColor::Faint),
                    ),
            )
            .child(
                group("Surface").child(
                    SegmentedControl::new(
                        "shading",
                        [
                            Segment::label("Faceted"),
                            Segment::label("Smooth"),
                            Segment::label("Mesh"),
                        ],
                        match self.shading {
                            Shading::Faceted => 0,
                            Shading::Smooth => 1,
                            Shading::Mesh => 2,
                        },
                    )
                    .size(ControlSize::Small)
                    .on_select(cx.listener(|this, index: &usize, _, cx| {
                        this.shading = [Shading::Faceted, Shading::Smooth, Shading::Mesh][*index];
                        cx.notify();
                    })),
                ),
            )
    }
}

impl Render for PlotLab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let switch = common::theme_switch(self, cx);
        let figure = if self.figure == 0 {
            self.figure_2d(cx).into_any_element()
        } else {
            self.figure_3d(cx).into_any_element()
        };
        WindowFrame::new()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .size_full()
            .flex()
            .flex_col()
            .child(
                TitleBar::new()
                    .title("Plot lab")
                    .subtitle("oscillators.m · saved")
                    .child(div().w(theme.spacing.md))
                    .child(
                        Tabs::new("figure", ["Figure 1 · 2-D", "Figure 2 · 3-D"], self.figure)
                            .on_select(cx.listener(|this, index: &usize, _, cx| {
                                this.figure = *index;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("run", "Run")
                            .size(ControlSize::Small)
                            .icon(IconName::Play),
                    )
                    .trailing(switch)
                    .trailing(Divider::vertical())
                    .trailing(Button::icon_only("export", IconName::Copy).tooltip("Copy figure")),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.workspace(cx))
                    .child(
                        div()
                            .id("figure")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(theme.spacing.lg)
                            .p(theme.spacing.lg)
                            .overflow_y_scroll()
                            .bg(theme.colors.background)
                            .child(figure),
                    )
                    .child(self.inspector(cx)),
            )
    }
}

fn main() {
    let figure = match common::argument("--figure").as_deref() {
        Some("3d") => 1,
        _ => 0,
    };
    common::run("Plot lab", 1600., 1000., move |choice, _| {
        PlotLab::new(choice, figure)
    });
}
