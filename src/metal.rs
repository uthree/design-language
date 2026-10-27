//! Liquid metal: polished, slowly flowing chrome for ornament — a ring around a button, the
//! backdrop of a welcome screen, a mark beside a plan's name.
//!
//! gpui has no custom shaders, so the metal is composed from the primitive its renderer already
//! draws on the GPU with an anti-aliased edge: the rounded rectangle. A shape is painted as a
//! stack of concentric rounded rectangles, one per device pixel of a ring's width (coarser
//! towards the middle of a solid shape), each painted out of horizontal bands, one per segment
//! of the reflection ramp ([`crate::Colors::metal`]), with a two-stop gradient whose stops sit
//! exactly on the band. So the ramp is exact at any size, nothing bands, and every edge is
//! anti-aliased by the same signed-distance shader that draws every other control.
//!
//! What the metal reflects depends on the height in the layer and on its depth into the shape:
//! a ring is a tube, bright along its outer top and inner bottom with the dark horizon running
//! round its middle; a solid shape is a sphere or a pill. The reflection flows by moving the
//! horizon with time, travelling inwards like a ripple.
//!
//! Metal is ornament, never information: put no text on it, give it no state to show, and use it
//! for one thing per window. When [`crate::Motion::enabled`] is off it stands still. Flowing
//! metal keeps gpui redrawing the whole window every frame, so it flows only where no work is on
//! screen; elsewhere, draw a still frame with `.phase(..)`.

use std::f32::consts::{PI, TAU};
use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, BoxShadow, ContentMask, Corners, Div, ElementId, Hsla, IntoElement,
    ParentElement, Pixels, RenderOnce, StyleRefinement, Styled, Window, canvas, div,
    linear_color_stop, linear_gradient, point, px, quad, size, transparent_black,
};

use crate::motion::looping;
use crate::theme::{ActiveTheme, mix};

/// Where the stops of [`crate::Colors::metal`] fall, from the top of a surface (0) to its bottom
/// (1). The horizon is the fifth stop, between the two coloured fringes.
pub const STOPS: [f32; 9] = [0.0, 0.24, 0.4, 0.45, 0.5, 0.55, 0.64, 0.84, 1.0];

/// The default flow cycle.
const PERIOD: Duration = Duration::from_secs(7);

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

/// A liquid-metal shape that fills its parent: a capsule (a sphere when square) by default, or
/// a rounded rectangle with [`LiquidMetal::radius`]; with [`LiquidMetal::ring`], only a metal
/// ring around a fill. See the [module documentation](self).
#[derive(IntoElement)]
pub struct LiquidMetal {
    id: ElementId,
    shape: Shape,
    period: Duration,
    fixed: bool,
}

impl LiquidMetal {
    /// A flowing, solid metal capsule.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            shape: Shape {
                body: Body::Solid,
                radius: None,
                liquid: 1.,
                phase: REST,
            },
            period: PERIOD,
            fixed: false,
        }
    }

    /// Round the corners by `radius` instead of drawing a capsule.
    pub fn radius(mut self, radius: Pixels) -> Self {
        self.shape.radius = Some(radius);
        self
    }

    /// Draw only a metal ring `width` wide, around the theme's surface colour.
    pub fn ring(mut self, width: Pixels) -> Self {
        self.shape.body = Body::Ring { width, fill: None };
        self
    }

    /// The colour inside a ring.
    pub fn fill(mut self, fill: Hsla) -> Self {
        if let Body::Ring { width, .. } = self.shape.body {
            self.shape.body = Body::Ring {
                width,
                fill: Some(fill),
            };
        }
        self
    }

    /// How much the reflection flows, from 0 (still chrome) to 1. Defaults to 1.
    pub fn liquid(mut self, liquid: f32) -> Self {
        self.shape.liquid = liquid.clamp(0., 1.);
        self
    }

    /// How long one flow cycle takes. Keep it slow; the default is seven seconds.
    pub fn period(mut self, period: Duration) -> Self {
        self.period = period;
        self
    }

    /// Draw one still frame at `phase` (0 to 1) of the cycle instead of flowing.
    pub fn phase(mut self, phase: f32) -> Self {
        self.shape.phase = phase;
        self.fixed = true;
        self
    }
}

impl RenderOnce for LiquidMetal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let frame = Frame(self.shape);
        if self.fixed || self.shape.liquid == 0. {
            return frame.into_any_element();
        }
        looping(frame, self.id, self.period, REST, cx, |frame, phase| {
            frame.at(phase)
        })
    }
}

#[derive(IntoElement)]
struct Frame(Shape);

impl Frame {
    fn at(mut self, phase: f32) -> Self {
        self.0.phase = phase;
        self
    }
}

impl RenderOnce for Frame {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let shape = self.0;
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| paint_shape(&shape, bounds, window, cx),
        )
        .size_full()
    }
}

/// A metal ring around its children: a button, a tool palette, a segmented control. The ring
/// sits outside the content, so wrap the control as it is.
///
/// ```ignore
/// MetalFrame::new("new-frame").child(Button::new("new", "新しいプロジェクト").primary())
/// ```
#[derive(IntoElement)]
pub struct MetalFrame {
    id: ElementId,
    base: Div,
    width: Pixels,
    radius: Option<Pixels>,
    fill: Option<Hsla>,
    liquid: f32,
    phase: Option<f32>,
}

impl MetalFrame {
    /// A 4 px ring that follows the theme's control shape.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            base: div(),
            width: px(4.),
            radius: None,
            fill: None,
            liquid: 1.,
            phase: None,
        }
    }

    /// The width of the ring.
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    /// The outer corner radius. Defaults to a capsule under capsule-shaped controls, otherwise
    /// the control radius plus the ring, so the ring stays concentric with the control.
    pub fn radius(mut self, radius: Pixels) -> Self {
        self.radius = Some(radius);
        self
    }

    /// The colour inside the ring, behind the content. Defaults to the theme's surface.
    pub fn fill(mut self, fill: Hsla) -> Self {
        self.fill = Some(fill);
        self
    }

    /// How much the reflection flows, from 0 to 1.
    pub fn liquid(mut self, liquid: f32) -> Self {
        self.liquid = liquid;
        self
    }

    /// Draw one still frame at `phase` instead of flowing.
    pub fn phase(mut self, phase: f32) -> Self {
        self.phase = Some(phase);
        self
    }
}

impl Styled for MetalFrame {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for MetalFrame {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements)
    }
}

impl RenderOnce for MetalFrame {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let radius = self.radius.or_else(|| {
            (!theme.shape.capsule_controls).then(|| theme.shape.radius_sm + self.width)
        });
        let mut metal = LiquidMetal::new(self.id)
            .ring(self.width)
            .fill(self.fill.unwrap_or(theme.colors.surface))
            .liquid(self.liquid);
        if let Some(radius) = radius {
            metal = metal.radius(radius);
        }
        if let Some(phase) = self.phase {
            metal = metal.phase(phase);
        }
        // The ring paints first, so the content sits on top of it.
        div()
            .relative()
            .flex_none()
            .child(div().absolute().inset_0().child(metal))
            .child(
                self.base
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p(self.width),
            )
    }
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
                    let at = Bounds::new(point(px(x - w / 2.), px(y - h / 2.)), size(px(w), px(h)));
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
            },
        )
        .size_full()
    }
}

/// The ramp at `r`, extended flat past both ends so a moving horizon never runs out of colour.
fn ramp_at(ramp: &[Hsla; 9], r: f32) -> Hsla {
    if r <= STOPS[0] {
        return ramp[0];
    }
    for index in 1..STOPS.len() {
        if r <= STOPS[index] {
            let t = (r - STOPS[index - 1]) / (STOPS[index] - STOPS[index - 1]);
            return mix(ramp[index - 1], ramp[index], t);
        }
    }
    ramp[8]
}

fn paint_shape(shape: &Shape, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let theme = cx.theme();
    let ramp = theme.colors.metal;
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
            let base = ramp_at(&ramp, mid + (to_shape(v) - 0.5) * slope);
            if edge > 0. {
                mix(base, ramp[6], edge)
            } else {
                base
            }
        };

        // Where the reflection crosses a stop, in fractions of the layer's height.
        let mut cuts = vec![0., 1.];
        if slope.abs() > 1e-3 {
            for stop in STOPS {
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
            // The softbox: a soft highlight on the upper left, drifting with the flow.
            let side = width.min(height);
            let glint = Bounds::new(
                point(
                    bounds.left() + px(width * 0.5 - side * (0.24 - flow * (wave + 0.6).sin())),
                    bounds.top() + px(height * 0.5 - side * 0.3),
                ),
                size(px(side * 0.16), px(side * 0.07)),
            );
            window.paint_shadows(
                glint,
                Corners::all(px(side * 0.035)),
                &[BoxShadow {
                    color: gpui::white().opacity(0.8),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(side * 0.04),
                    spread_radius: px(0.),
                }],
            );
        }
    }
}
