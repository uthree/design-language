//! Liquid metal: a pool of mercury-like chrome that flows behind a welcome screen.
//!
//! It is kept to that one place on purpose. On controls — a ring round a button — it made
//! sizes uneven and said too much; under work it would keep gpui redrawing the whole window.
//!
//! The metal is shaded as a real surface, not painted as flat bands. A handful of metaballs
//! drift on slow orbits; their summed field is the liquid, so drops merge, stretch and pinch off
//! as they pass. The field's height gives every pixel a normal, the view reflects off it, and the
//! reflected ray looks up a studio environment: the theme's reflection ramp
//! ([`crate::Colors::metal`]) from sky to ground with its coloured fringes at the horizon, a
//! softbox above and to the left, and a strip light behind on the right. Where the surface turns
//! away at the silhouette the ray looks behind the metal into the dark of the studio, so every
//! edge darkens the way polished metal does, with the strip light catching a thin rim.
//!
//! gpui takes no custom shaders, so the backdrop brings its own: a small wgpu device beside gpui's
//! renderer shades each frame off screen (`liquid.wgsl`, one pixel per logical pixel, four
//! samples each, with dispersion, ripples and a second softbox), reads it back, and hands it to
//! gpui as an image. It runs at 30 frames a second, a frame is reused until the clock moves on,
//! and it costs nothing once the welcome screen closes.
//!
//! Without a hardware GPU there is no animation, as with the ambient scenes: the metal is shaded
//! once on the CPU, across every core, at 0.6 samples per logical pixel and without the extras,
//! and shown as a still image that answers nothing. It is shaded again only when its size or the
//! theme changes.
//!
//! On the GPU it answers the person in front of it. The pointer pushes drops aside and drags them along,
//! leaving ripples where it crosses the metal; moving or resizing the window leaves the liquid
//! behind for a moment and it sloshes back (see `sim.rs`). While it is answering, the GPU path
//! renders at 60 frames a second.
//!
//! The drops can also be picked up and played with, like the oil on a bowl of ramen: press on
//! the metal to take a drop, carry it, and let go; carried drops fuse with whatever they touch
//! into larger drops. A fused drop sheds a drop now and then, more often when the pointer
//! brushes across it, and tears apart when carried too fast; a double click parts them all. Anything laid over the metal
//! that should take its own clicks — the card, a bar of controls — should `occlude()`, or a
//! press on it will reach the metal beneath.
//!
//! Put the screen's words on a card above the metal, never on it. When
//! [`crate::Motion::enabled`] is off it stands still and answers nothing. While its window is
//! not the active one it stands still too, where it was, and flows on from there when the
//! window comes back: a backdrop in a window nobody is looking at should cost nothing.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, CursorStyle, DispatchPhase, ElementId, Entity, Hitbox, HitboxBehavior, Hsla,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    RenderImage, RenderOnce, Rgba, Styled, Window, canvas, fill,
};
use image::{Frame, ImageBuffer};

mod gpu;
mod sim;

use crate::backdrop::{BackdropPalette, Clock};
use crate::theme::{ActiveTheme, Appearance, Direction};

/// Where the stops of [`crate::Colors::metal`] fall, from straight up (0) through the horizon
/// (0.5, the fifth stop, between the two coloured fringes) to straight down (1).
pub const STOPS: [f32; 9] = [0.0, 0.24, 0.4, 0.45, 0.5, 0.55, 0.64, 0.84, 1.0];

/// The frame drawn when motion is off.
const REST: f32 = 0.12;

/// Samples per logical pixel of the still frame shaded on the CPU.
const CPU_DENSITY: f32 = 0.6;

/// New frames per second while the metal flows.
const FPS: f32 = 30.;
/// New frames per second while the metal answers the pointer or the window.
const LIVELY_FPS: f32 = 60.;

/// A welcome-screen backdrop: liquid chrome flowing slowly over the window's background. Put
/// the screen's words on a card above it, never on the metal.
#[derive(IntoElement)]
pub struct MetalBackdrop {
    id: ElementId,
    palette: BackdropPalette,
    period: Duration,
    phase: Option<f32>,
}

impl MetalBackdrop {
    /// A flowing backdrop on a 30-second cycle.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            palette: BackdropPalette::Theme,
            period: Duration::from_secs(30),
            phase: None,
        }
    }

    /// Finish the metal in `palette` instead of the theme's: its body tinted a little towards
    /// the palette and its fringes in the palette's warm and cool ends.
    pub fn palette(mut self, palette: BackdropPalette) -> Self {
        self.palette = palette;
        self
    }

    /// Draw one still frame at `phase` (0 to 1) of the cycle instead of flowing.
    pub fn phase(mut self, phase: f32) -> Self {
        self.phase = Some(phase);
        self
    }
}

/// The last frame rendered, kept so a redraw at the same moment reuses it, and the liquid's
/// response to the pointer and the window.
#[derive(Default)]
struct Cache {
    key: Option<Key>,
    image: Option<Arc<RenderImage>>,
    sim: sim::Sim,
    clock: Option<Clock>,
}

#[derive(Clone, Copy, PartialEq)]
struct Key {
    palette: BackdropPalette,
    width: u32,
    height: u32,
    frame: u32,
    /// While the liquid is answering the pointer or the window, its own clock, in frames.
    lively: Option<u32>,
    direction: Direction,
    appearance: Appearance,
}

impl RenderOnce for MetalBackdrop {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let cache = window.use_keyed_state(self.id.clone(), cx, |_, _| Cache::default());
        let frames = (self.period.as_secs_f32() * FPS).max(1.);
        // Without a GPU the metal is a still image: shading it on the CPU every frame would cost
        // far more than a backdrop is worth.
        let still = self.phase.is_some() || !gpu::available();
        let flowing = !still && cx.theme().motion.enabled;
        // In an inactive window it holds still where it was.
        let running = flowing && window.is_window_active();
        let period = self.period;
        let clock = cache.update(cx, |cache, _| {
            Clock::advance(&mut cache.clock, running, REST, period)
        });
        let phase = match self.phase {
            Some(phase) => phase,
            None if flowing => clock.phase(period),
            None => REST,
        };
        if running {
            window.request_animation_frame();
        }
        Scene {
            palette: self.palette,
            phase,
            frames,
            interactive: running,
            cache,
        }
    }
}

#[derive(IntoElement)]
struct Scene {
    palette: BackdropPalette,
    phase: f32,
    frames: f32,
    /// Whether the liquid answers the pointer and the window: only while it flows.
    interactive: bool,
    cache: Entity<Cache>,
}

impl RenderOnce for Scene {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |bounds, hitbox, window, cx| paint(&self, bounds, &hitbox, window, cx),
        )
        .size_full()
    }
}

fn paint(
    scene: &Scene,
    bounds: Bounds<Pixels>,
    hitbox: &Hitbox,
    window: &mut Window,
    cx: &mut App,
) {
    let theme = cx.theme();
    let (direction, appearance) = (theme.direction, theme.appearance);
    let (background, metal) = (theme.colors.background, scene.palette.metal(theme));
    window.paint_quad(fill(bounds, background));

    // On the GPU, one pixel per logical pixel, each from four samples; gpui's sampler scales it
    // up on dense displays. The CPU's still frame has a little over half a sample per logical
    // pixel.
    let gpu = gpu::available();
    let density = if gpu { 1. } else { CPU_DENSITY }.min(window.scale_factor());
    let size = [f32::from(bounds.size.width), f32::from(bounds.size.height)];
    let width = (size[0] * density).round().max(1.) as u32;
    let height = (size[1] * density).round().max(1.) as u32;
    let frame = ((scene.phase.rem_euclid(1.)) * scene.frames) as u32;
    let phase = frame as f32 / scene.frames;

    let orbits = drops_at(size[0], size[1], phase);
    if scene.interactive {
        let origin = window.bounds().origin;
        let local = move |pointer: Point<Pixels>| {
            let local = pointer - bounds.origin;
            [
                f32::from(local.x).clamp(0., size[0]),
                f32::from(local.y).clamp(0., size[1]),
            ]
        };
        // A drop in hand follows the pointer wherever it goes; otherwise the liquid only
        // notices the pointer over it.
        let holding = scene.cache.read(cx).sim.holding();
        let pointer = if holding {
            Some(local(window.mouse_position()))
        } else {
            window
                .is_window_hovered()
                .then(|| window.mouse_position())
                .filter(|pointer| bounds.contains(pointer))
                .map(local)
        };
        let input = sim::Input {
            now: Instant::now(),
            origin: [f32::from(origin.x), f32::from(origin.y)],
            size,
            pointer,
            drops: &orbits,
        };
        scene.cache.update(cx, |cache, _| cache.sim.step(input));
        listen(scene, local, hitbox, &orbits, window, cx);
    }

    let cached = scene.cache.read(cx);
    let key = Key {
        palette: scene.palette,
        width,
        height,
        frame,
        lively: cached
            .sim
            .moving()
            .then(|| (cached.sim.clock() * LIVELY_FPS) as u32),
        direction,
        appearance,
    };
    let image = match (&cached.key, &cached.image) {
        (Some(cached_key), Some(image)) if *cached_key == key => image.clone(),
        _ => {
            let studio = Studio::new(metal, background, appearance);
            let drops: Vec<_> = cached
                .sim
                .drops(&orbits)
                .into_iter()
                .map(|(x, y, reach)| (x * density, y * density, reach * density))
                .collect();
            let waves: Vec<_> = cached
                .sim
                .waves(size[0].min(size[1]))
                .map(|[x, y, spread, strength]| {
                    [x * density, y * density, spread * density, strength]
                })
                .collect();
            let pixels = gpu
                .then(|| gpu::render(width, height, phase, &drops, &waves, &studio))
                .flatten()
                .unwrap_or_else(|| render(width, height, &drops, &studio));
            let Some(buffer) = ImageBuffer::from_raw(width, height, pixels) else {
                return;
            };
            let image = Arc::new(RenderImage::new(vec![Frame::new(buffer)]));
            let previous = scene.cache.update(cx, |cache, _| {
                cache.key = Some(key);
                cache.image.replace(image.clone())
            });
            if let Some(previous) = previous {
                window.drop_image(previous).ok();
            }
            image
        }
    };
    window
        .paint_image(bounds, Default::default(), image, 0, false)
        .ok();
}

/// Let the person pick up drops, carry them and let go, and part them with a double click; and
/// show an open hand over the metal and a closed one while a drop is in hand.
fn listen(
    scene: &Scene,
    local: impl Fn(Point<Pixels>) -> [f32; 2] + Copy + 'static,
    hitbox: &Hitbox,
    orbits: &[(f32, f32, f32)],
    window: &mut Window,
    cx: &mut App,
) {
    let sim = &scene.cache.read(cx).sim;
    if sim.holding() {
        window.set_window_cursor_style(CursorStyle::ClosedHand);
    } else if hitbox.is_hovered(window) && sim.over_metal(local(window.mouse_position()), orbits) {
        window.set_cursor_style(CursorStyle::OpenHand, hitbox);
    }

    let (cache, hitbox, orbits) = (scene.cache.clone(), hitbox.clone(), orbits.to_vec());
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble
            || event.button != MouseButton::Left
            || !hitbox.is_hovered(window)
        {
            return;
        }
        let pointer = local(event.position);

        let taken = cache.update(cx, |cache, _| {
            if event.click_count >= 2 {
                cache.sim.scatter();
                true
            } else {
                cache.sim.grab(pointer, &orbits)
            }
        });
        if taken {
            cx.stop_propagation();
            window.refresh();
        }
    });
    let cache = scene.cache.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
            cache.update(cx, |cache, _| cache.sim.release());
        }
    });
    // A release outside the window can go unreported; let go once the button is seen up.
    let cache = scene.cache.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
        if phase == DispatchPhase::Capture
            && event.pressed_button != Some(MouseButton::Left)
            && cache.read(cx).sim.holding()
        {
            cache.update(cx, |cache, _| cache.sim.release());
        }
    });
}

/// One drop of the liquid: where its orbit is centred (in fractions of the box), its radius (in
/// fractions of the box's shorter side), how far it swings and how fast (whole turns per cycle,
/// so the cycle loops seamlessly), and where on its orbit it starts.
struct Drop {
    x: f32,
    y: f32,
    radius: f32,
    swing: (f32, f32),
    turns: (f32, f32),
    start: f32,
}

const fn drop(
    x: f32,
    y: f32,
    radius: f32,
    swing: (f32, f32),
    turns: (f32, f32),
    start: f32,
) -> Drop {
    Drop {
        x,
        y,
        radius,
        swing,
        turns,
        start,
    }
}

/// Three pools that merge and part as their drops pass each other, and two strays.
const DROPS: [Drop; 14] = [
    // A large pool, upper right.
    drop(0.84, 0.3, 0.2, (0.06, 0.05), (1., 2.), 0.),
    drop(0.74, 0.18, 0.13, (0.1, 0.07), (2., 1.), 1.3),
    drop(0.95, 0.46, 0.12, (0.07, 0.1), (1., 1.), 2.6),
    drop(0.7, 0.42, 0.09, (0.12, 0.06), (3., 2.), 4.1),
    drop(0.93, 0.12, 0.08, (0.05, 0.08), (2., 3.), 5.2),
    // A pool lower left.
    drop(0.17, 0.82, 0.17, (0.05, 0.04), (1., 1.), 0.7),
    drop(0.28, 0.7, 0.1, (0.09, 0.08), (2., 1.), 3.3),
    drop(0.07, 0.68, 0.08, (0.06, 0.1), (1., 2.), 4.8),
    // A long pool along the bottom.
    drop(0.56, 0.97, 0.11, (0.09, 0.03), (1., 2.), 1.9),
    drop(0.68, 0.95, 0.1, (0.1, 0.04), (2., 1.), 5.7),
    drop(0.45, 0.99, 0.08, (0.07, 0.03), (1., 3.), 3.1),
    // Strays that wander into the pools and out again.
    drop(0.12, 0.2, 0.07, (0.05, 0.07), (1., 2.), 2.2),
    drop(0.38, 0.1, 0.05, (0.08, 0.05), (2., 1.), 0.4),
    drop(0.5, 0.78, 0.045, (0.16, 0.08), (1., 1.), 4.4),
];

/// A drop's field reaches zero at this multiple of its radius; its surface, where the field
/// alone crosses [`THRESHOLD`], sits at its radius.
const REACH: f32 = 1. / 0.608;
/// The field level of the liquid's surface.
const THRESHOLD: f32 = 0.25;

/// What the metal reflects, and what it sits on.
struct Studio {
    /// The reflection ramp, at [`STOPS`].
    ramp: [[f32; 3]; 9],
    background: [f32; 3],
    /// The dark of the studio behind the metal, seen at its edges.
    behind: [f32; 3],
    /// The fill wall behind the viewer, seen in the faces of the drops.
    front: [f32; 3],
    shadow: f32,
}

fn rgb(color: Hsla) -> [f32; 3] {
    let Rgba { r, g, b, .. } = color.into();
    [r, g, b]
}

impl Studio {
    fn new(ramp: [Hsla; 9], background: Hsla, appearance: Appearance) -> Self {
        let ramp = ramp.map(rgb);
        let horizon = ramp[4];
        Self {
            ramp,
            background: rgb(background),
            behind: mix3(horizon, ramp[6], 0.35),
            front: mix3(ramp[1], ramp[2], 0.5),
            shadow: if appearance.is_dark() { 0.55 } else { 0.2 },
        }
    }

    /// The colour the studio shows along the reflected ray `r` (x right, y down, z towards the
    /// viewer).
    fn look(&self, r: [f32; 3]) -> [f32; 3] {
        // Sky to ground by elevation.
        let elevation = (-r[1]).clamp(-1., 1.).asin();
        let t = 0.5 - elevation / PI;
        let mut color = ramp_at(&self.ramp, t);

        // In front, behind the viewer, a soft fill wall: a drop's face is light, and the
        // horizon reads as a ring round it rather than a band across it.
        let front = smoothstep(0.45, 1., r[2]);
        color = mix3(color, self.front, front * 0.6);

        // Behind the metal the studio is dark: rays that graze the silhouette look there.
        let back = smoothstep(-0.15, -0.9, r[2]);
        color = mix3(color, self.behind, back * 0.85);

        // The softbox, above and to the left in front: a soft-edged rectangle of light.
        let (e1, e2, facing) = project(r, SOFTBOX);
        if facing > 0. {
            let light = smoothstep(0.34, 0.26, e1.abs()) * smoothstep(0.16, 0.1, e2.abs());
            color = mix3(color, [1., 1., 1.], light * 0.95);
        }
        // The strip light, behind on the right: a thin rim along the far edges.
        let strip = dot(r, STRIP);
        let rim = smoothstep(0.93, 0.985, strip);
        mix3(color, [1., 1., 1.], rim * 0.8)
    }
}

/// The softbox's direction (x right, y down, z towards the viewer), normalised.
const SOFTBOX: [f32; 3] = [-0.43, -0.62, 0.656];
/// The strip light's direction, normalised.
const STRIP: [f32; 3] = [0.78, -0.3, -0.55];

/// `r` in a frame around `axis`: across, up, and how much it faces the axis.
fn project(r: [f32; 3], axis: [f32; 3]) -> (f32, f32, f32) {
    let across = normalize(cross(axis, [0., 1., 0.]));
    let up = cross(across, axis);
    (dot(r, across), dot(r, up), dot(r, axis))
}

/// Each drop's centre and the reach of its field, in pixels, at `phase` of the cycle.
fn drops_at(width: f32, height: f32, phase: f32) -> Vec<(f32, f32, f32)> {
    let unit = width.min(height);
    let angle = phase * TAU;
    DROPS
        .iter()
        .map(|d| {
            let x = d.x * width + d.swing.0 * unit * (d.turns.0 * angle + d.start).sin();
            let y = d.y * height + d.swing.1 * unit * (d.turns.1 * angle + d.start * 1.7).cos();
            (x, y, d.radius * unit * REACH)
        })
        .collect()
}

/// The height of the liquid, scaled so a lone drop is a hemisphere.
fn relief(unit: f32) -> f32 {
    0.2 * unit / (1. - THRESHOLD).sqrt()
}

/// How far a drop's shadow falls, and how much wider its field is.
fn drop_shadow(unit: f32) -> (f32, f32) {
    (0.03 * unit, 1.3)
}

/// The CPU's still frame, for when no GPU adapter is available: the same liquid and studio with a
/// single sample per pixel, and without the dispersion, ripples and second softbox. `drops` are
/// each drop's centre and reach in pixels.
fn render(width: u32, height: u32, drops: &[(f32, f32, f32)], studio: &Studio) -> Vec<u8> {
    let unit = (width as f32).min(height as f32);
    let relief = relief(unit);
    let drop_shadow = drop_shadow(unit);

    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let rows = height as usize;
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(rows.max(1));
    let band = rows.div_ceil(threads);
    std::thread::scope(|scope| {
        for (chunk, out) in pixels.chunks_mut(band * width as usize * 4).enumerate() {
            scope.spawn(move || {
                let first = chunk * band;
                let mut near = Vec::with_capacity(drops.len());
                let mut near_shadow = Vec::with_capacity(drops.len());
                for (index, pixel) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    let row = first + index / width as usize;
                    let column = index % width as usize;
                    let (px, py) = (column as f32 + 0.5, row as f32 + 0.5);
                    if column == 0 {
                        // The drops that reach this row, for the metal and for its shadow.
                        near.clear();
                        near_shadow.clear();
                        for &(x, y, reach) in drops {
                            if (py - y).abs() < reach {
                                near.push((x, y, reach));
                            }
                            let (sy, sr) = (y + drop_shadow.0, reach * drop_shadow.1);
                            if (py - sy).abs() < sr {
                                near_shadow.push((x, sy, sr));
                            }
                        }
                    }
                    let (field, gx, gy) = field_at(&near, px, py);
                    let (shadow_field, _, _) = field_at(&near_shadow, px, py);
                    let shade = (shadow_field / 0.45).clamp(0., 1.) * studio.shadow;
                    let ground = studio.background.map(|c| c * (1. - shade));

                    let gradient = (gx * gx + gy * gy).sqrt().max(1e-6);
                    // Signed distance to the surface in pixels, for an anti-aliased edge.
                    let coverage = ((field - THRESHOLD) / gradient + 0.5).clamp(0., 1.);
                    let color = if coverage > 0. {
                        let lift = (field - THRESHOLD).max(1e-4).sqrt();
                        let slope = relief / (2. * lift);
                        let n = normalize([-gx * slope, -gy * slope, 1.]);
                        // The view, straight into the screen, reflected off the surface.
                        let r = [2. * n[2] * n[0], 2. * n[2] * n[1], 2. * n[2] * n[2] - 1.];
                        mix3(ground, studio.look(r), coverage)
                    } else {
                        ground
                    };
                    // BGRA, opaque.
                    pixel[0] = to_byte(color[2]);
                    pixel[1] = to_byte(color[1]);
                    pixel[2] = to_byte(color[0]);
                    pixel[3] = 255;
                }
            });
        }
    });
    pixels
}

/// The liquid's field at a point, and its gradient.
fn field_at(drops: &[(f32, f32, f32)], x: f32, y: f32) -> (f32, f32, f32) {
    let (mut field, mut gx, mut gy) = (0., 0., 0.);
    for &(cx, cy, reach) in drops {
        let (dx, dy) = (x - cx, y - cy);
        let inv = 1. / (reach * reach);
        let q = (dx * dx + dy * dy) * inv;
        if q < 1. {
            let k = 1. - q;
            field += k * k * k;
            let g = -6. * k * k * inv;
            gx += g * dx;
            gy += g * dy;
        }
    }
    (field, gx, gy)
}

/// The ramp at `t`, flat past both ends.
fn ramp_at(ramp: &[[f32; 3]; 9], t: f32) -> [f32; 3] {
    if t <= STOPS[0] {
        return ramp[0];
    }
    for index in 1..STOPS.len() {
        if t <= STOPS[index] {
            let f = (t - STOPS[index - 1]) / (STOPS[index] - STOPS[index - 1]);
            return mix3(ramp[index - 1], ramp[index], f);
        }
    }
    ramp[8]
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt().max(1e-9);
    [v[0] / length, v[1] / length, v[2] / length]
}

fn to_byte(value: f32) -> u8 {
    (value.clamp(0., 1.) * 255. + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn studio() -> Studio {
        let theme = Theme::new(Direction::Glass, Appearance::Dark);
        Studio::new(
            theme.colors.metal,
            theme.colors.background,
            theme.appearance,
        )
    }

    fn luminance(color: [f32; 3]) -> f32 {
        0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2]
    }

    #[test]
    fn edges_are_darker_than_the_face() {
        let studio = studio();
        // Facing the viewer: the ray comes straight back.
        let face = studio.look([0., -0.3, 0.95]);
        // Grazing the silhouette: the ray goes on behind the metal.
        let edge = studio.look([0.05, -0.3, -0.95]);
        assert!(luminance(edge) < luminance(face) * 0.6);
    }

    #[test]
    #[ignore = "timing; run with --release --ignored --nocapture"]
    fn frame_time() {
        let studio = studio();
        let start = std::time::Instant::now();
        for frame in 0..20 {
            render(780, 450, &drops_at(780., 450., frame as f32 / 20.), &studio);
        }
        println!(
            "{:?} per 780 x 450 frame (a 1300 x 750 window)",
            start.elapsed() / 20
        );
    }

    #[test]
    fn a_frame_renders_and_loops() {
        let studio = studio();
        let first = render(64, 40, &drops_at(64., 40., 0.), &studio);
        let last = render(64, 40, &drops_at(64., 40., 1.), &studio);
        assert_eq!(first.len(), 64 * 40 * 4);
        let differing = first
            .iter()
            .zip(&last)
            .filter(|(a, b)| a.abs_diff(**b) > 2)
            .count();
        assert!(differing < first.len() / 100, "the cycle loops seamlessly");
    }
}
