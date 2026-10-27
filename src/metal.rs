//! Liquid metal: polished, slowly flowing chrome for the background of a welcome screen.
//!
//! It is kept to that one place on purpose. On controls — a ring round a button — it made
//! sizes uneven and said too much; under work it would keep gpui redrawing the whole window.
//!
//! gpui has no custom shaders, so the metal is composed from the primitive its renderer already
//! draws on the GPU with an anti-aliased edge: the rounded rectangle. A shape is painted as a
//! stack of concentric rounded rectangles, one per device pixel of a ring's width (coarser
//! towards the middle of a solid shape), each painted out of horizontal bands, one per segment
//! of the reflection ramp ([`crate::Colors::metal`]), with a two-stop gradient whose stops sit
//! exactly on the band. So the ramp is exact at any size, nothing bands, and every edge is
//! anti-aliased by the same signed-distance shader that draws every other control.
//!
//! What the metal reflects depends on the height in the shape and on the depth into it: a ring
//! is a tube, bright along its outer top and inner bottom with the dark horizon running round
//! its middle; a solid shape is a sphere or a pill. The reflection flows by moving the horizon
//! with time, travelling inwards like a ripple, while the pieces drift.
//!
//! Put the screen's words on a card above the metal, never on it. When
//! [`crate::Motion::enabled`] is off it stands still.

use std::f32::consts::{PI, TAU};
use std::time::Duration;

use gpui::{
    App, Bounds, BoxShadow, ContentMask, Corners, ElementId, Hsla, IntoElement, Pixels, RenderOnce,
    Styled, Window, canvas, linear_color_stop, linear_gradient, point, px, quad, size,
    transparent_black,
};

use crate::motion::looping;
use crate::theme::{ActiveTheme, mix};

/// Where the stops of [`crate::Colors::metal`] fall, from the top of a surface (0) to its bottom
/// (1). The horizon is the fifth stop, between the two coloured fringes.
pub const STOPS: [f32; 9] = [0.0, 0.24, 0.4, 0.45, 0.5, 0.55, 0.64, 0.84, 1.0];

/// The frame drawn when motion is off.
const REST: f32 = 0.12;

/// Whether a shape is metal through, or a metal ring around a fill.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Body {
    Solid,
    Ring { width: Pixels, fill: Option<Hsla> },
}

/// One frame of a metal shape.
#[derive(Clone, Copy, Debug)]
struct Shape {
    body: Body,
    /// Corner radius; `None` for a capsule (a circle when the box is square).
    radius: Option<Pixels>,
    liquid: f32,
    phase: f32,
}

/// A welcome-screen backdrop: a few metal rings and drops drifting slowly over the window's
/// background. Put the screen's words on a card above it, never on the metal.
#[derive(IntoElement)]
pub struct MetalBackdrop {
    id: ElementId,
    period: Duration,
    phase: Option<f32>,
}

impl MetalBackdrop {
    /// A drifting backdrop on a 24-second cycle.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            period: Duration::from_secs(24),
            phase: None,
        }
    }

    /// Draw one still frame at `phase` instead of drifting.
    pub fn phase(mut self, phase: f32) -> Self {
        self.phase = Some(phase);
        self
    }
}

impl RenderOnce for MetalBackdrop {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let scene = Scene(self.phase.unwrap_or(REST));
        if self.phase.is_some() {
            return scene.into_any_element();
        }
        looping(scene, self.id, self.period, REST, cx, |_, phase| {
            Scene(phase)
        })
    }
}

#[derive(IntoElement)]
struct Scene(f32);

/// One piece of the backdrop: its centre (in fractions of the box), its size (in fractions of
/// the box's shorter side), its ring width (0 for a solid drop) and where its drift starts.
struct Piece {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    ring: f32,
    drift: f32,
}

const SCENE: [Piece; 6] = [
    Piece {
        x: 0.84,
        y: 0.28,
        width: 0.62,
        height: 0.62,
        ring: 0.075,
        drift: 0.,
    },
    Piece {
        x: 0.12,
        y: 0.2,
        width: 0.22,
        height: 0.22,
        ring: 0.04,
        drift: 2.1,
    },
    Piece {
        x: 0.16,
        y: 0.8,
        width: 0.34,
        height: 0.34,
        ring: 0.,
        drift: 1.3,
    },
    Piece {
        x: 0.62,
        y: 0.92,
        width: 0.5,
        height: 0.15,
        ring: 0.,
        drift: 4.,
    },
    Piece {
        x: 0.94,
        y: 0.8,
        width: 0.17,
        height: 0.17,
        ring: 0.,
        drift: 3.2,
    },
    Piece {
        x: 0.38,
        y: 0.1,
        width: 0.1,
        height: 0.1,
        ring: 0.,
        drift: 5.1,
    },
];

impl RenderOnce for Scene {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let phase = self.0;
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                let theme = cx.theme();
                let background = theme.colors.background;
                let shadow = theme.colors.shadow;
                let dark = theme.appearance.is_dark();
                window.paint_quad(gpui::fill(bounds, background));
                // Pieces run off the edges; keep them, and their shadows, inside the box.
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    let unit = f32::from(bounds.size.width.min(bounds.size.height));
                    let angle = phase * TAU;
                    for piece in &SCENE {
                        let x = f32::from(bounds.left())
                            + f32::from(bounds.size.width) * piece.x
                            + unit * 0.025 * (angle + piece.drift).sin();
                        let y = f32::from(bounds.top())
                            + f32::from(bounds.size.height) * piece.y
                            + unit * 0.02 * (angle * 2. + piece.drift).cos();
                        let (w, h) = (unit * piece.width, unit * piece.height);
                        let at =
                            Bounds::new(point(px(x - w / 2.), px(y - h / 2.)), size(px(w), px(h)));
                        let radius = px(w.min(h) / 2.);
                        window.paint_shadows(
                            at,
                            Corners::all(radius),
                            &[BoxShadow {
                                color: shadow.opacity(if dark { 0.5 } else { 0.16 }),
                                offset: point(px(0.), px(unit * 0.02)),
                                blur_radius: px(unit * 0.04),
                                spread_radius: px(0.),
                            }],
                        );
                        let body = if piece.ring > 0. {
                            Body::Ring {
                                width: px(unit * piece.ring),
                                fill: Some(background),
                            }
                        } else {
                            Body::Solid
                        };
                        let shape = Shape {
                            body,
                            radius: None,
                            liquid: 1.,
                            phase: (phase * 3. + piece.drift / TAU).fract(),
                        };
                        paint_shape(&shape, at, window, cx);
                    }
                });
            },
        )
        .size_full()
    }
}

/// The reflection the metal paints: the theme's ramp with a studio softbox added — a crisp,
/// near-white highlight high in the sky and a faint second one in the ground — which is what
/// makes polished metal glint rather than merely shine.
fn reflection(ramp: &[Hsla; 9]) -> Vec<(f32, Hsla)> {
    let white = gpui::white();
    let mut stops = vec![
        (0., ramp[0]),
        (0.1, mix(ramp[0], ramp[1], 0.4)),
        (0.135, mix(ramp[1], white, 0.85)),
        (0.17, white),
        (0.195, mix(ramp[1], white, 0.7)),
    ];
    stops.extend(STOPS.iter().copied().zip(ramp.iter().copied()).skip(1));
    // The second softbox, reflected in the floor just above the bounce light.
    stops.insert(stops.len() - 1, (0.92, mix(ramp[8], white, 0.45)));
    stops
}

/// The reflection at `r`, extended flat past both ends so a moving horizon never runs out of
/// colour.
fn reflection_at(stops: &[(f32, Hsla)], r: f32) -> Hsla {
    if r <= stops[0].0 {
        return stops[0].1;
    }
    for pair in stops.windows(2) {
        let ((from, a), (to, b)) = (pair[0], pair[1]);
        if r <= to {
            return mix(a, b, (r - from) / (to - from));
        }
    }
    stops[stops.len() - 1].1
}

fn paint_shape(shape: &Shape, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let theme = cx.theme();
    let ramp = theme.colors.metal;
    let stops = reflection(&ramp);
    let surface = theme.colors.surface;
    let scale = window.scale_factor();
    let pixel = 1. / scale;
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if width <= 0. || height <= 0. {
        return;
    }
    let radius = shape
        .radius
        .map(f32::from)
        .unwrap_or(f32::MAX)
        .min(width.min(height) / 2.);
    let flow = shape.liquid * 0.07;
    let wave = shape.phase * TAU;

    // How deep the metal goes, and in how many layers. A ring gets one per device pixel; a
    // solid shape gets coarser layers towards its middle, where the shading changes slowly.
    let (depth, layers) = match shape.body {
        Body::Ring { width: ring, .. } => {
            let depth = f32::from(ring).min(width.min(height) / 2.);
            (depth, ((depth * scale).ceil() as usize).max(1))
        }
        Body::Solid => {
            let depth = width.min(height) / 2.;
            (depth, ((depth * scale / 2.).ceil() as usize).clamp(4, 48))
        }
    };

    let turn = match shape.body {
        Body::Ring { width: ring, .. } => ((f32::from(ring) - 3.) / 8.).clamp(0.3, 1.),
        Body::Solid => 1.,
    };

    for layer in 0..layers {
        let inset = depth * layer as f32 / layers as f32;
        let d = (layer as f32 + 0.5) / layers as f32;
        // The reflection at height y (0 at the top of the shape, 1 at its bottom) is
        // r = mid + (y - 0.5) * slope; `edge` darkens the layer towards the ground. Heights are
        // the shape's, not the layer's, so neighbouring layers differ only by a little depth.
        let (slope, mid, edge) = match shape.body {
            // A tube: upright reflection on its outer side, inverted on its inner side, the
            // horizon round its middle.
            // A thin ring has too few pixels to show a tube, so it turns less of the way and
            // reads as a bevel, lit from above, with the fringes at its sides.
            Body::Ring { .. } => (
                1.15 * (PI * d * turn).cos(),
                0.5 + flow * (wave + 2.5 * d).sin(),
                if layer == 0 { 0.3 } else { 0. },
            ),
            // A sphere or pill: the reflection tightens and the horizon bows down towards the
            // middle, and the rim turns away into the darker ground.
            Body::Solid => (
                1. - 0.4 * d,
                0.5 - 0.07 * d + flow * (wave + 3. * d).sin(),
                (1. - d / 0.15).max(0.) * 0.4,
            ),
        };

        let x0 = f32::from(bounds.left()) + inset;
        let y0 = f32::from(bounds.top()) + inset;
        let (w, h) = (width - 2. * inset, height - 2. * inset);
        if w <= pixel || h <= pixel {
            break;
        }
        let rect = Bounds::new(point(px(x0), px(y0)), size(px(w), px(h)));
        let corner = Corners::all(px((radius - inset).max(0.)));
        // From a fraction of the layer's height to a fraction of the shape's, and back.
        let to_shape = |v: f32| (inset + v * h) / height;
        let to_layer = |y: f32| (y * height - inset) / h;
        let color = |v: f32| -> Hsla {
            let y = to_shape(v);
            let base = reflection_at(&stops, mid + (y - 0.5) * slope);
            if edge > 0. {
                // The rim turns away: into the ground above, into the bounce light below.
                let below = ((y - 0.55) / 0.35).clamp(0., 1.);
                mix(base, mix(ramp[6], ramp[8], below), edge)
            } else {
                base
            }
        };

        // Where the reflection crosses a stop, in fractions of the layer's height.
        let mut cuts = vec![0., 1.];
        if slope.abs() > 1e-3 {
            for (stop, _) in &stops {
                let v = to_layer(0.5 + (stop - mid) / slope);
                if v > 0. && v < 1. {
                    cuts.push(v);
                }
            }
        }
        cuts.sort_by(|a, b| a.total_cmp(b));

        // Band edges on device pixels, so neighbouring bands meet without a seam. Each band
        // paints the whole rounded layer, clipped to the band, so its edge stays anti-aliased.
        let snap = |v: f32| ((y0 + v * h) * scale).round() / scale;
        for pair in cuts.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            let (top, bottom) = (
                if from == 0. { y0 - pixel } else { snap(from) },
                if to == 1. { y0 + h + pixel } else { snap(to) },
            );
            if bottom <= top {
                continue;
            }
            let band = Bounds::new(
                point(px(x0 - pixel), px(top)),
                size(px(w + 2. * pixel), px(bottom - top)),
            );
            let background = linear_gradient(
                180.,
                linear_color_stop(color(from), from),
                linear_color_stop(color(to), to),
            );
            window.with_content_mask(Some(ContentMask { bounds: band }), |window| {
                window.paint_quad(quad(
                    rect,
                    corner,
                    background,
                    px(0.),
                    transparent_black(),
                    Default::default(),
                ));
            });
        }
    }

    match shape.body {
        Body::Ring { fill, .. } => {
            let (w, h) = (width - 2. * depth, height - 2. * depth);
            if w > 0. && h > 0. {
                let rect = Bounds::new(
                    point(bounds.left() + px(depth), bounds.top() + px(depth)),
                    size(px(w), px(h)),
                );
                window.paint_quad(
                    gpui::fill(rect, fill.unwrap_or(surface))
                        .corner_radii(Corners::all(px((radius - depth).max(0.)))),
                );
            }
        }
        Body::Solid => {
            // Bloom: the softbox in the reflection is bright enough to glow a little past its
            // edges, on the upper left where it peaks.
            let side = width.min(height);
            let drift = flow * (wave + 0.6).sin();
            let glow = Bounds::new(
                point(
                    bounds.left() + px(width * 0.5 - side * (0.24 - drift)),
                    bounds.top() + px(height * 0.5 - side * 0.36),
                ),
                size(px(side * 0.2), px(side * 0.06)),
            );
            window.paint_shadows(
                glow,
                Corners::all(px(side * 0.03)),
                &[BoxShadow {
                    color: gpui::white().opacity(0.45),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(side * 0.07),
                    spread_radius: px(0.),
                }],
            );
        }
    }
}
