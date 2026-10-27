//! Liquid metal: a polished, slowly flowing chrome surface for ornament — a brand mark, a hero,
//! the moment a long job finishes.
//!
//! gpui has no custom shaders, so the surface is composed from the primitives its renderer
//! already runs on the GPU. The shape is cut into one column per device pixel; each column is a
//! stack of quads with two-stop linear gradients, one per segment of the reflection ramp
//! ([`crate::Colors::metal`]), so the ramp is exact at any size and nothing bands. The surface
//! "flows" by moving where the reflected horizon crosses each column, and a mercury drop also
//! wobbles its outline. A specular streak and a one-pixel rim finish it.
//!
//! Metal is ornament, never information: put no text on it, give it no state to show, and use it
//! once per window at most. It is achromatic so it can never be mistaken for the accent, a
//! signal or the agent. When [`crate::Motion::enabled`] is off it stands still.
//!
//! Size it with its parent, like the plots in [`crate::viz`]:
//!
//! ```ignore
//! div().w(px(240.)).h(px(150.)).child(LiquidMetal::new("hero"))
//! ```

use std::f32::consts::{PI, TAU};
use std::time::Duration;

use gpui::{
    App, Bounds, ElementId, Hsla, IntoElement, PathBuilder, Pixels, RenderOnce, Styled, Window,
    canvas, fill, linear_color_stop, linear_gradient, point, px, size,
};

use crate::motion::looping;
use crate::theme::{ActiveTheme, mix};

/// Where the stops of [`crate::Colors::metal`] fall, from the top of a surface (0) to its bottom
/// (1). The horizon is the jump from the third stop to the fourth.
pub const STOPS: [f32; 7] = [0.0, 0.27, 0.45, 0.5, 0.6, 0.84, 1.0];

/// The outline the metal fills.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MetalShape {
    /// A rounded, slightly heavy drop that fills its box: a mark, a hero, an icon-sized jewel.
    #[default]
    Drop,
    /// A capsule that fills its box: a bar or a pill set beside a label.
    Capsule,
}

/// A liquid-metal surface. See the [module documentation](self).
#[derive(IntoElement)]
pub struct LiquidMetal {
    id: ElementId,
    shape: MetalShape,
    liquid: f32,
    period: Duration,
    phase: Option<f32>,
}

impl LiquidMetal {
    /// A flowing mercury drop.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            shape: MetalShape::Drop,
            liquid: 1.,
            period: Duration::from_secs(9),
            phase: None,
        }
    }

    /// The outline to fill.
    pub fn shape(mut self, shape: MetalShape) -> Self {
        self.shape = shape;
        self
    }

    /// How liquid the metal is, from 0 (rigid chrome: nothing moves but the light) to 1
    /// (mercury: the outline wobbles too). Defaults to 1.
    pub fn liquid(mut self, liquid: f32) -> Self {
        self.liquid = liquid.clamp(0., 1.);
        self
    }

    /// How long one flow cycle takes. Keep it slow; the default is nine seconds.
    pub fn period(mut self, period: Duration) -> Self {
        self.period = period;
        self
    }

    /// Draw one still frame at `phase` (0 to 1) of the cycle instead of flowing.
    pub fn phase(mut self, phase: f32) -> Self {
        self.phase = Some(phase);
        self
    }
}

/// The frame shown when motion is off.
const REST: f32 = 0.12;

impl RenderOnce for LiquidMetal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let surface = Surface {
            shape: self.shape,
            liquid: self.liquid,
            phase: self.phase.unwrap_or(REST),
        };
        if self.phase.is_some() {
            return surface.into_any_element();
        }
        looping(surface, self.id, self.period, REST, cx, |surface, delta| {
            surface.at(delta)
        })
    }
}

/// One frame of the surface.
#[derive(IntoElement)]
struct Surface {
    shape: MetalShape,
    liquid: f32,
    phase: f32,
}

impl Surface {
    fn at(mut self, phase: f32) -> Self {
        self.phase = phase;
        self
    }
}

impl RenderOnce for Surface {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| paint(&self, bounds, window, cx),
        )
        .size_full()
    }
}

/// Where the surface crosses one column: its top and bottom edges, and how near the column is to
/// the side of the shape (0 in the body, 1 at the silhouette), which darkens it as a curved
/// surface turns away.
struct Column {
    top: f32,
    bottom: f32,
    side: f32,
}

/// The column through `x` (-1 at the left of the box, 1 at the right), in pixels from the top
/// of the box, for a box `width` × `height`.
fn column(surface: &Surface, x: f32, width: f32, height: f32, phase: f32) -> Option<Column> {
    let liquid = surface.liquid;
    match surface.shape {
        MetalShape::Capsule => {
            let radius = height.min(width) / 2.;
            let from_center = x.abs() * width / 2.;
            let straight = width / 2. - radius;
            let into_cap = (from_center - straight).max(0.);
            if into_cap >= radius {
                return None;
            }
            let half = (radius * radius - into_cap * into_cap).sqrt();
            Some(Column {
                top: height / 2. - half,
                bottom: height / 2. + half,
                side: (into_cap / radius).powi(3),
            })
        }
        MetalShape::Drop => {
            // Room for the wobble so the outline never leaves the box.
            let reach = 1. + 0.03 * liquid;
            let breathe = 1. + 0.025 * liquid * (phase).sin();
            let x = x * reach / breathe;
            if x.abs() >= 1. {
                return None;
            }
            // A superellipse with a little more mass than an ellipse, slightly heavier below.
            let body = (1. - x.abs().powf(2.3)).powf(1. / 2.3);
            let wobble_top = liquid
                * (0.035 * (PI * x + phase).sin()
                    + 0.015 * (2.2 * PI * x - 2. * phase + 1.3).sin());
            let wobble_bottom = liquid
                * (0.03 * (1.3 * PI * x - phase + 0.7).sin()
                    + 0.012 * (2.6 * PI * x + 3. * phase).sin());
            let ry = height / 2. / (1. + 0.05 * liquid);
            let center = height / 2. + ry * 0.04;
            Some(Column {
                top: center - ry * 0.93 * body * (1. + wobble_top),
                bottom: center + ry * 1.03 * body * (1. + wobble_bottom),
                side: x.abs().powi(4),
            })
        }
    }
}

/// How far the reflection is pushed down (in fractions of the column) at `x`: the horizon tilts
/// a little and, on liquid metal, flows.
fn flow(surface: &Surface, x: f32, phase: f32) -> f32 {
    let still = 0.035 * x;
    let moving = 0.045 * (1.7 * x + phase).sin() + 0.022 * (3.3 * x - 2. * phase + 0.4).sin();
    // Rigid chrome keeps a slower, smaller sway: the light moves, not the metal.
    let amount = 0.35 + 0.65 * surface.liquid;
    still + amount * moving
}

fn paint(surface: &Surface, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let theme = cx.theme();
    let ramp = theme.colors.metal;
    let shade = ramp[4];
    let rim = theme.colors.border_strong;
    let phase = surface.phase * TAU;

    let scale = window.scale_factor();
    let pixel = 1. / scale;
    let left = f32::from(bounds.left());
    let top = f32::from(bounds.top());
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if width <= 0. || height <= 0. {
        return;
    }
    // Columns on device pixels, so neighbours meet exactly and leave no seam.
    let start = (left * scale).floor() / scale;
    let count = ((left + width - start) * scale).ceil() as usize;

    // The ramp, extended flat past both ends so a flowing horizon never runs out of colour.
    let mut stops = vec![(-1., ramp[0])];
    stops.extend(STOPS.iter().copied().zip(ramp));
    stops.push((2., ramp[6]));

    let mut outline_top = Vec::with_capacity(count);
    let mut outline_bottom = Vec::with_capacity(count);
    for index in 0..count {
        let x0 = start + index as f32 * pixel;
        let unit = ((x0 + pixel / 2. - left) / width) * 2. - 1.;
        let Some(cut) = column(surface, unit, width, height, phase) else {
            continue;
        };
        let span = cut.bottom - cut.top;
        if span <= 0. {
            continue;
        }
        let offset = flow(surface, unit, phase);
        let darken = 0.6 * cut.side;
        let color_at = |r: f32| -> Hsla {
            let index = stops
                .windows(2)
                .position(|pair| r <= pair[1].0)
                .unwrap_or(stops.len() - 2);
            let (from, to) = (stops[index], stops[index + 1]);
            let t = ((r - from.0) / (to.0 - from.0)).clamp(0., 1.);
            mix(mix(from.1, to.1, t), shade, darken)
        };

        // One quad per ramp segment the column crosses, each a two-stop gradient.
        for pair in stops.windows(2) {
            let v0 = (pair[0].0 - offset).max(0.);
            let v1 = (pair[1].0 - offset).min(1.);
            if v1 <= v0 {
                continue;
            }
            let y0 = top + cut.top + v0 * span;
            // Each segment runs one pixel into the next, which paints over it: no seam.
            let y1 = top + cut.top + v1 * span + if v1 < 1. { pixel } else { 0. };
            let quad = Bounds::new(point(px(x0), px(y0)), size(px(pixel), px(y1 - y0)));
            window.paint_quad(fill(
                quad,
                linear_gradient(
                    180.,
                    linear_color_stop(color_at(v0 + offset), 0.),
                    linear_color_stop(color_at(v1 + offset), 1.),
                ),
            ));
        }

        // The softbox: a crisp highlight in the sky, drifting slowly across the surface.
        let centre = -0.3 + 0.14 * (phase + 0.6).sin();
        let reach = match surface.shape {
            MetalShape::Drop => 0.26,
            MetalShape::Capsule => 0.6 * height / width,
        };
        let strength = 0.9 * (-((unit - centre) / reach).powi(4)).exp() * (1. - cut.side).powi(2);
        if strength > 0.02 {
            let glint = gpui::white();
            let middle = (0.17 - 0.5 * offset).clamp(0.08, 0.4);
            let (half, edge) = (0.035, 0.02);
            for (from, to, a0, a1) in [
                (middle - half - edge, middle - half, 0., strength),
                (middle - half, middle + half, strength, strength),
                (middle + half, middle + half + edge, strength, 0.),
            ] {
                let y0 = top + cut.top + from.max(0.) * span;
                let y1 = top + cut.top + to.min(1.) * span;
                if y1 <= y0 {
                    continue;
                }
                window.paint_quad(fill(
                    Bounds::new(point(px(x0), px(y0)), size(px(pixel), px(y1 - y0))),
                    linear_gradient(
                        180.,
                        linear_color_stop(glint.opacity(a0), 0.),
                        linear_color_stop(glint.opacity(a1), 1.),
                    ),
                ));
            }
        }

        let x = px(x0 + pixel / 2.);
        outline_top.push(point(x, px(top + cut.top)));
        outline_bottom.push(point(x, px(top + cut.bottom)));
    }

    // A hairline rim keeps the silhouette crisp on any background.
    if outline_top.len() > 1 {
        let mut builder = PathBuilder::stroke(px(1.));
        builder.move_to(outline_top[0]);
        for at in outline_top.iter().skip(1) {
            builder.line_to(*at);
        }
        for at in outline_bottom.iter().rev() {
            builder.line_to(*at);
        }
        builder.close();
        if let Ok(path) = builder.build() {
            window.paint_path(path, rim);
        }
    }
}
