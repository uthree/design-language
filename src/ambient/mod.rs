//! Ambient backdrops: slow, abstract scenes of light and colour to fill a welcome screen, in the
//! manner of a desktop wallpaper, beside the liquid metal ([`crate::metal`]).
//!
//! - [`Ambience::Aurora`]: curtains of light folding over a night sky.
//! - [`Ambience::Dunes`]: sheets of silk lying in layers, their crests rolling.
//! - [`Ambience::Bloom`]: great clouds of colour drifting and melting into one another.
//! - [`Ambience::Caustics`]: light through moving water, playing over a deep floor.
//! - [`Ambience::Solar`]: a low sun in a sky of graded light, over ridges in haze.
//! - [`Ambience::Galaxy`]: a spiral galaxy turning slowly in deep space.
//! - [`Ambience::Jellyfish`]: jellyfish drifting up through deep water, lit from within.
//! - [`Ambience::Shoal`]: schools of small fish wheeling through sunlit shallows.
//! - [`Ambience::Formation`]: points of light gathering into one turning figure after another.
//! - [`Ambience::Garden`]: a row of plants growing, blooming and fading over the cycle.
//!
//! Every one is drawn from the theme: its background, the agent gradient and the accent, so it
//! sits with the rest of the screen in each direction and appearance. A [`BackdropPalette`]
//! replaces the gradient and accent with colours of its own. In a dark appearance the
//! scene is light over the dark; in a light one, washes of colour over the light.
//!
//! Like the metal, each brings its own shader: a WGSL file shaded off screen on the crate's wgpu
//! device, one sample per logical pixel with a little grain so long gradients don't band, read
//! back and handed to gpui as an image. It runs at 30 frames a second, 60 while it answers, a
//! frame is reused until the clock moves on, and it costs nothing once the welcome screen closes.
//!
//! Without a hardware GPU there is no animation: the scene is shaded once on the CPU, across every
//! core, at half a sample per logical pixel and without the caustics' fringes, and shown as a
//! still image that answers nothing. It is shaded again only when its size or the theme changes.
//!
//! On the GPU each answers the window (see `sim.rs`): moving or resizing it leaves the scene
//! behind for a moment and it swings back, nearer layers further, so the window has depth. Most
//! also answer the pointer, each in its own way (see [`Ambience`]); Bloom, Caustics, Galaxy,
//! Jellyfish, Formation and Garden leave ripples as it travels. Formation's points and Garden's
//! plants are laid out on the CPU each frame and handed to their shaders (see `figures.rs`). Aurora's curtains leave it alone, but its brighter stars
//! wake as the pointer passes and join into constellations, and lines can be drawn between them
//! (see `stars.rs`). Dunes leaves it alone, since ripples through a stack of silk would break the
//! stillness it is there for. Anything laid over a backdrop that takes clicks should `occlude()`,
//! or presses on it reach Aurora's stars underneath.
//!
//! Put the screen's words on a card above the backdrop, never on it. When
//! [`crate::Motion::enabled`] is off it stands still and answers nothing. While its window is
//! not the active one it holds still where it was, and drifts on from there when the window
//! comes back.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, CursorStyle, DispatchPhase, ElementId, Entity, Hitbox, HitboxBehavior, Hsla,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    RenderImage, RenderOnce, Rgba, Styled, Window, canvas, fill,
};
use image::{Frame as ImageFrame, ImageBuffer};

mod cpu;
mod figures;
mod gpu;
mod sim;
mod stars;

use crate::backdrop::{BackdropPalette, Clock};
use crate::theme::{ActiveTheme, Appearance, Direction, Theme};

/// The frame drawn when motion is off.
const REST: f32 = 0.12;

/// Samples per logical pixel of the still frame shaded on the CPU.
const CPU_DENSITY: f32 = 0.5;

/// New frames per second while the scene drifts.
const FPS: f32 = 30.;
/// New frames per second while the scene answers the pointer or the window.
const LIVELY_FPS: f32 = 60.;

/// Which scene an [`AmbientBackdrop`] shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ambience {
    /// Curtains of light folding slowly over a night sky, mirrored in still water. The window
    /// swings them; the pointer leaves them alone but wakes the brighter stars as it passes,
    /// joining them into constellations. Press on a star and let go over another to draw a line
    /// between them that stays (draw it again to take it away), press on the open sky to send a
    /// shooting star, and double-click to wipe the lines away.
    Aurora,
    /// Sheets of silk in layers, their crests rolling, each with a sheen and a soft shadow. The
    /// window slides the layers apart; the pointer leaves them alone.
    Dunes,
    /// Great clouds of colour drifting and melting together under a frosted sheen. The pointer
    /// stirs the colour round itself; the window leaves the clouds sloshing.
    Bloom,
    /// Light through moving water, in a net of bright lines with coloured fringes over a deep
    /// floor. The pointer is a lens that gathers it; the window sloshes the water.
    Caustics,
    /// A low sun in a sky of graded light, its bands breathing and its corona turning, with soft
    /// clouds and ridges standing in haze below. The pointer is a lens, gathering ghosts of the
    /// sun along the line to it; the window slides the ridges apart.
    Solar,
    /// A spiral galaxy turning slowly over a field of stars, its arms lit with young stars, dust
    /// and glowing gas, with now and then a shooting star. The pointer bends the light round
    /// itself like a great mass; the window swings the galaxy against the stars behind.
    Galaxy,
    /// Jellyfish drifting up through deep water, their bells pulsing and their tentacles
    /// trailing, among sinking marine snow. Those near the pointer glow brighter and shy away
    /// from it; the window leaves them behind.
    Jellyfish,
    /// Two schools of small fish wheeling through sunlit shallows. Fish near the pointer dart away
    /// from it, and the school closes up behind; the window leaves the schools behind.
    Shoal,
    /// Points of light gathering into one figure after another (a cube, a rippling sheet, a knot,
    /// a double helix), turning slowly over a floor of faint lines and crossing from one side of
    /// the frame to the other with each new figure. The pointer scatters the
    /// points near it; the window swings the figure.
    Formation,
    /// A row of plants that grow, leaf and bloom over the cycle, then let their petals go and
    /// fade, swaying over a dusk of soft lights. They lean away from the pointer; the window leaves
    /// them behind.
    Garden,
}

impl Ambience {
    /// Every scene, in the order a picker should offer them.
    pub const ALL: [Ambience; 10] = [
        Ambience::Aurora,
        Ambience::Dunes,
        Ambience::Bloom,
        Ambience::Caustics,
        Ambience::Solar,
        Ambience::Galaxy,
        Ambience::Jellyfish,
        Ambience::Shoal,
        Ambience::Formation,
        Ambience::Garden,
    ];

    /// The scene's name.
    pub fn label(self) -> &'static str {
        match self {
            Ambience::Aurora => "Aurora",
            Ambience::Dunes => "Dunes",
            Ambience::Bloom => "Bloom",
            Ambience::Caustics => "Caustics",
            Ambience::Solar => "Solar",
            Ambience::Galaxy => "Galaxy",
            Ambience::Jellyfish => "Jellyfish",
            Ambience::Shoal => "Shoal",
            Ambience::Formation => "Formation",
            Ambience::Garden => "Garden",
        }
    }

    /// Whether the scene answers the pointer as well as the window.
    pub fn answers_pointer(self) -> bool {
        self != Ambience::Dunes
    }

    /// Whether the pointer leaves ripples in the scene and moves it, rather than only its stars.
    fn stirred(self) -> bool {
        self.answers_pointer() && self != Ambience::Aurora
    }

    /// How long one cycle of the scene takes.
    fn period(self) -> Duration {
        Duration::from_secs(match self {
            Ambience::Aurora => 40,
            Ambience::Dunes => 48,
            Ambience::Bloom => 36,
            Ambience::Caustics => 32,
            Ambience::Solar => 60,
            Ambience::Galaxy => 90,
            Ambience::Jellyfish => 48,
            Ambience::Shoal => 40,
            Ambience::Formation => 64,
            Ambience::Garden => 60,
        })
    }
}

/// A welcome-screen backdrop: an ambient scene drifting slowly over the window's background. Put
/// the screen's words on a card above it, never on the scene.
#[derive(IntoElement)]
pub struct AmbientBackdrop {
    id: ElementId,
    ambience: Ambience,
    palette: BackdropPalette,
    period: Option<Duration>,
    phase: Option<f32>,
}

impl AmbientBackdrop {
    /// A drifting backdrop showing `ambience`, on the scene's own cycle (half a minute or so).
    pub fn new(id: impl Into<ElementId>, ambience: Ambience) -> Self {
        Self {
            id: id.into(),
            ambience,
            palette: BackdropPalette::Theme,
            period: None,
            phase: None,
        }
    }

    /// Paint the scene in `palette` instead of the theme's colours.
    pub fn palette(mut self, palette: BackdropPalette) -> Self {
        self.palette = palette;
        self
    }

    /// Draw one still frame at `phase` (0 to 1) of the cycle instead of drifting.
    pub fn phase(mut self, phase: f32) -> Self {
        self.phase = Some(phase);
        self
    }

    /// Take `period` over one cycle instead of the scene's own.
    pub fn period(mut self, period: Duration) -> Self {
        self.period = Some(period);
        self
    }
}

/// The last frame rendered, kept so a redraw at the same moment reuses it, and the scene's
/// response to the pointer and the window.
#[derive(Default)]
struct Cache {
    key: Option<Key>,
    image: Option<Arc<RenderImage>>,
    sim: sim::Sim,
    /// Aurora's brighter stars.
    sky: stars::Sky,
    clock: Option<Clock>,
}

#[derive(Clone, Copy, PartialEq)]
struct Key {
    ambience: Ambience,
    palette: BackdropPalette,
    width: u32,
    height: u32,
    frame: u32,
    /// While the scene is answering the pointer or the window, its own clock, in frames.
    lively: Option<u32>,
    /// While Aurora's stars are changing, their own clock, in frames.
    starry: Option<u32>,
    direction: Direction,
    appearance: Appearance,
}

impl RenderOnce for AmbientBackdrop {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let cache = window.use_keyed_state(self.id.clone(), cx, |_, _| Cache::default());
        let period = self.period.unwrap_or(self.ambience.period());
        let frames = (period.as_secs_f32() * FPS).max(1.);
        // Without a GPU the scene is a still image: shading it on the CPU every frame would cost
        // far more than an ambient backdrop is worth.
        let still = self.phase.is_some() || !gpu::available();
        let drifting = !still && cx.theme().motion.enabled;
        // In an inactive window it holds still where it was.
        let running = drifting && window.is_window_active();
        let clock = cache.update(cx, |cache, _| {
            Clock::advance(&mut cache.clock, running, REST, period)
        });
        let phase = match self.phase {
            Some(phase) => phase,
            None if drifting => clock.phase(period),
            None => REST,
        };
        if running {
            window.request_animation_frame();
        }
        Scene {
            ambience: self.ambience,
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
    ambience: Ambience,
    palette: BackdropPalette,
    phase: f32,
    frames: f32,
    /// Whether the scene answers the pointer and the window: only while it drifts.
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
    window.paint_quad(fill(bounds, theme.colors.background));
    let palette = Palette::new(theme, scene.palette);

    // On the GPU, one pixel per logical pixel; gpui's sampler scales it up on dense displays.
    // The CPU's still frame has half a sample per logical pixel.
    let gpu = gpu::available();
    let density = if gpu { 1. } else { CPU_DENSITY }.min(window.scale_factor());
    let size = [f32::from(bounds.size.width), f32::from(bounds.size.height)];
    let width = (size[0] * density).round().max(1.) as u32;
    let height = (size[1] * density).round().max(1.) as u32;
    let frame = ((scene.phase.rem_euclid(1.)) * scene.frames) as u32;
    let phase = frame as f32 / scene.frames;

    let starry = scene.ambience == Ambience::Aurora;
    if scene.interactive {
        let origin = window.bounds().origin;
        let local = move |pointer: Point<Pixels>| {
            let local = pointer - bounds.origin;
            [
                f32::from(local.x).clamp(0., size[0]),
                f32::from(local.y).clamp(0., size[1]),
            ]
        };
        let over = window
            .is_window_hovered()
            .then(|| window.mouse_position())
            .filter(|pointer| bounds.contains(pointer))
            .map(local);
        let now = Instant::now();
        let input = sim::Input {
            now,
            origin: [f32::from(origin.x), f32::from(origin.y)],
            size,
            pointer: over.filter(|_| scene.ambience.stirred()),
        };
        scene.cache.update(cx, |cache, _| {
            cache.sim.step(input);
            if starry {
                // A line being drawn follows the pointer wherever it goes.
                let pointer = if cache.sky.drawing() {
                    Some(local(window.mouse_position()))
                } else {
                    over
                };
                let sway = cache.sim.response(size).sway;
                cache.sky.step(now, size, pointer, sway);
            }
        });
        if starry {
            listen(scene, local, bounds, hitbox, window, cx);
        }
    }

    let cached = scene.cache.read(cx);
    let key = Key {
        ambience: scene.ambience,
        palette: scene.palette,
        width,
        height,
        frame,
        lively: cached
            .sim
            .moving()
            .then(|| (cached.sim.clock() * LIVELY_FPS) as u32),
        starry: (starry && cached.sky.moving()).then(|| (cached.sky.clock() * LIVELY_FPS) as u32),
        direction,
        appearance,
    };
    let image = match (&cached.key, &cached.image) {
        (Some(cached_key), Some(image)) if *cached_key == key => image.clone(),
        _ => {
            let response = cached.sim.response(size);
            let sky = if starry {
                scene.cache.update(cx, |cache, _| cache.sky.marks(size))
            } else {
                Default::default()
            };
            let time = phase * std::f32::consts::TAU;
            let frame = Frame {
                width,
                height,
                time,
                palette,
                figures: figures::figures(scene.ambience, time, size, &response),
                response,
                sky,
            };
            let pixels = gpu
                .then(|| gpu::render(scene.ambience, &frame))
                .flatten()
                .unwrap_or_else(|| cpu::render(scene.ambience, &frame));
            let Some(buffer) = ImageBuffer::from_raw(width, height, pixels) else {
                return;
            };
            let image = Arc::new(RenderImage::new(vec![ImageFrame::new(buffer)]));
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

/// Let the person draw lines between Aurora's stars and send shooting stars across its sky, and
/// wipe the lines away with a double click; and show a pointing hand over a star.
fn listen(
    scene: &Scene,
    local: impl Fn(Point<Pixels>) -> [f32; 2] + Copy + 'static,
    bounds: Bounds<Pixels>,
    hitbox: &Hitbox,
    window: &mut Window,
    cx: &mut App,
) {
    let sky = &scene.cache.read(cx).sky;
    if sky.drawing() {
        window.set_window_cursor_style(CursorStyle::PointingHand);
    } else if hitbox.is_hovered(window) && sky.over_star(local(window.mouse_position())) {
        window.set_cursor_style(CursorStyle::PointingHand, hitbox);
    }

    let (cache, hitbox) = (scene.cache.clone(), hitbox.clone());
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
                cache.sky.wipe();
                true
            } else {
                cache.sky.press(pointer)
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
            let pointer = bounds
                .contains(&event.position)
                .then(|| local(event.position));
            cache.update(cx, |cache, _| cache.sky.release(pointer));
        }
    });
    // A release outside the window can go unreported; let go once the button is seen up.
    let cache = scene.cache.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
        if phase == DispatchPhase::Capture
            && event.pressed_button != Some(MouseButton::Left)
            && cache.read(cx).sky.drawing()
        {
            cache.update(cx, |cache, _| cache.sky.release(None));
        }
    });
}

/// One frame to shade: its size in pixels, the moment in the cycle (in radians), the colours,
/// the response to the pointer and the window, Aurora's stars, and Formation's and Garden's
/// pieces.
struct Frame {
    width: u32,
    height: u32,
    time: f32,
    palette: Palette,
    response: sim::Response,
    sky: stars::Marks,
    figures: figures::Figures,
}

impl Frame {
    /// A still frame of `ambience` at `phase` of the cycle, answering nothing.
    #[cfg(test)]
    fn still(ambience: Ambience, width: u32, height: u32, phase: f32, theme: &Theme) -> Self {
        let size = [width as f32, height as f32];
        let time = phase * std::f32::consts::TAU;
        let response = sim::Response::default();
        Self {
            width,
            height,
            time,
            palette: Palette::new(theme, BackdropPalette::Theme),
            figures: figures::figures(ambience, time, size, &response),
            response,
            sky: stars::Sky::default().marks(size),
        }
    }
}

/// The scenes' colours, from the theme and the backdrop's palette.
#[derive(Clone, Copy)]
struct Palette {
    /// The background where the scene is lightest: the horizon, the shallows.
    base: [f32; 3],
    /// Where it is deepest: the sky overhead, the floor of the water.
    deep: [f32; 3],
    /// The colours of the light, warm to cool: by default the agent gradient and the accent.
    hues: [[f32; 3]; 4],
    dark: bool,
}

fn rgb(color: Hsla) -> [f32; 3] {
    let Rgba { r, g, b, .. } = color.into();
    [r, g, b]
}

impl Palette {
    fn new(theme: &Theme, palette: BackdropPalette) -> Self {
        let dark = theme.appearance.is_dark();
        let background = rgb(theme.colors.background);
        let hues = palette.hues(theme).map(rgb);
        // The sky and the water lean towards the cool end, as they do towards the accent.
        let accent = hues[3];
        if dark {
            Self {
                base: mix3(background, accent, 0.1),
                deep: mix3(background, [0., 0., 0.], 0.55),
                hues,
                dark,
            }
        } else {
            // Washes of colour on a light background read as pastels.
            Self {
                base: background,
                deep: mix3(background, accent, 0.16),
                hues: hues.map(|hue| mix3(hue, [1., 1., 1.], 0.3)),
                dark,
            }
        }
    }
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// Films each scene answering a pointer that sweeps across it and a window that is moved,
    /// writing raw BGRA frames (with a ring for the pointer) to `$ATELIER_AMBIENT_FILM`. Aurora's
    /// pointer instead wakes its stars, draws a constellation and sends a shooting star.
    #[test]
    #[ignore = "needs a GPU; run with --release --ignored"]
    fn film() {
        let Ok(dir) = std::env::var("ATELIER_AMBIENT_FILM") else {
            return;
        };
        let only = std::env::var("ATELIER_AMBIENT_ONLY").ok();
        let (width, height, fps, seconds) = (960u32, 540u32, 30., 8.);
        let size = [width as f32, height as f32];
        let theme = Theme::new(Direction::Glass, Appearance::Dark);
        for ambience in Ambience::ALL {
            if only
                .as_ref()
                .is_some_and(|only| !only.eq_ignore_ascii_case(ambience.label()))
            {
                continue;
            }
            let mut sim = sim::Sim::default();
            let mut sky = stars::Sky::default();
            // The stars Aurora's pointer joins, in turn.
            let tour = [9, 10, 17, 16, 9].map(|index| sky.star(index, size));
            let start = Instant::now();
            for n in 0..(seconds * fps) as usize {
                let t = n as f32 / fps;
                // The pointer sweeps across from 0.5 s to 3.5 s, then leaves; the window is
                // dragged 160 pixels right and 60 up at 4.5 s.
                let sweeping = ambience.stirred() && (0.5..3.5).contains(&t);
                let mut pointer = sweeping.then(|| {
                    let u = (t - 0.5) / 3.;
                    [
                        width as f32 * (0.15 + 0.7 * u),
                        height as f32 * (0.5 + 0.25 * (u * std::f32::consts::TAU).sin()),
                    ]
                });
                let drag = ((t - 4.5) / 0.35).clamp(0., 1.);
                let mut origin = [160. * drag, -60. * drag];
                if ambience == Ambience::Aurora {
                    origin = [0., 0.];
                    // Over the sky to the first star, then from star to star, pressed, each
                    // leg 0.8 s; then a press on the open sky.
                    let leg = ((t - 1.5) / 0.8).floor();
                    pointer = Some(if t < 1.5 {
                        let u = t / 1.5;
                        [width as f32 * (0.1 + 0.2 * u), height as f32 * 0.2]
                    } else if (leg as usize) < tour.len() - 1 {
                        let (from, to) = (tour[leg as usize], tour[leg as usize + 1]);
                        let u = ((t - 1.5) / 0.8).fract().min(0.85) / 0.85;
                        let u = u * u * (3. - 2. * u);
                        [
                            from[0] + (to[0] - from[0]) * u,
                            from[1] + (to[1] - from[1]) * u,
                        ]
                    } else {
                        [width as f32 * 0.8, height as f32 * 0.15]
                    });
                    let previous = ((n as f32 - 1.) / fps - 1.5) / 0.8;
                    if t >= 1.5 && previous.floor() < leg && (leg as usize) < tour.len() {
                        let at = tour[leg as usize];
                        if leg > 0. {
                            sky.release(Some(at));
                        }
                        if (leg as usize) < tour.len() - 1 {
                            sky.press(at);
                        }
                    }
                    if (t - 5.6).abs() < 0.5 / fps {
                        sky.press(pointer.unwrap());
                    }
                }
                sim.step(sim::Input {
                    now: start + Duration::from_secs_f32(t),
                    origin,
                    size,
                    pointer: pointer.filter(|_| ambience.stirred()),
                });
                sky.step(
                    start + Duration::from_secs_f32(t),
                    size,
                    pointer,
                    sim.response(size).sway,
                );
                let phase = REST + t / ambience.period().as_secs_f32();
                let time = phase * std::f32::consts::TAU;
                let response = sim.response(size);
                let frame = Frame {
                    width,
                    height,
                    time,
                    palette: Palette::new(&theme, BackdropPalette::Theme),
                    figures: figures::figures(ambience, time, size, &response),
                    response,
                    sky: sky.marks(size),
                };
                let mut pixels = gpu::render(ambience, &frame).expect("a GPU");
                if let Some([x, y]) = pointer {
                    for py in (y as i32 - 12).max(0)..(y as i32 + 12).min(height as i32) {
                        for px in (x as i32 - 12).max(0)..(x as i32 + 12).min(width as i32) {
                            let d = ((px as f32 - x).powi(2) + (py as f32 - y).powi(2)).sqrt();
                            let ring = (1.5 - (d - 9.).abs()).clamp(0., 1.);
                            let i = (py as u32 * width + px as u32) as usize * 4;
                            for c in &mut pixels[i..i + 3] {
                                *c = (*c as f32 + (255. - *c as f32) * ring * 0.9) as u8;
                            }
                        }
                    }
                }
                std::fs::write(format!("{dir}/{ambience:?}-{n:04}.bgra"), pixels).unwrap();
            }
        }
    }
}
