//! Plots for sound: waveforms, spectra, heat maps, vectorscopes and vibrating strings.
//!
//! Each plot paints into whatever box its parent gives it — wrap it in a sized `div` — and takes
//! its data as plain numbers, so it can draw a live analyser, a captured physical-model frame or
//! a mock-up alike. Colours come from the theme's plot tokens: [`crate::Colors::series`] for
//! curves, [`crate::Colors::heat`] for intensity, [`crate::Colors::grid`] for the grid.
//!
//! To animate a plot, rebuild it with new data each frame, for example from a gpui
//! `with_animation` animator: `|scope, t| scope.samples(signal_at(t))`.

use gpui::{
    App, Bounds, Hsla, IntoElement, PathBuilder, Pixels, Point, RenderOnce, Styled, Window, canvas,
    fill, point, px, size,
};

use crate::theme::{ActiveTheme, Theme, mix};

/// The colour of `value` (0 to 1) on the theme's heat ramp.
pub fn heat_color(theme: &Theme, value: f32) -> Hsla {
    let ramp = theme.colors.heat;
    let scaled = value.clamp(0., 1.) * (ramp.len() - 1) as f32;
    let index = (scaled.floor() as usize).min(ramp.len() - 2);
    mix(ramp[index], ramp[index + 1], scaled - index as f32)
}

/// Grid lines: `rows` horizontal divisions and `columns` vertical ones, the middle ones stronger.
fn paint_grid(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    rows: usize,
    columns: usize,
    grid: Hsla,
    faint: Hsla,
) {
    for row in 1..rows {
        let y = bounds.top() + bounds.size.height * (row as f32 / rows as f32);
        let color = if row * 2 == rows { grid } else { faint };
        window.paint_quad(fill(
            Bounds::new(point(bounds.left(), y), size(bounds.size.width, px(1.))),
            color,
        ));
    }
    for column in 1..columns {
        let x = bounds.left() + bounds.size.width * (column as f32 / columns as f32);
        window.paint_quad(fill(
            Bounds::new(point(x, bounds.top()), size(px(1.), bounds.size.height)),
            faint,
        ));
    }
}

fn polyline(points: &[Point<Pixels>], width: Pixels) -> Option<gpui::Path<Pixels>> {
    let (first, rest) = points.split_first()?;
    let mut builder = PathBuilder::stroke(width);
    builder.move_to(*first);
    for at in rest {
        builder.line_to(*at);
    }
    builder.build().ok()
}

fn area(points: &[Point<Pixels>], baseline: Pixels) -> Option<gpui::Path<Pixels>> {
    let (first, last) = (points.first()?, points.last()?);
    let mut builder = PathBuilder::fill();
    builder.move_to(point(first.x, baseline));
    for at in points {
        builder.line_to(*at);
    }
    builder.line_to(point(last.x, baseline));
    builder.close();
    builder.build().ok()
}

/// Samples from -1 to 1 drawn across the box, centred on its middle line.
#[derive(IntoElement)]
pub struct Waveform {
    samples: Vec<f32>,
    color: Option<Hsla>,
    filled: bool,
    grid: bool,
}

impl Waveform {
    /// A waveform of `samples`.
    pub fn new(samples: Vec<f32>) -> Self {
        Self {
            samples,
            color: None,
            filled: false,
            grid: true,
        }
    }

    /// Replace the samples, keeping the other settings.
    pub fn samples(mut self, samples: Vec<f32>) -> Self {
        self.samples = samples;
        self
    }

    /// Curve colour; defaults to the first series colour.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Shade between the curve and the centre line, as an audio region does.
    pub fn filled(mut self) -> Self {
        self.filled = true;
        self
    }

    /// Hide the grid.
    pub fn no_grid(mut self) -> Self {
        self.grid = false;
        self
    }
}

impl RenderOnce for Waveform {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = self.color.unwrap_or(theme.colors.series[0]);
        let (grid, faint) = (theme.colors.grid, theme.colors.grid_faint);
        let Self {
            samples,
            filled,
            grid: show_grid,
            ..
        } = self;
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                if show_grid {
                    paint_grid(window, bounds, 4, 8, grid, faint);
                }
                if samples.len() < 2 {
                    return;
                }
                let middle = bounds.top() + bounds.size.height / 2.;
                let half = bounds.size.height / 2. * 0.92;
                let step = bounds.size.width / (samples.len() - 1) as f32;
                let points: Vec<_> = samples
                    .iter()
                    .enumerate()
                    .map(|(index, sample)| {
                        point(
                            bounds.left() + step * index as f32,
                            middle - half * sample.clamp(-1., 1.),
                        )
                    })
                    .collect();
                if filled && let Some(path) = area(&points, middle) {
                    window.paint_path(path, color.opacity(0.22));
                }
                if let Some(path) = polyline(&points, px(1.5)) {
                    window.paint_path(path, color);
                }
            },
        )
        .size_full()
    }
}

/// How a [`Spectrum`] is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpectrumStyle {
    /// Separate bars, one per band: a graphic analyser.
    Bars,
    /// A shaded curve through the bands: an EQ's analyser.
    Curve,
}

/// Band levels from 0 to 1, low to high, with optional peak holds.
#[derive(IntoElement)]
pub struct Spectrum {
    levels: Vec<f32>,
    peaks: Option<Vec<f32>>,
    style: SpectrumStyle,
    color: Option<Hsla>,
    octaves: usize,
}

impl Spectrum {
    /// A spectrum of `levels`.
    pub fn new(levels: Vec<f32>) -> Self {
        Self {
            levels,
            peaks: None,
            style: SpectrumStyle::Bars,
            color: None,
            octaves: 10,
        }
    }

    /// Replace the levels, keeping the other settings.
    pub fn levels(mut self, levels: Vec<f32>) -> Self {
        self.levels = levels;
        self
    }

    /// Peak-hold marks, one per band.
    pub fn peaks(mut self, peaks: Vec<f32>) -> Self {
        self.peaks = Some(peaks);
        self
    }

    /// Bars or curve.
    pub fn style(mut self, style: SpectrumStyle) -> Self {
        self.style = style;
        self
    }

    /// Colour; bars default to the heat ramp by level, curves to the first series colour.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// How many vertical divisions to draw, usually the octaves shown.
    pub fn octaves(mut self, octaves: usize) -> Self {
        self.octaves = octaves;
        self
    }
}

impl RenderOnce for Spectrum {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Self {
            levels,
            peaks,
            style,
            color,
            octaves,
        } = self;
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let colors = &theme.colors;
                paint_grid(window, bounds, 6, octaves, colors.grid, colors.grid_faint);
                let count = levels.len();
                if count == 0 {
                    return;
                }
                let band = bounds.size.width / count as f32;
                let height = |level: f32| bounds.size.height * level.clamp(0., 1.);
                match style {
                    SpectrumStyle::Bars => {
                        let gap = if band > px(4.) { px(1.5) } else { px(0.) };
                        for (index, level) in levels.iter().enumerate() {
                            let x = bounds.left() + band * index as f32;
                            let top = bounds.bottom() - height(*level);
                            let fill_color =
                                color.unwrap_or_else(|| heat_color(&theme, 0.35 + level * 0.65));
                            window.paint_quad(
                                fill(
                                    Bounds::new(
                                        point(x + gap / 2., top),
                                        size(band - gap, height(*level)),
                                    ),
                                    fill_color,
                                )
                                .corner_radii(px(1.)),
                            );
                        }
                    }
                    SpectrumStyle::Curve => {
                        let line = color.unwrap_or(colors.series[0]);
                        let points: Vec<_> = levels
                            .iter()
                            .enumerate()
                            .map(|(index, level)| {
                                point(
                                    bounds.left() + band * (index as f32 + 0.5),
                                    bounds.bottom() - height(*level),
                                )
                            })
                            .collect();
                        if let Some(path) = area(&points, bounds.bottom()) {
                            window.paint_path(path, line.opacity(0.18));
                        }
                        if let Some(path) = polyline(&points, px(1.5)) {
                            window.paint_path(path, line);
                        }
                    }
                }
                if let Some(peaks) = &peaks {
                    for (index, peak) in peaks.iter().enumerate().take(count) {
                        let x = bounds.left() + band * index as f32;
                        let y = bounds.bottom() - height(*peak);
                        window.paint_quad(fill(
                            Bounds::new(point(x + px(1.), y), size(band - px(2.), px(1.5))),
                            colors.text.opacity(0.7),
                        ));
                    }
                }
            },
        )
        .size_full()
    }
}

/// A grid of intensities from 0 to 1 on the heat ramp: a spectrogram, a membrane's displacement.
#[derive(IntoElement)]
pub struct Heatmap {
    columns: usize,
    rows: usize,
    values: Vec<f32>,
    circular: bool,
    signed: bool,
}

impl Heatmap {
    /// `values` in row-major order, top row first.
    pub fn new(columns: usize, rows: usize, values: Vec<f32>) -> Self {
        Self {
            columns,
            rows,
            values,
            circular: false,
            signed: false,
        }
    }

    /// Replace the values, keeping the other settings.
    pub fn values(mut self, values: Vec<f32>) -> Self {
        self.values = values;
        self
    }

    /// Mask to the inscribed circle and draw its rim: a drum head.
    pub fn circular(mut self) -> Self {
        self.circular = true;
        self
    }

    /// Values run from -1 to 1: negative in the second series colour, positive in the first,
    /// rest in the background. For displacement, where the sign is the point.
    pub fn signed(mut self) -> Self {
        self.signed = true;
        self
    }
}

impl RenderOnce for Heatmap {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Self {
            columns,
            rows,
            values,
            circular,
            signed,
        } = self;
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                if columns == 0 || rows == 0 {
                    return;
                }
                let side = bounds.size.width.min(bounds.size.height);
                let area = if circular {
                    Bounds::new(
                        point(
                            bounds.left() + (bounds.size.width - side) / 2.,
                            bounds.top() + (bounds.size.height - side) / 2.,
                        ),
                        size(side, side),
                    )
                } else {
                    bounds
                };
                let cell = size(
                    area.size.width / columns as f32,
                    area.size.height / rows as f32,
                );
                let colors = &theme.colors;
                for row in 0..rows {
                    for column in 0..columns {
                        if circular {
                            let x = (column as f32 + 0.5) / columns as f32 * 2. - 1.;
                            let y = (row as f32 + 0.5) / rows as f32 * 2. - 1.;
                            if x * x + y * y > 1. {
                                continue;
                            }
                        }
                        let value = values.get(row * columns + column).copied().unwrap_or(0.);
                        let color = if signed {
                            let tint = if value >= 0. {
                                colors.series[0]
                            } else {
                                colors.series[1]
                            };
                            mix(colors.sunken, tint, value.abs().clamp(0., 1.))
                        } else {
                            heat_color(&theme, value)
                        };
                        let origin = point(
                            area.left() + cell.width * column as f32,
                            area.top() + cell.height * row as f32,
                        );
                        // Half a pixel of overlap hides seams between cells.
                        window.paint_quad(fill(
                            Bounds::new(origin, size(cell.width + px(0.5), cell.height + px(0.5))),
                            color,
                        ));
                    }
                }
                if circular {
                    window.paint_quad(
                        fill(area, gpui::transparent_black())
                            .corner_radii(side / 2.)
                            .border_widths(px(1.5))
                            .border_color(colors.grid),
                    );
                }
            },
        )
        .size_full()
    }
}

/// Stereo samples as points: left on one diagonal, right on the other, mono on the vertical.
#[derive(IntoElement)]
pub struct Vectorscope {
    points: Vec<(f32, f32)>,
}

impl Vectorscope {
    /// A vectorscope of `(left, right)` sample pairs from -1 to 1.
    pub fn new(points: Vec<(f32, f32)>) -> Self {
        Self { points }
    }

    /// Replace the points.
    pub fn points(mut self, points: Vec<(f32, f32)>) -> Self {
        self.points = points;
        self
    }
}

impl RenderOnce for Vectorscope {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (grid, faint, dot) = (
            theme.colors.grid,
            theme.colors.grid_faint,
            theme.colors.series[0],
        );
        let points = self.points;
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let side = bounds.size.width.min(bounds.size.height);
                let center = bounds.center();
                let radius = side / 2. * 0.94;
                let ring = Bounds::new(
                    point(center.x - radius, center.y - radius),
                    size(radius * 2., radius * 2.),
                );
                window.paint_quad(
                    fill(ring, gpui::transparent_black())
                        .corner_radii(radius)
                        .border_widths(px(1.))
                        .border_color(grid),
                );
                let diagonal = std::f32::consts::FRAC_1_SQRT_2;
                for (dx, dy) in [
                    (0., 1.),
                    (1., 0.),
                    (diagonal, diagonal),
                    (diagonal, -diagonal),
                ] {
                    if let Some(path) = polyline(
                        &[
                            point(center.x - radius * dx, center.y - radius * dy),
                            point(center.x + radius * dx, center.y + radius * dy),
                        ],
                        px(1.),
                    ) {
                        window.paint_path(path, if dx == 0. { grid } else { faint });
                    }
                }
                // Mid (L+R) is vertical and side (L−R) horizontal, as on a goniometer.
                for (left, right) in &points {
                    let mid = (left + right) * 0.5;
                    let side = (left - right) * 0.5;
                    let at = point(center.x + radius * side, center.y - radius * mid);
                    window.paint_quad(
                        fill(
                            Bounds::new(point(at.x - px(1.), at.y - px(1.)), size(px(2.), px(2.))),
                            dot.opacity(0.55),
                        )
                        .corner_radii(px(1.)),
                    );
                }
            },
        )
        .size_full()
    }
}

/// One vibrating string between two anchors: its displacement now, fading traces of the last
/// few frames, and where it is being bowed, plucked or struck.
#[derive(IntoElement)]
pub struct StringLane {
    points: Vec<f32>,
    trails: Vec<Vec<f32>>,
    contact: Option<f32>,
    color: Option<Hsla>,
    active: bool,
}

impl StringLane {
    /// A string whose displacement along its length is `points`, each from -1 to 1.
    pub fn new(points: Vec<f32>) -> Self {
        Self {
            points,
            trails: Vec::new(),
            contact: None,
            color: None,
            active: true,
        }
    }

    /// Replace the displacement.
    pub fn points(mut self, points: Vec<f32>) -> Self {
        self.points = points;
        self
    }

    /// Earlier frames, oldest first, drawn fading behind the current one.
    pub fn trails(mut self, trails: Vec<Vec<f32>>) -> Self {
        self.trails = trails;
        self
    }

    /// Excitation position from 0 (nut) to 1 (bridge).
    pub fn contact(mut self, contact: f32) -> Self {
        self.contact = Some(contact.clamp(0., 1.));
        self
    }

    /// The voice colour.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// A released or silent string, drawn at rest strength.
    pub fn inactive(mut self) -> Self {
        self.active = false;
        self
    }
}

impl RenderOnce for StringLane {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = self.color.unwrap_or(theme.colors.series[0]);
        let (rest, anchor, contact_ring) = (
            theme.colors.grid,
            theme.colors.text_faint,
            theme.colors.text,
        );
        let Self {
            points,
            trails,
            contact,
            active,
            ..
        } = self;
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let inset = px(10.);
                let left = bounds.left() + inset;
                let width = bounds.size.width - inset * 2.;
                let middle = bounds.top() + bounds.size.height / 2.;
                let half = bounds.size.height / 2. * 0.8;
                let curve = |values: &[f32]| -> Vec<Point<Pixels>> {
                    let last = values.len().saturating_sub(1).max(1) as f32;
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            point(
                                left + width * (index as f32 / last),
                                middle - half * value.clamp(-1., 1.),
                            )
                        })
                        .collect()
                };
                if let Some(path) =
                    polyline(&[point(left, middle), point(left + width, middle)], px(1.))
                {
                    window.paint_path(path, rest);
                }
                for end in [left, left + width] {
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                point(end - px(1.5), middle - px(8.)),
                                size(px(3.), px(16.)),
                            ),
                            anchor,
                        )
                        .corner_radii(px(1.5)),
                    );
                }
                let count = trails.len();
                for (age, trail) in trails.iter().enumerate() {
                    let strength = (age + 1) as f32 / (count + 1) as f32 * 0.35;
                    if let Some(path) = polyline(&curve(trail), px(1.)) {
                        window.paint_path(path, color.opacity(strength));
                    }
                }
                let now = curve(&points);
                let strength = if active { 1. } else { 0.35 };
                if let Some(path) = polyline(&now, px(2.)) {
                    window.paint_path(path, color.opacity(strength));
                }
                if let (Some(contact), true) = (contact, active) {
                    let x = left + width * contact;
                    let index =
                        ((points.len().saturating_sub(1)) as f32 * contact).round() as usize;
                    let y = now.get(index).map(|at| at.y).unwrap_or(middle);
                    let radius = px(5.);
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                point(x - radius, y - radius),
                                size(radius * 2., radius * 2.),
                            ),
                            color,
                        )
                        .corner_radii(radius)
                        .border_widths(px(1.5))
                        .border_color(contact_ring),
                    );
                }
            },
        )
        .size_full()
    }
}

/// A clip's loudness as a row of bars, played part in colour and the rest faint: the scrubber
/// of a preview player.
#[derive(IntoElement)]
pub struct PeakBars {
    peaks: Vec<f32>,
    progress: f32,
    color: Option<Hsla>,
}

impl PeakBars {
    /// Bars for `peaks`, each from 0 to 1.
    pub fn new(peaks: Vec<f32>) -> Self {
        Self {
            peaks,
            progress: 0.,
            color: None,
        }
    }

    /// How much has played, from 0 to 1.
    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0., 1.);
        self
    }

    /// Colour of the played part; defaults to the accent.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for PeakBars {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let played = self.color.unwrap_or(theme.colors.accent);
        let rest = theme.colors.text_faint.opacity(0.45);
        let Self {
            peaks, progress, ..
        } = self;
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let count = peaks.len();
                if count == 0 {
                    return;
                }
                let band = bounds.size.width / count as f32;
                let bar = (band * 0.6).max(px(1.));
                let middle = bounds.top() + bounds.size.height / 2.;
                for (index, peak) in peaks.iter().enumerate() {
                    let height = (bounds.size.height * peak.clamp(0.04, 1.)).max(px(2.));
                    let x = bounds.left() + band * index as f32 + (band - bar) / 2.;
                    let color = if (index as f32 + 0.5) / count as f32 <= progress {
                        played
                    } else {
                        rest
                    };
                    window.paint_quad(
                        fill(
                            Bounds::new(point(x, middle - height / 2.), size(bar, height)),
                            color,
                        )
                        .corner_radii(bar / 2.),
                    );
                }
            },
        )
        .size_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Appearance, Direction};

    #[test]
    fn heat_ramp_hits_its_ends() {
        let theme = Theme::new(Direction::Glass, Appearance::Dark);
        assert_eq!(heat_color(&theme, 0.), theme.colors.heat[0]);
        assert_eq!(heat_color(&theme, 1.), theme.colors.heat[4]);
        assert_eq!(heat_color(&theme, 2.), theme.colors.heat[4], "clamped");
    }
}
