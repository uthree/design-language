//! Ambient backdrops: slow, abstract scenes of light and colour to fill a welcome screen, in the
//! manner of a desktop wallpaper, beside the liquid metal ([`crate::metal`]).
//!
//! - [`Ambience::Aurora`]: curtains of light folding over a night sky.
//! - [`Ambience::Dunes`]: sheets of silk lying in layers, their crests rolling.
//! - [`Ambience::Bloom`]: great clouds of colour drifting and melting into one another.
//! - [`Ambience::Caustics`]: light through moving water, playing over a deep floor.
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
//! behind for a moment and it swings back, nearer layers further, so the window has depth. Bloom
//! and Caustics also answer the pointer — stirring the colour round it, or gathering the light
//! like a lens — and leave ripples as it travels; Aurora and Dunes leave it alone, since
//! ripples through a sky or a stack of silk would break the stillness they are there for.
//!
//! Put the screen's words on a card above the backdrop, never on it. When
//! [`crate::Motion::enabled`] is off it stands still and answers nothing. While its window is
//! not the active one it holds still where it was, and drifts on from there when the window
//! comes back.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, ElementId, Entity, Hsla, IntoElement, Pixels, RenderImage, RenderOnce, Rgba,
    Styled, Window, canvas, fill,
};
use image::{Frame as ImageFrame, ImageBuffer};

mod cpu;
mod gpu;
mod sim;

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
    /// swings them; the pointer leaves them alone.
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
}

impl Ambience {
    /// Every scene, in the order a picker should offer them.
    pub const ALL: [Ambience; 4] = [
        Ambience::Aurora,
        Ambience::Dunes,
        Ambience::Bloom,
        Ambience::Caustics,
    ];

    /// The scene's name.
    pub fn label(self) -> &'static str {
        match self {
            Ambience::Aurora => "Aurora",
            Ambience::Dunes => "Dunes",
            Ambience::Bloom => "Bloom",
            Ambience::Caustics => "Caustics",
        }
    }

    /// Whether the scene answers the pointer as well as the window.
    pub fn answers_pointer(self) -> bool {
        matches!(self, Ambience::Bloom | Ambience::Caustics)
    }

    /// How long one cycle of the scene takes.
    fn period(self) -> Duration {
        Duration::from_secs(match self {
            Ambience::Aurora => 40,
            Ambience::Dunes => 48,
            Ambience::Bloom => 36,
            Ambience::Caustics => 32,
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
            |_, _, _| {},
            move |bounds, _, window, cx| paint(&self, bounds, window, cx),
        )
        .size_full()
    }
}

fn paint(scene: &Scene, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
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

    if scene.interactive {
        let origin = window.bounds().origin;
        let pointer = window
            .is_window_hovered()
            .then(|| window.mouse_position())
            .filter(|pointer| scene.ambience.answers_pointer() && bounds.contains(pointer))
            .map(|pointer| {
                let local = pointer - bounds.origin;
                [f32::from(local.x), f32::from(local.y)]
            });
        let input = sim::Input {
            now: Instant::now(),
            origin: [f32::from(origin.x), f32::from(origin.y)],
            size,
            pointer,
        };
        scene.cache.update(cx, |cache, _| cache.sim.step(input));
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
        direction,
        appearance,
    };
    let image = match (&cached.key, &cached.image) {
        (Some(cached_key), Some(image)) if *cached_key == key => image.clone(),
        _ => {
            let frame = Frame {
                width,
                height,
                time: phase * std::f32::consts::TAU,
                palette,
                response: cached.sim.response(size),
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

/// One frame to shade: its size in pixels, the moment in the cycle (in radians), the colours,
/// and the response to the pointer and the window.
struct Frame {
    width: u32,
    height: u32,
    time: f32,
    palette: Palette,
    response: sim::Response,
}

impl Frame {
    /// A still frame at `phase` of the cycle, answering nothing.
    #[cfg(test)]
    fn still(width: u32, height: u32, phase: f32, theme: &Theme) -> Self {
        Self {
            width,
            height,
            time: phase * std::f32::consts::TAU,
            palette: Palette::new(theme, BackdropPalette::Theme),
            response: Default::default(),
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
    /// writing raw BGRA frames (with a ring for the pointer) to `$ATELIER_AMBIENT_FILM`.
    #[test]
    #[ignore = "needs a GPU; run with --release --ignored"]
    fn film() {
        let Ok(dir) = std::env::var("ATELIER_AMBIENT_FILM") else {
            return;
        };
        let (width, height, fps, seconds) = (960u32, 540u32, 30., 8.);
        let size = [width as f32, height as f32];
        let theme = Theme::new(Direction::Glass, Appearance::Dark);
        for ambience in Ambience::ALL {
            let mut sim = sim::Sim::default();
            let start = Instant::now();
            for n in 0..(seconds * fps) as usize {
                let t = n as f32 / fps;
                // The pointer sweeps across from 0.5 s to 3.5 s, then leaves; the window is
                // dragged 160 pixels right and 60 up at 4.5 s.
                let sweeping = ambience.answers_pointer() && (0.5..3.5).contains(&t);
                let pointer = sweeping.then(|| {
                    let u = (t - 0.5) / 3.;
                    [
                        width as f32 * (0.15 + 0.7 * u),
                        height as f32 * (0.5 + 0.25 * (u * std::f32::consts::TAU).sin()),
                    ]
                });
                let drag = ((t - 4.5) / 0.35).clamp(0., 1.);
                let origin = [160. * drag, -60. * drag];
                sim.step(sim::Input {
                    now: start + Duration::from_secs_f32(t),
                    origin,
                    size,
                    pointer,
                });
                let phase = REST + t / ambience.period().as_secs_f32();
                let frame = Frame {
                    width,
                    height,
                    time: phase * std::f32::consts::TAU,
                    palette: Palette::new(&theme, BackdropPalette::Theme),
                    response: sim.response(size),
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
