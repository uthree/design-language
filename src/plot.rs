//! Plots for numbers: line and scatter plots, histograms, scalar fields and 3-D views, each
//! with labelled axes, for the scientific side of a tool — a fitted curve, a parameter sweep,
//! a simulation's output.
//!
//! Where [`crate::viz`] draws sound in fixed units, these plots take data in its own units and
//! draw the axes for it: ticks at round numbers, tick labels in the mono face, an optional axis
//! title in the units people use. Like the `viz` plots, each one fills its parent — wrap it in a
//! sized `div` on a `sunken` well — and takes plain numbers.
//!
//! The 3-D view is orthographic and is turned with [`Plot3d::view`], in MATLAB's convention:
//! azimuth about the vertical axis from the −y side, elevation above the x–y plane, in degrees.

use std::f32::consts::PI;

use gpui::{
    App, Bounds, ContentMask, Font, Hsla, IntoElement, PathBuilder, Pixels, Point, RenderOnce,
    SharedString, Styled, TextRun, Window, canvas, fill, point, px, size,
};

use crate::theme::{ActiveTheme, Theme, mix};
use crate::viz::{heat_color, polyline};

/// Round values from `min` to `max` to put ticks at, about `target` of them: steps of 1, 2,
/// 2.5 or 5 times a power of ten.
pub fn nice_ticks(min: f32, max: f32, target: usize) -> Vec<f32> {
    if max <= min || !min.is_finite() || !max.is_finite() {
        return vec![min];
    }
    let step = nice_step(min, max, target);
    let first = (min / step - 1e-4).ceil();
    (0..)
        .map(|index| (first + index as f32) * step)
        .take_while(|value| *value <= max + step * 1e-4)
        .take(64)
        .map(|value| if value.abs() < step * 1e-4 { 0. } else { value })
        .collect()
}

fn nice_step(min: f32, max: f32, target: usize) -> f32 {
    let raw = (max - min) / target.max(1) as f32;
    let magnitude = 10f32.powf(raw.log10().floor());
    [1., 2., 2.5, 5., 10.]
        .into_iter()
        .map(|multiple| multiple * magnitude)
        .find(|step| *step >= raw * 0.7)
        .unwrap_or(10. * magnitude)
}

/// `value` written with as many decimals as `step` needs, with a true minus sign; very large
/// and very small scales switch to exponent notation.
pub fn format_tick(value: f32, step: f32) -> String {
    let text = if value == 0. {
        "0".to_string()
    } else if value != 0. && (value.abs() >= 1e5 || step < 1e-4) {
        format!("{value:.1e}")
    } else {
        let mut decimals = 0;
        while decimals < 6 {
            let scaled = step * 10f32.powi(decimals);
            if (scaled - scaled.round()).abs() < 1e-3 * scaled {
                break;
            }
            decimals += 1;
        }
        format!("{value:.*}", decimals as usize)
    };
    text.replace('-', "−")
}

/// A range around `values`, padded when they are all equal.
fn extent(values: impl IntoIterator<Item = f32>) -> (f32, f32) {
    let (min, max) = values
        .into_iter()
        .filter(|value| value.is_finite())
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), value| {
            (lo.min(value), hi.max(value))
        });
    if !min.is_finite() {
        (0., 1.)
    } else if min == max {
        (min - 1., max + 1.)
    } else {
        (min, max)
    }
}

/// `range` widened to the round numbers around it, as an auto-scaled value axis is.
fn round_out((min, max): (f32, f32)) -> (f32, f32) {
    let step = nice_step(min, max, 5);
    ((min / step).floor() * step, (max / step).ceil() * step)
}

/// One axis of a 2-D plot: its range, if fixed, and its title.
#[derive(Clone, Default)]
struct Axis {
    range: Option<(f32, f32)>,
    label: Option<SharedString>,
}

/// Text shaped for painting inside a canvas.
struct Text {
    line: gpui::ShapedLine,
    height: Pixels,
}

impl Text {
    fn new(
        window: &mut Window,
        text: impl Into<SharedString>,
        font: &Font,
        size: Pixels,
        color: Hsla,
    ) -> Self {
        let text = text.into();
        let run = TextRun {
            len: text.len(),
            font: font.clone(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        Self {
            line: window.text_system().shape_line(text, size, &[run], None),
            height: size * 1.3,
        }
    }

    fn width(&self) -> Pixels {
        self.line.width
    }

    /// Paint with the box's top-left at `origin`.
    fn paint(&self, origin: Point<Pixels>, window: &mut Window, cx: &mut App) {
        self.line.paint(origin, self.height, window, cx).ok();
    }

    /// Paint centred on `at`.
    fn paint_centered(&self, at: Point<Pixels>, window: &mut Window, cx: &mut App) {
        self.paint(
            point(at.x - self.width() / 2., at.y - self.height / 2.),
            window,
            cx,
        );
    }
}

fn tick_text(window: &mut Window, theme: &Theme, value: f32, step: f32) -> Text {
    Text::new(
        window,
        format_tick(value, step),
        &theme.typography.mono,
        theme.typography.size_xs,
        theme.colors.text_faint,
    )
}

fn title_text(window: &mut Window, theme: &Theme, label: &SharedString) -> Text {
    Text::new(
        window,
        label.clone(),
        &theme.typography.ui,
        theme.typography.size_xs,
        theme.colors.text_muted,
    )
}

/// The data area of a 2-D plot and the ranges it maps.
struct Frame {
    area: Bounds<Pixels>,
    x: (f32, f32),
    y: (f32, f32),
}

impl Frame {
    fn at(&self, x: f32, y: f32) -> Point<Pixels> {
        let fx = (x - self.x.0) / (self.x.1 - self.x.0);
        let fy = (y - self.y.0) / (self.y.1 - self.y.0);
        point(
            self.area.left() + self.area.size.width * fx,
            self.area.bottom() - self.area.size.height * fy,
        )
    }

    fn mask(&self) -> Option<ContentMask<Pixels>> {
        Some(ContentMask {
            bounds: Bounds::new(
                self.area.origin - point(px(1.), px(1.)),
                self.area.size + size(px(2.), px(2.)),
            ),
        })
    }
}

/// Lay out and paint the axes — gutters, grid, zero lines, tick labels, titles — and return
/// where the data goes.
#[allow(clippy::too_many_arguments)]
fn paint_axes(
    window: &mut Window,
    cx: &mut App,
    theme: &Theme,
    bounds: Bounds<Pixels>,
    x: (f32, f32),
    y: (f32, f32),
    x_label: Option<&SharedString>,
    y_label: Option<&SharedString>,
    grid: bool,
) -> Frame {
    let colors = &theme.colors;
    let x_ticks = nice_ticks(
        x.0,
        x.1,
        ((bounds.size.width / px(90.)) as usize).clamp(2, 10),
    );
    let y_ticks = nice_ticks(
        y.0,
        y.1,
        ((bounds.size.height / px(44.)) as usize).clamp(2, 8),
    );
    let x_step = nice_step(
        x.0,
        x.1,
        ((bounds.size.width / px(90.)) as usize).clamp(2, 10),
    );
    let y_step = nice_step(
        y.0,
        y.1,
        ((bounds.size.height / px(44.)) as usize).clamp(2, 8),
    );
    let y_texts: Vec<_> = y_ticks
        .iter()
        .map(|value| tick_text(window, theme, *value, y_step))
        .collect();
    let x_texts: Vec<_> = x_ticks
        .iter()
        .map(|value| tick_text(window, theme, *value, x_step))
        .collect();
    let x_title = x_label.map(|label| title_text(window, theme, label));
    let y_title = y_label.map(|label| title_text(window, theme, label));
    let tick_height = theme.typography.size_xs * 1.3;
    let widest = y_texts
        .iter()
        .map(Text::width)
        .fold(px(0.), |a, b| a.max(b));
    let left = widest + px(14.);
    let top = px(10.)
        + y_title
            .as_ref()
            .map_or(px(0.), |title| title.height + px(2.));
    let bottom = tick_height + px(10.) + x_title.as_ref().map_or(px(0.), |title| title.height);
    let area = Bounds::new(
        point(bounds.left() + left, bounds.top() + top),
        size(
            (bounds.size.width - left - px(14.)).max(px(1.)),
            (bounds.size.height - top - bottom).max(px(1.)),
        ),
    );
    let frame = Frame { area, x, y };
    let hline = |window: &mut Window, at: Pixels, color: Hsla| {
        window.paint_quad(fill(
            Bounds::new(point(area.left(), at), size(area.size.width, px(1.))),
            color,
        ))
    };
    let vline = |window: &mut Window, at: Pixels, color: Hsla| {
        window.paint_quad(fill(
            Bounds::new(point(at, area.top()), size(px(1.), area.size.height)),
            color,
        ))
    };
    if grid {
        for value in &y_ticks {
            hline(window, frame.at(x.0, *value).y, colors.grid_faint);
        }
        for value in &x_ticks {
            vline(window, frame.at(*value, y.0).x, colors.grid_faint);
        }
    }
    // Zero lines are the reference a reader looks for, so they are stronger than the grid.
    if y.0 < 0. && y.1 > 0. {
        hline(window, frame.at(x.0, 0.).y, colors.grid);
    }
    if x.0 < 0. && x.1 > 0. {
        vline(window, frame.at(0., y.0).x, colors.grid);
    }
    hline(window, area.bottom(), colors.grid);
    vline(window, area.left(), colors.grid);
    for (value, text) in y_ticks.iter().zip(&y_texts) {
        let at = frame.at(x.0, *value).y;
        text.paint(
            point(area.left() - px(8.) - text.width(), at - text.height / 2.),
            window,
            cx,
        );
    }
    for (value, text) in x_ticks.iter().zip(&x_texts) {
        let at = frame.at(*value, y.0).x;
        let left = (at - text.width() / 2.)
            .max(bounds.left())
            .min(bounds.right() - text.width());
        text.paint(point(left, area.bottom() + px(6.)), window, cx);
    }
    if let Some(title) = x_title {
        title.paint(
            point(
                area.left() + (area.size.width - title.width()) / 2.,
                area.bottom() + px(6.) + tick_height,
            ),
            window,
            cx,
        );
    }
    if let Some(title) = y_title {
        title.paint(
            point(
                (area.left() - title.width() / 2.).max(bounds.left() + px(4.)),
                bounds.top() + px(6.),
            ),
            window,
            cx,
        );
    }
    frame
}

/// A legend in the top-right corner of the data area: a swatch and a name per entry.
fn paint_legend(
    window: &mut Window,
    cx: &mut App,
    theme: &Theme,
    area: Bounds<Pixels>,
    entries: &[(SharedString, Hsla, bool)],
) {
    if entries.is_empty() {
        return;
    }
    let texts: Vec<_> = entries
        .iter()
        .map(|(name, _, _)| {
            Text::new(
                window,
                name.clone(),
                &theme.typography.ui,
                theme.typography.size_xs,
                theme.colors.text_muted,
            )
        })
        .collect();
    let row = theme.typography.size_xs * 1.5;
    let swatch = px(14.);
    let pad = px(8.);
    let widest = texts.iter().map(Text::width).fold(px(0.), |a, b| a.max(b));
    let box_size = size(
        pad * 2. + swatch + px(6.) + widest,
        pad * 2. + row * entries.len() as f32 - px(4.),
    );
    let origin = point(area.right() - box_size.width - px(8.), area.top() + px(8.));
    window.paint_quad(
        fill(
            Bounds::new(origin, box_size),
            theme.colors.elevated.opacity(0.9),
        )
        .corner_radii(theme.shape.radius_sm)
        .border_widths(px(1.))
        .border_color(theme.colors.border),
    );
    for (index, ((_, color, dot), text)) in entries.iter().zip(&texts).enumerate() {
        let top = origin.y + pad + row * index as f32;
        let middle = top + text.height / 2.;
        let mark = if *dot {
            Bounds::new(
                point(origin.x + pad + swatch / 2. - px(3.), middle - px(3.)),
                size(px(6.), px(6.)),
            )
        } else {
            Bounds::new(point(origin.x + pad, middle - px(1.)), size(swatch, px(2.)))
        };
        window.paint_quad(fill(mark, *color).corner_radii(px(3.)));
        text.paint(point(origin.x + pad + swatch + px(6.), top), window, cx);
    }
}

fn paint_marker(window: &mut Window, at: Point<Pixels>, radius: Pixels, color: Hsla) {
    window.paint_quad(
        fill(
            Bounds::new(
                point(at.x - radius, at.y - radius),
                size(radius * 2., radius * 2.),
            ),
            color,
        )
        .corner_radii(radius),
    );
}

/// How a [`Series`] is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesStyle {
    /// A line through the points: `plot`.
    Line,
    /// A dot at each point: `scatter`.
    Markers,
    /// A line with a dot at each point, for measured samples.
    LineMarkers,
    /// A stalk from zero to each point: `stem`, for discrete sequences and coefficients.
    Stems,
    /// Horizontal runs between points: `stairs`, for piecewise-constant signals.
    Steps,
    /// A line shaded down to zero.
    Area,
}

/// One set of `(x, y)` points in a [`LinePlot`].
#[derive(Clone)]
pub struct Series {
    points: Vec<(f32, f32)>,
    style: SeriesStyle,
    color: Option<Hsla>,
    name: Option<SharedString>,
    faint: bool,
}

impl Series {
    /// A line through `points`.
    pub fn line(points: Vec<(f32, f32)>) -> Self {
        Self {
            points,
            style: SeriesStyle::Line,
            color: None,
            name: None,
            faint: false,
        }
    }

    /// A dot at each of `points`.
    pub fn markers(points: Vec<(f32, f32)>) -> Self {
        Self::line(points).style(SeriesStyle::Markers)
    }

    /// A stem from zero to each of `points`.
    pub fn stems(points: Vec<(f32, f32)>) -> Self {
        Self::line(points).style(SeriesStyle::Stems)
    }

    /// Change how it is drawn.
    pub fn style(mut self, style: SeriesStyle) -> Self {
        self.style = style;
        self
    }

    /// Colour; defaults to the series colour for its position in the plot.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// The name shown in the legend.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Draw at reference strength: an envelope, a theoretical curve behind measured points.
    pub fn faint(mut self) -> Self {
        self.faint = true;
        self
    }
}

/// `(x, y)` series on labelled axes: MATLAB's `plot`, `scatter`, `stem` and `stairs`.
#[derive(IntoElement)]
pub struct LinePlot {
    series: Vec<Series>,
    x: Axis,
    y: Axis,
    legend: bool,
    grid: bool,
}

impl LinePlot {
    /// An empty plot; add series with [`LinePlot::series`].
    pub fn new() -> Self {
        Self {
            series: Vec::new(),
            x: Axis::default(),
            y: Axis::default(),
            legend: false,
            grid: true,
        }
    }

    /// Add a series, drawn over the ones before it.
    pub fn series(mut self, series: Series) -> Self {
        self.series.push(series);
        self
    }

    /// Fix the x range; by default it spans the data exactly.
    pub fn x_range(mut self, min: f32, max: f32) -> Self {
        self.x.range = Some((min, max));
        self
    }

    /// Fix the y range; by default it spans the data, rounded out to whole ticks.
    pub fn y_range(mut self, min: f32, max: f32) -> Self {
        self.y.range = Some((min, max));
        self
    }

    /// Title under the x axis, with its unit: "t [s]".
    pub fn x_label(mut self, label: impl Into<SharedString>) -> Self {
        self.x.label = Some(label.into());
        self
    }

    /// Title over the y axis, with its unit.
    pub fn y_label(mut self, label: impl Into<SharedString>) -> Self {
        self.y.label = Some(label.into());
        self
    }

    /// Show a legend of the named series.
    pub fn legend(mut self, legend: bool) -> Self {
        self.legend = legend;
        self
    }

    /// Show the grid at the ticks (on by default).
    pub fn grid(mut self, grid: bool) -> Self {
        self.grid = grid;
        self
    }
}

impl Default for LinePlot {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for LinePlot {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Self {
            series,
            x,
            y,
            legend,
            grid,
        } = self;
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, cx| {
                let points = || series.iter().flat_map(|series| series.points.iter());
                let x_range = x.range.unwrap_or_else(|| extent(points().map(|at| at.0)));
                let y_range = y.range.unwrap_or_else(|| {
                    let stems = series.iter().any(|s| s.style == SeriesStyle::Stems);
                    let (lo, hi) = extent(points().map(|at| at.1));
                    let (lo, hi) = if stems {
                        (lo.min(0.), hi.max(0.))
                    } else {
                        (lo, hi)
                    };
                    round_out((lo, hi))
                });
                let frame = paint_axes(
                    window,
                    cx,
                    &theme,
                    bounds,
                    x_range,
                    y_range,
                    x.label.as_ref(),
                    y.label.as_ref(),
                    grid,
                );
                let colors = &theme.colors;
                let mut entries = Vec::new();
                window.with_content_mask(frame.mask(), |window| {
                    for (index, series) in series.iter().enumerate() {
                        let color = series
                            .color
                            .unwrap_or(colors.series[index % colors.series.len()]);
                        let color = if series.faint {
                            color.opacity(0.5)
                        } else {
                            color
                        };
                        let at: Vec<_> = series
                            .points
                            .iter()
                            .map(|(x, y)| frame.at(*x, *y))
                            .collect();
                        let baseline = frame.at(x_range.0, 0f32.clamp(y_range.0, y_range.1)).y;
                        match series.style {
                            SeriesStyle::Line | SeriesStyle::LineMarkers | SeriesStyle::Area => {
                                if series.style == SeriesStyle::Area
                                    && let Some(path) = crate::viz::area(&at, baseline)
                                {
                                    window.paint_path(path, color.opacity(0.18));
                                }
                                if let Some(path) = polyline(&at, px(1.75)) {
                                    window.paint_path(path, color);
                                }
                                if series.style == SeriesStyle::LineMarkers {
                                    for point in &at {
                                        paint_marker(window, *point, px(3.), color);
                                    }
                                }
                            }
                            SeriesStyle::Markers => {
                                for point in &at {
                                    paint_marker(window, *point, px(2.5), color.opacity(0.75));
                                }
                            }
                            SeriesStyle::Stems => {
                                for top in &at {
                                    if let Some(path) =
                                        polyline(&[point(top.x, baseline), *top], px(1.5))
                                    {
                                        window.paint_path(path, color);
                                    }
                                    paint_marker(window, *top, px(3.5), color);
                                }
                            }
                            SeriesStyle::Steps => {
                                let mut steps = Vec::with_capacity(at.len() * 2);
                                for pair in at.windows(2) {
                                    steps.push(pair[0]);
                                    steps.push(point(pair[1].x, pair[0].y));
                                }
                                steps.extend(at.last());
                                if let Some(path) = polyline(&steps, px(1.75)) {
                                    window.paint_path(path, color);
                                }
                            }
                        }
                        if let Some(name) = &series.name {
                            entries.push((
                                name.clone(),
                                color,
                                series.style == SeriesStyle::Markers,
                            ));
                        }
                    }
                });
                if legend {
                    paint_legend(window, cx, &theme, frame.area, &entries);
                }
            },
        )
        .size_full()
    }
}

/// Samples counted into equal bins, with an optional curve over them: MATLAB's `histogram`
/// with a fitted or theoretical distribution laid on top.
#[derive(IntoElement)]
pub struct Histogram {
    samples: Vec<f32>,
    bins: usize,
    range: Option<(f32, f32)>,
    density: bool,
    color: Option<Hsla>,
    name: Option<SharedString>,
    overlay: Option<(Vec<(f32, f32)>, SharedString)>,
    x: Axis,
    y_label: Option<SharedString>,
}

impl Histogram {
    /// A histogram of `samples` in 30 bins over their range.
    pub fn new(samples: Vec<f32>) -> Self {
        Self {
            samples,
            bins: 30,
            range: None,
            density: false,
            color: None,
            name: None,
            overlay: None,
            x: Axis::default(),
            y_label: None,
        }
    }

    /// Replace the samples, keeping the other settings.
    pub fn samples(mut self, samples: Vec<f32>) -> Self {
        self.samples = samples;
        self
    }

    /// Number of bins.
    pub fn bins(mut self, bins: usize) -> Self {
        self.bins = bins.max(1);
        self
    }

    /// The range the bins cover; samples outside it are not counted.
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.range = Some((min, max));
        self
    }

    /// Scale bars to a probability density, so a distribution's curve can be laid over them.
    pub fn density(mut self) -> Self {
        self.density = true;
        self
    }

    /// Bar colour; defaults to the first series colour.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// The name of the samples in the legend.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// A curve over the bars, in the second series colour: the expected distribution.
    pub fn overlay(mut self, points: Vec<(f32, f32)>, name: impl Into<SharedString>) -> Self {
        self.overlay = Some((points, name.into()));
        self
    }

    /// Title under the x axis.
    pub fn x_label(mut self, label: impl Into<SharedString>) -> Self {
        self.x.label = Some(label.into());
        self
    }

    /// Title over the y axis; defaults to "Count" or "Density".
    pub fn y_label(mut self, label: impl Into<SharedString>) -> Self {
        self.y_label = Some(label.into());
        self
    }

    /// Counts per bin, scaled to density when asked.
    fn heights(&self, (min, max): (f32, f32)) -> Vec<f32> {
        let mut counts = vec![0f32; self.bins];
        let width = (max - min) / self.bins as f32;
        for sample in &self.samples {
            if *sample < min || *sample > max {
                continue;
            }
            let index = (((sample - min) / width) as usize).min(self.bins - 1);
            counts[index] += 1.;
        }
        if self.density {
            let scale = 1. / (self.samples.len().max(1) as f32 * width);
            counts.iter_mut().for_each(|count| *count *= scale);
        }
        counts
    }
}

impl RenderOnce for Histogram {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let range = self
            .range
            .unwrap_or_else(|| extent(self.samples.iter().copied()));
        let heights = self.heights(range);
        let y_label = self
            .y_label
            .clone()
            .unwrap_or_else(|| if self.density { "Density" } else { "Count" }.into());
        let Self {
            color,
            name,
            overlay,
            x,
            ..
        } = self;
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, cx| {
                let colors = &theme.colors;
                let top = heights
                    .iter()
                    .copied()
                    .chain(
                        overlay
                            .iter()
                            .flat_map(|(points, _)| points.iter().map(|at| at.1)),
                    )
                    .fold(0f32, f32::max);
                let y_range = round_out((0., if top > 0. { top * 1.05 } else { 1. }));
                let frame = paint_axes(
                    window,
                    cx,
                    &theme,
                    bounds,
                    range,
                    y_range,
                    x.label.as_ref(),
                    Some(&y_label),
                    true,
                );
                let bar = color.unwrap_or(colors.series[0]);
                let width = (range.1 - range.0) / heights.len() as f32;
                let gap = if frame.area.size.width / heights.len() as f32 > px(5.) {
                    px(1.)
                } else {
                    px(0.)
                };
                let mut entries = Vec::new();
                window.with_content_mask(frame.mask(), |window| {
                    for (index, height) in heights.iter().enumerate() {
                        let left = frame.at(range.0 + width * index as f32, 0.);
                        let right = frame.at(range.0 + width * (index + 1) as f32, *height);
                        let bounds = Bounds::from_corners(
                            point(left.x + gap / 2., right.y),
                            point(right.x - gap / 2., left.y),
                        );
                        window.paint_quad(fill(bounds, bar.opacity(0.45)));
                        window.paint_quad(fill(
                            Bounds::new(bounds.origin, size(bounds.size.width, px(1.5))),
                            bar,
                        ));
                    }
                    if let Some((points, _)) = &overlay {
                        let at: Vec<_> = points.iter().map(|(x, y)| frame.at(*x, *y)).collect();
                        if let Some(path) = polyline(&at, px(2.)) {
                            window.paint_path(path, colors.series[1]);
                        }
                    }
                });
                if let Some(name) = name {
                    entries.push((name, bar, false));
                }
                if let Some((_, name)) = overlay {
                    entries.push((name, colors.series[1], false));
                }
                paint_legend(window, cx, &theme, frame.area, &entries);
            },
        )
        .size_full()
    }
}

/// The colour of `value` (0 to 1) on the ramp a field uses: the heat ramp, or for a signed
/// field the second series colour through the well to the first. The heat ramp starts a step
/// in, because its first stop is the well's own colour and a field's minimum is a value, not
/// silence.
fn field_color(theme: &Theme, value: f32, signed: bool) -> Hsla {
    if signed {
        let colors = &theme.colors;
        let centred = value * 2. - 1.;
        let tint = if centred >= 0. {
            colors.series[0]
        } else {
            colors.series[1]
        };
        mix(colors.sunken, tint, centred.abs().clamp(0., 1.).powf(0.8))
    } else {
        heat_color(theme, 0.12 + 0.88 * value.clamp(0., 1.))
    }
}

/// Where on a cell's edge a crossing at `t` along it lies.
type Place = fn(f32) -> (f32, f32);

/// Segments of the line where a grid of values crosses `level`, by marching squares, in cell
/// coordinates: x along columns, y along rows.
fn contour_segments(
    columns: usize,
    rows: usize,
    values: &[f32],
    level: f32,
) -> Vec<[(f32, f32); 2]> {
    let mut segments = Vec::new();
    let at = |column: usize, row: usize| values[row * columns + column];
    let cross = |a: f32, b: f32| ((level - a) / (b - a)).clamp(0., 1.);
    for row in 0..rows.saturating_sub(1) {
        for column in 0..columns.saturating_sub(1) {
            let corners = [
                at(column, row),
                at(column + 1, row),
                at(column + 1, row + 1),
                at(column, row + 1),
            ];
            // Where the level crosses each edge: bottom, right, top, left.
            let (x, y) = (column as f32, row as f32);
            let edges: [(f32, f32, Place); 4] = [
                (corners[0], corners[1], |t: f32| (t, 0.)),
                (corners[1], corners[2], |t: f32| (1., t)),
                (corners[3], corners[2], |t: f32| (t, 1.)),
                (corners[0], corners[3], |t: f32| (0., t)),
            ];
            let crossings: Vec<_> = edges
                .iter()
                .filter(|(a, b, _)| (*a < level) != (*b < level))
                .map(|(a, b, place)| {
                    let (dx, dy) = place(cross(*a, *b));
                    (x + dx, y + dy)
                })
                .collect();
            match crossings.as_slice() {
                [a, b] => segments.push([*a, *b]),
                // A saddle: pair the crossings by the centre's side.
                [a, b, c, d] => {
                    let centre = corners.iter().sum::<f32>() / 4.;
                    if (centre < level) == (corners[0] < level) {
                        segments.push([*a, *b]);
                        segments.push([*c, *d]);
                    } else {
                        segments.push([*a, *d]);
                        segments.push([*b, *c]);
                    }
                }
                _ => {}
            }
        }
    }
    segments
}

/// A scalar field over a rectangle, coloured by value, with optional contour lines: MATLAB's
/// `imagesc` with `contour` over it. For potentials, temperatures, probability densities.
#[derive(IntoElement)]
pub struct FieldPlot {
    columns: usize,
    rows: usize,
    values: Vec<f32>,
    value_range: Option<(f32, f32)>,
    contours: usize,
    signed: bool,
    x: Axis,
    y: Axis,
}

impl FieldPlot {
    /// `values` in row-major order, the first row at the bottom (`y` minimum), as a field
    /// sampled on a grid with `y` up.
    pub fn new(columns: usize, rows: usize, values: Vec<f32>) -> Self {
        Self {
            columns,
            rows,
            values,
            value_range: None,
            contours: 0,
            signed: false,
            x: Axis {
                range: Some((0., 1.)),
                label: None,
            },
            y: Axis {
                range: Some((0., 1.)),
                label: None,
            },
        }
    }

    /// Replace the values, keeping the other settings.
    pub fn values(mut self, values: Vec<f32>) -> Self {
        self.values = values;
        self
    }

    /// The x extent of the grid, from the first column's centre to the last's.
    pub fn x_range(mut self, min: f32, max: f32) -> Self {
        self.x.range = Some((min, max));
        self
    }

    /// The y extent of the grid.
    pub fn y_range(mut self, min: f32, max: f32) -> Self {
        self.y.range = Some((min, max));
        self
    }

    /// The values at the ends of the colour ramp; by default the data's own range, or for a
    /// signed field symmetric about zero.
    pub fn value_range(mut self, min: f32, max: f32) -> Self {
        self.value_range = Some((min, max));
        self
    }

    /// Draw `count` contour lines at even steps through the value range.
    pub fn contours(mut self, count: usize) -> Self {
        self.contours = count;
        self
    }

    /// Colour negative values in the second series colour and positive in the first, with
    /// zero in the well's own colour: for potentials and other signed fields.
    pub fn signed(mut self) -> Self {
        self.signed = true;
        self
    }

    /// Title under the x axis.
    pub fn x_label(mut self, label: impl Into<SharedString>) -> Self {
        self.x.label = Some(label.into());
        self
    }

    /// Title over the y axis.
    pub fn y_label(mut self, label: impl Into<SharedString>) -> Self {
        self.y.label = Some(label.into());
        self
    }
}

impl RenderOnce for FieldPlot {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Self {
            columns,
            rows,
            values,
            value_range,
            contours,
            signed,
            x,
            y,
        } = self;
        let (low, high) = value_range.unwrap_or_else(|| {
            let (lo, hi) = extent(values.iter().copied());
            if signed {
                let reach = lo.abs().max(hi.abs());
                (-reach, reach)
            } else {
                (lo, hi)
            }
        });
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, cx| {
                if columns < 2 || rows < 2 || values.len() < columns * rows {
                    return;
                }
                let (x_range, y_range) = (x.range.unwrap_or((0., 1.)), y.range.unwrap_or((0., 1.)));
                let frame = paint_axes(
                    window,
                    cx,
                    &theme,
                    bounds,
                    x_range,
                    y_range,
                    x.label.as_ref(),
                    y.label.as_ref(),
                    false,
                );
                let normal = |value: f32| ((value - low) / (high - low)).clamp(0., 1.);
                // Cell centres sit on the grid points, so the edge cells are half cut off.
                let cell_x = (x_range.1 - x_range.0) / (columns - 1) as f32;
                let cell_y = (y_range.1 - y_range.0) / (rows - 1) as f32;
                let place = |column: f32, row: f32| {
                    frame.at(x_range.0 + column * cell_x, y_range.0 + row * cell_y)
                };
                window.with_content_mask(frame.mask(), |window| {
                    for row in 0..rows {
                        for column in 0..columns {
                            let value = values[row * columns + column];
                            let corner = place(column as f32 - 0.5, row as f32 + 0.5);
                            let opposite = place(column as f32 + 0.5, row as f32 - 0.5);
                            window.paint_quad(fill(
                                Bounds::from_corners(corner, opposite + point(px(0.5), px(0.5))),
                                field_color(&theme, normal(value), signed),
                            ));
                        }
                    }
                    for step in 1..=contours {
                        let level = low + (high - low) * step as f32 / (contours + 1) as f32;
                        let color = theme.colors.text.opacity(if signed && level.abs() < 1e-6 {
                            0.5
                        } else {
                            0.28
                        });
                        for [a, b] in contour_segments(columns, rows, &values, level) {
                            if let Some(path) =
                                polyline(&[place(a.0, a.1), place(b.0, b.1)], px(1.))
                            {
                                window.paint_path(path, color);
                            }
                        }
                    }
                });
                window.paint_quad(
                    fill(frame.area, gpui::transparent_black())
                        .border_widths(px(1.))
                        .border_color(theme.colors.grid),
                );
            },
        )
        .size_full()
    }
}

/// A vertical scale from `min` to `max` on the field or surface colour ramp, with ticks: put
/// it beside a [`FieldPlot`] or [`Plot3d`] whose colours it explains.
#[derive(IntoElement)]
pub struct Colorbar {
    min: f32,
    max: f32,
    signed: bool,
    label: Option<SharedString>,
}

impl Colorbar {
    /// A colour bar from `min` at the bottom to `max` at the top.
    pub fn new(min: f32, max: f32) -> Self {
        Self {
            min,
            max,
            signed: false,
            label: None,
        }
    }

    /// The signed ramp, to match [`FieldPlot::signed`].
    pub fn signed(mut self) -> Self {
        self.signed = true;
        self
    }

    /// The quantity and unit, shown above the bar.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl RenderOnce for Colorbar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Self {
            min,
            max,
            signed,
            label,
        } = self;
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, cx| {
                let title = label
                    .as_ref()
                    .map(|label| title_text(window, &theme, label));
                let top = bounds.top()
                    + px(10.)
                    + title.as_ref().map_or(px(0.), |title| title.height + px(2.));
                let bottom = bounds.bottom() - theme.typography.size_xs * 1.3 - px(10.);
                let height = (bottom - top).max(px(1.));
                let strip = Bounds::new(point(bounds.left() + px(2.), top), size(px(10.), height));
                let steps = 48;
                for index in 0..steps {
                    let value = (index as f32 + 0.5) / steps as f32;
                    let y = strip.bottom() - height * ((index + 1) as f32 / steps as f32);
                    window.paint_quad(fill(
                        Bounds::new(
                            point(strip.left(), y),
                            size(strip.size.width, height / steps as f32 + px(0.5)),
                        ),
                        field_color(&theme, value, signed),
                    ));
                }
                window.paint_quad(
                    fill(strip, gpui::transparent_black())
                        .border_widths(px(1.))
                        .border_color(theme.colors.grid),
                );
                let target = ((height / px(36.)) as usize).clamp(2, 8);
                let step = nice_step(min, max, target);
                for value in nice_ticks(min, max, target) {
                    let y = strip.bottom() - height * ((value - min) / (max - min));
                    window.paint_quad(fill(
                        Bounds::new(point(strip.right(), y), size(px(3.), px(1.))),
                        theme.colors.grid,
                    ));
                    let text = tick_text(window, &theme, value, step);
                    text.paint(
                        point(strip.right() + px(6.), y - text.height / 2.),
                        window,
                        cx,
                    );
                }
                if let Some(title) = title {
                    title.paint(point(bounds.left(), bounds.top() + px(6.)), window, cx);
                }
            },
        )
        .size_full()
    }
}

/// How a [`Plot3d`] surface is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shading {
    /// Coloured facets with their edges: MATLAB's `surf`.
    #[default]
    Faceted,
    /// Coloured facets without edges: `shading interp`.
    Smooth,
    /// Coloured edges only, hiding what is behind: `mesh`.
    Mesh,
}

#[derive(Clone)]
struct Surface {
    columns: usize,
    rows: usize,
    values: Vec<f32>,
    x: (f32, f32),
    y: (f32, f32),
}

#[derive(Clone)]
struct Curve {
    points: Vec<[f32; 3]>,
    color: Option<Hsla>,
    markers: bool,
}

/// How much shorter the vertical axis is than the horizontal ones, as in MATLAB's default
/// 3-D box.
const Z_ASPECT: f32 = 0.75;

/// A 3-D view on a box with gridded back walls: a surface over an x–y grid (`surf`, `mesh`),
/// curves (`plot3`) and point clouds (`scatter3`), seen from a chosen azimuth and elevation.
#[derive(IntoElement)]
pub struct Plot3d {
    surface: Option<Surface>,
    curves: Vec<Curve>,
    azimuth: f32,
    elevation: f32,
    shading: Shading,
    z_range: Option<(f32, f32)>,
    labels: [Option<SharedString>; 3],
}

impl Plot3d {
    /// An empty view from MATLAB's default angle, azimuth −37.5° and elevation 30°.
    pub fn new() -> Self {
        Self {
            surface: None,
            curves: Vec::new(),
            azimuth: -37.5,
            elevation: 30.,
            shading: Shading::Faceted,
            z_range: None,
            labels: [None, None, None],
        }
    }

    /// A surface of height `values`, row-major with the first row at `y` minimum, over the
    /// grid spanning `x` and `y`. Coloured by height on the heat ramp.
    pub fn surface(
        mut self,
        columns: usize,
        rows: usize,
        values: Vec<f32>,
        x: (f32, f32),
        y: (f32, f32),
    ) -> Self {
        self.surface = Some(Surface {
            columns,
            rows,
            values,
            x,
            y,
        });
        self
    }

    /// A curve through `points`. Without a colour it runs along the heat ramp from start to
    /// end, so the direction of travel shows.
    pub fn curve(mut self, points: Vec<[f32; 3]>, color: Option<Hsla>) -> Self {
        self.curves.push(Curve {
            points,
            color,
            markers: false,
        });
        self
    }

    /// A dot at each of `points`, nearest drawn last.
    pub fn scatter(mut self, points: Vec<[f32; 3]>, color: Option<Hsla>) -> Self {
        self.curves.push(Curve {
            points,
            color,
            markers: true,
        });
        self
    }

    /// Viewing angle in degrees: azimuth about the vertical axis, elevation above the x–y plane.
    pub fn view(mut self, azimuth: f32, elevation: f32) -> Self {
        self.azimuth = azimuth;
        self.elevation = elevation.clamp(-90., 90.);
        self
    }

    /// Change only the azimuth, for a turntable animation.
    pub fn azimuth(mut self, azimuth: f32) -> Self {
        self.azimuth = azimuth;
        self
    }

    /// How the surface is drawn.
    pub fn shading(mut self, shading: Shading) -> Self {
        self.shading = shading;
        self
    }

    /// Fix the vertical range; by default it spans the data, rounded out to whole ticks.
    pub fn z_range(mut self, min: f32, max: f32) -> Self {
        self.z_range = Some((min, max));
        self
    }

    /// Axis titles, with units.
    pub fn labels(
        mut self,
        x: impl Into<SharedString>,
        y: impl Into<SharedString>,
        z: impl Into<SharedString>,
    ) -> Self {
        self.labels = [Some(x.into()), Some(y.into()), Some(z.into())];
        self
    }
}

impl Default for Plot3d {
    fn default() -> Self {
        Self::new()
    }
}

/// An orthographic camera: data ranges to the unit box, the box to the screen.
struct Camera {
    ranges: [(f32, f32); 3],
    cos_a: f32,
    sin_a: f32,
    cos_e: f32,
    sin_e: f32,
    scale: f32,
    center: Point<Pixels>,
    offset: (f32, f32),
}

impl Camera {
    fn new(ranges: [(f32, f32); 3], azimuth: f32, elevation: f32) -> Self {
        let (a, e) = (azimuth * PI / 180., elevation * PI / 180.);
        Self {
            ranges,
            cos_a: a.cos(),
            sin_a: a.sin(),
            cos_e: e.cos(),
            sin_e: e.sin(),
            scale: 1.,
            center: point(px(0.), px(0.)),
            offset: (0., 0.),
        }
    }

    /// Data to the box: x and y from −1 to 1, z from −`Z_ASPECT` to `Z_ASPECT`.
    fn normalize(&self, at: [f32; 3]) -> [f32; 3] {
        let unit = |value: f32, (min, max): (f32, f32)| (value - min) / (max - min) * 2. - 1.;
        [
            unit(at[0], self.ranges[0]),
            unit(at[1], self.ranges[1]),
            unit(at[2], self.ranges[2]) * Z_ASPECT,
        ]
    }

    /// The box to view space: right, up, and depth (larger is farther).
    fn view(&self, [x, y, z]: [f32; 3]) -> (f32, f32, f32) {
        let right = x * self.cos_a + y * self.sin_a;
        let away = -x * self.sin_a + y * self.cos_a;
        (
            right,
            z * self.cos_e + away * self.sin_e,
            away * self.cos_e - z * self.sin_e,
        )
    }

    /// Fit the box's eight corners into `bounds`.
    fn fit(&mut self, bounds: Bounds<Pixels>, margin: Pixels) {
        let corners = box_corners().map(|corner| self.view(corner));
        let (mut lo, mut hi) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
        for (u, v, _) in corners {
            lo = (lo.0.min(u), lo.1.min(v));
            hi = (hi.0.max(u), hi.1.max(v));
        }
        let width = f32::from(bounds.size.width - margin * 2.).max(1.);
        let height = f32::from(bounds.size.height - margin * 2.).max(1.);
        self.scale = (width / (hi.0 - lo.0)).min(height / (hi.1 - lo.1));
        self.center = bounds.center();
        self.offset = ((hi.0 + lo.0) / 2., (hi.1 + lo.1) / 2.);
    }

    /// A box point on screen, with its depth.
    fn screen(&self, at: [f32; 3]) -> (Point<Pixels>, f32) {
        let (u, v, depth) = self.view(at);
        (
            point(
                self.center.x + px((u - self.offset.0) * self.scale),
                self.center.y - px((v - self.offset.1) * self.scale),
            ),
            depth,
        )
    }

    fn depth(&self, at: [f32; 3]) -> f32 {
        self.view(at).2
    }
}

fn box_corners() -> [[f32; 3]; 8] {
    let mut corners = [[0.; 3]; 8];
    for (index, corner) in corners.iter_mut().enumerate() {
        *corner = [
            if index & 1 == 0 { -1. } else { 1. },
            if index & 2 == 0 { -1. } else { 1. },
            if index & 4 == 0 { -Z_ASPECT } else { Z_ASPECT },
        ];
    }
    corners
}

fn segment(window: &mut Window, a: Point<Pixels>, b: Point<Pixels>, width: Pixels, color: Hsla) {
    if let Some(path) = polyline(&[a, b], width) {
        window.paint_path(path, color);
    }
}

impl RenderOnce for Plot3d {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Self {
            surface,
            curves,
            azimuth,
            elevation,
            shading,
            z_range,
            labels,
        } = self;
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, cx| {
                let colors = &theme.colors;
                let curve_points = || curves.iter().flat_map(|curve| curve.points.iter());
                let mut ranges = [
                    extent(curve_points().map(|at| at[0])),
                    extent(curve_points().map(|at| at[1])),
                    extent(curve_points().map(|at| at[2])),
                ];
                if let Some(surface) = &surface {
                    let heights = extent(surface.values.iter().copied());
                    if curves.is_empty() {
                        ranges = [surface.x, surface.y, heights];
                    } else {
                        ranges[0] = (ranges[0].0.min(surface.x.0), ranges[0].1.max(surface.x.1));
                        ranges[1] = (ranges[1].0.min(surface.y.0), ranges[1].1.max(surface.y.1));
                        ranges[2] = (ranges[2].0.min(heights.0), ranges[2].1.max(heights.1));
                    }
                }
                let heights = ranges[2];
                ranges[2] = z_range.unwrap_or_else(|| round_out(ranges[2]));
                if surface.is_none() {
                    ranges[0] = round_out(ranges[0]);
                    ranges[1] = round_out(ranges[1]);
                }
                let mut camera = Camera::new(ranges, azimuth, elevation);
                camera.fit(bounds, px(44.));

                // The back walls: of each pair of opposite faces, the one farther away.
                // Seen square-on, a pair of walls is level in depth: then the upper wall, or
                // failing that the right one, counts as the back, so labels land below and
                // left. The floor stays the lower face.
                let far = |axis: usize| {
                    let mut probe = [0.; 3];
                    probe[axis] = 1.;
                    let (right, up, depth) = camera.view(probe);
                    let lean = if depth.abs() > 1e-3 {
                        depth
                    } else if axis == 2 {
                        -up
                    } else if up.abs() > 1e-3 {
                        up
                    } else {
                        right
                    };
                    if lean > 0. { 1. } else { -1. }
                };
                // An axis seen end-on has no room for labels.
                let visible = |from: [f32; 3], to: [f32; 3]| {
                    let (a, b) = (camera.screen(from).0, camera.screen(to).0);
                    let (dx, dy) = (f32::from(b.x - a.x), f32::from(b.y - a.y));
                    (dx * dx + dy * dy).sqrt() > 40.
                };
                let (wall_x, wall_y, floor) = (far(0), far(1), far(2) * Z_ASPECT);
                let ticks: [Vec<f32>; 3] = [0, 1, 2].map(|axis| {
                    nice_ticks(
                        ranges[axis].0,
                        ranges[axis].1,
                        if axis == 2 { 4 } else { 5 },
                    )
                });
                let steps: [f32; 3] = [0, 1, 2].map(|axis| {
                    nice_step(
                        ranges[axis].0,
                        ranges[axis].1,
                        if axis == 2 { 4 } else { 5 },
                    )
                });
                let unit = |axis: usize, value: f32| {
                    let mut at = [0.; 3];
                    at[axis] = value;
                    camera.normalize(at)[axis]
                };
                let line3 = |window: &mut Window, a: [f32; 3], b: [f32; 3], color: Hsla| {
                    segment(
                        window,
                        camera.screen(a).0,
                        camera.screen(b).0,
                        px(1.),
                        color,
                    );
                };
                let z = Z_ASPECT;
                for value in &ticks[0] {
                    let x = unit(0, *value);
                    line3(window, [x, -1., floor], [x, 1., floor], colors.grid_faint);
                    line3(window, [x, wall_y, -z], [x, wall_y, z], colors.grid_faint);
                }
                for value in &ticks[1] {
                    let y = unit(1, *value);
                    line3(window, [-1., y, floor], [1., y, floor], colors.grid_faint);
                    line3(window, [wall_x, y, -z], [wall_x, y, z], colors.grid_faint);
                }
                for value in &ticks[2] {
                    let h = unit(2, *value);
                    line3(window, [-1., wall_y, h], [1., wall_y, h], colors.grid_faint);
                    line3(window, [wall_x, -1., h], [wall_x, 1., h], colors.grid_faint);
                }
                // Edges of the three back faces.
                for (a, b) in [
                    ([-1., wall_y, floor], [1., wall_y, floor]),
                    ([wall_x, -1., floor], [wall_x, 1., floor]),
                    ([wall_x, wall_y, -z], [wall_x, wall_y, z]),
                    ([-1., -wall_y, floor], [1., -wall_y, floor]),
                    ([-wall_x, -1., floor], [-wall_x, 1., floor]),
                    ([-1., wall_y, -floor], [1., wall_y, -floor]),
                    ([wall_x, -1., -floor], [wall_x, 1., -floor]),
                    ([-wall_x, wall_y, -z], [-wall_x, wall_y, z]),
                    ([wall_x, -wall_y, -z], [wall_x, -wall_y, z]),
                ] {
                    line3(window, a, b, colors.grid);
                }

                if let Some(surface) = &surface
                    && surface.columns >= 2
                    && surface.rows >= 2
                    && surface.values.len() >= surface.columns * surface.rows
                {
                    let (columns, rows) = (surface.columns, surface.rows);
                    let grid: Vec<[f32; 3]> = (0..rows)
                        .flat_map(|row| (0..columns).map(move |column| (column, row)))
                        .map(|(column, row)| {
                            let x = surface.x.0
                                + (surface.x.1 - surface.x.0) * column as f32
                                    / (columns - 1) as f32;
                            let y = surface.y.0
                                + (surface.y.1 - surface.y.0) * row as f32 / (rows - 1) as f32;
                            camera.normalize([x, y, surface.values[row * columns + column]])
                        })
                        .collect();
                    let mut cells: Vec<(f32, usize)> = (0..rows - 1)
                        .flat_map(|row| (0..columns - 1).map(move |column| row * columns + column))
                        .map(|index| {
                            let corners = [index, index + 1, index + columns + 1, index + columns];
                            let depth = corners
                                .iter()
                                .map(|corner| camera.depth(grid[*corner]))
                                .sum::<f32>();
                            (depth, index)
                        })
                        .collect();
                    // Painter's order: farthest first.
                    cells.sort_by(|a, b| b.0.total_cmp(&a.0));
                    let light = {
                        let (x, y, z) = (-0.4f32, -0.6f32, 0.7f32);
                        let length = (x * x + y * y + z * z).sqrt();
                        [x / length, y / length, z / length]
                    };
                    let edge = colors.sunken.opacity(0.6);
                    for (_, index) in cells {
                        let corners = [index, index + 1, index + columns + 1, index + columns];
                        let at = corners.map(|corner| grid[corner]);
                        let height = corners
                            .iter()
                            .map(|corner| surface.values[*corner])
                            .sum::<f32>()
                            / 4.;
                        let level = (height - heights.0) / (heights.1 - heights.0);
                        let a = [
                            at[1][0] - at[0][0],
                            at[1][1] - at[0][1],
                            at[1][2] - at[0][2],
                        ];
                        let b = [
                            at[3][0] - at[0][0],
                            at[3][1] - at[0][1],
                            at[3][2] - at[0][2],
                        ];
                        let normal = [
                            a[1] * b[2] - a[2] * b[1],
                            a[2] * b[0] - a[0] * b[2],
                            a[0] * b[1] - a[1] * b[0],
                        ];
                        let length = (normal.iter().map(|n| n * n).sum::<f32>()).sqrt().max(1e-6);
                        let lambert =
                            (normal[0] * light[0] + normal[1] * light[1] + normal[2] * light[2])
                                .abs()
                                / length;
                        let color = mix(
                            field_color(&theme, level, false),
                            gpui::black(),
                            (1. - lambert) * 0.35,
                        );
                        let screen = at.map(|corner| camera.screen(corner).0);
                        let mut builder = PathBuilder::fill();
                        builder.move_to(screen[0]);
                        for corner in &screen[1..] {
                            builder.line_to(*corner);
                        }
                        builder.close();
                        let face = match shading {
                            Shading::Mesh => colors.sunken,
                            _ => color,
                        };
                        if let Ok(path) = builder.build() {
                            window.paint_path(path, face);
                        }
                        let outline = match shading {
                            Shading::Faceted => Some((edge, px(0.75))),
                            Shading::Mesh => Some((field_color(&theme, level, false), px(1.))),
                            // A hairline in the face colour hides seams between facets.
                            Shading::Smooth => Some((color, px(0.6))),
                        };
                        if let Some((color, width)) = outline {
                            let mut ring = screen.to_vec();
                            ring.push(screen[0]);
                            if let Some(path) = polyline(&ring, width) {
                                window.paint_path(path, color);
                            }
                        }
                    }
                }

                for curve in &curves {
                    let points: Vec<_> = curve
                        .points
                        .iter()
                        .map(|at| camera.screen(camera.normalize(*at)))
                        .collect();
                    let count = points.len().max(2);
                    if curve.markers {
                        let mut order: Vec<usize> = (0..points.len()).collect();
                        order.sort_by(|a, b| points[*b].1.total_cmp(&points[*a].1));
                        for index in order {
                            let color = curve.color.unwrap_or_else(|| {
                                heat_color(&theme, 0.25 + 0.75 * index as f32 / (count - 1) as f32)
                            });
                            paint_marker(window, points[index].0, px(2.5), color.opacity(0.85));
                        }
                    } else if let Some(color) = curve.color {
                        let at: Vec<_> = points.iter().map(|(at, _)| *at).collect();
                        if let Some(path) = polyline(&at, px(1.5)) {
                            window.paint_path(path, color);
                        }
                    } else {
                        // Runs of one colour each, so a long curve is a few dozen paths.
                        let runs = 48;
                        let length = points.len();
                        for run in 0..runs {
                            let start = length * run / runs;
                            let end = (length * (run + 1) / runs + 1).min(length);
                            if end <= start + 1 {
                                continue;
                            }
                            let at: Vec<_> = points[start..end].iter().map(|(at, _)| *at).collect();
                            let color =
                                heat_color(&theme, 0.2 + 0.8 * run as f32 / (runs - 1) as f32);
                            if let Some(path) = polyline(&at, px(1.25)) {
                                window.paint_path(path, color);
                            }
                        }
                    }
                }

                // Tick labels on the front edges of the floor, each pushed away from the box
                // along its own floor direction so the two sets part at the shared corner.
                let centre = camera.screen([0., 0., 0.]).0;
                let away = |direction: [f32; 3], distance: f32| {
                    let at = camera.screen(direction).0;
                    let (dx, dy) = (f32::from(at.x - centre.x), f32::from(at.y - centre.y));
                    let length = (dx * dx + dy * dy).sqrt();
                    // Seen edge-on, the direction vanishes: push down instead.
                    let (dx, dy) = if length < 0.05 * camera.scale {
                        (0., 1.)
                    } else {
                        (dx / length, dy / length)
                    };
                    point(px(dx * distance), px(dy * distance))
                };
                let edges = [
                    (
                        [-1., -wall_y, floor],
                        [1., -wall_y, floor],
                        [0., -wall_y, 0.],
                    ),
                    (
                        [-wall_x, -1., floor],
                        [-wall_x, 1., floor],
                        [-wall_x, 0., 0.],
                    ),
                ];
                for (axis, (from, to, direction)) in edges.into_iter().enumerate() {
                    if !visible(from, to) {
                        continue;
                    }
                    let middle = camera
                        .screen([(from[0] + to[0]) / 2., (from[1] + to[1]) / 2., from[2]])
                        .0;
                    let push = away(direction, 16.);
                    for value in &ticks[axis] {
                        let mut at = from;
                        at[axis] = unit(axis, *value);
                        let text = tick_text(window, &theme, *value, steps[axis]);
                        text.paint_centered(camera.screen(at).0 + push, window, cx);
                    }
                    if let Some(label) = &labels[axis] {
                        let text = title_text(window, &theme, label);
                        text.paint_centered(middle + away(direction, 36.), window, cx);
                    }
                }
                let upright = [(-1., -1.), (1., -1.), (-1., 1.), (1., 1.)]
                    .into_iter()
                    .min_by(|a, b| {
                        let u = |(x, y): (f32, f32)| f32::from(camera.screen([x, y, 0.]).0.x);
                        u(*a).total_cmp(&u(*b))
                    })
                    .unwrap_or((-1., -1.));
                if !visible([upright.0, upright.1, -z], [upright.0, upright.1, z]) {
                    return;
                }
                for value in &ticks[2] {
                    // The floor's own tick would sit on the floor labels' corner.
                    if (unit(2, *value) - floor).abs() < 0.05 {
                        continue;
                    }
                    let at = camera.screen([upright.0, upright.1, unit(2, *value)]).0;
                    let text = tick_text(window, &theme, *value, steps[2]);
                    text.paint(
                        point(at.x - px(8.) - text.width(), at.y - text.height / 2.),
                        window,
                        cx,
                    );
                }
                if let Some(label) = &labels[2] {
                    let top = camera.screen([upright.0, upright.1, z]).0;
                    let text = title_text(window, &theme, label);
                    text.paint_centered(point(top.x, top.y - px(16.)), window, cx);
                }
            },
        )
        .size_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_fall_on_round_numbers() {
        assert_eq!(nice_ticks(0., 10., 5), vec![0., 2., 4., 6., 8., 10.]);
        assert_eq!(nice_ticks(-1., 1., 4), vec![-1., -0.5, 0., 0.5, 1.]);
        assert_eq!(nice_ticks(0.3, 2.9, 5), vec![0.5, 1., 1.5, 2., 2.5]);
    }

    #[test]
    fn tick_labels_carry_the_step_precision() {
        assert_eq!(format_tick(2., 1.), "2");
        assert_eq!(format_tick(0.5, 0.5), "0.5");
        assert_eq!(format_tick(-0.25, 0.25), "−0.25");
        assert_eq!(format_tick(250_000., 50_000.), "2.5e5");
    }

    #[test]
    fn a_level_crossing_one_corner_gives_one_segment() {
        // Only the top-right corner is above the level.
        let segments = contour_segments(2, 2, &[0., 0., 0., 1.], 0.5);
        assert_eq!(segments.len(), 1);
    }

    #[test]
    fn histogram_density_integrates_to_one() {
        let samples: Vec<f32> = (0..1000).map(|index| index as f32 / 1000.).collect();
        let histogram = Histogram::new(samples).bins(10).density();
        let heights = histogram.heights((0., 1.));
        let total: f32 = heights.iter().map(|height| height * 0.1).sum();
        assert!((total - 1.).abs() < 1e-4, "{total}");
    }
}
