//! The backdrops on the CPU, for when no GPU adapter is available: the same scenes as the
//! shaders, line for line, without the caustics' coloured fringes, shaded across every core for
//! the one still frame the backdrop shows there.

use std::f32::consts::TAU;
use std::ops::{Add, Mul, Sub};

use super::sim::WAVES;
use super::{Ambience, Frame};

/// `frame` shaded as `ambience`, BGRA.
pub(super) fn render(ambience: Ambience, frame: &Frame) -> Vec<u8> {
    let (width, height) = (frame.width as usize, frame.height as usize);
    let shader = Shader::new(frame);
    let scene = match ambience {
        Ambience::Aurora => Shader::aurora,
        Ambience::Dunes => Shader::dunes,
        Ambience::Bloom => Shader::bloom,
        Ambience::Caustics => Shader::caustics,
    };
    let mut pixels = vec![0u8; width * height * 4];
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(height.max(1));
    let band = height.div_ceil(threads);
    std::thread::scope(|scope| {
        for (chunk, out) in pixels.chunks_mut(band * width * 4).enumerate() {
            let shader = &shader;
            scope.spawn(move || {
                let first = chunk * band;
                for (index, pixel) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    let position = v2(
                        (index % width) as f32 + 0.5,
                        (first + index / width) as f32 + 0.5,
                    );
                    let p = (position - shader.size * 0.5) * (1. / shader.unit);
                    let grain = (hash(position) - 0.5) / 255. * 1.4;
                    let color = scene(shader, p);
                    // BGRA, opaque.
                    pixel[0] = to_byte(color.z + grain);
                    pixel[1] = to_byte(color.y + grain);
                    pixel[2] = to_byte(color.x + grain);
                    pixel[3] = 255;
                }
            });
        }
    });
    pixels
}

/// The shaders' `Params`, as the scenes read them.
struct Shader {
    size: V2,
    unit: f32,
    time: f32,
    dark: f32,
    pointer: V2,
    presence: f32,
    velocity: V2,
    sway: V2,
    base: V3,
    deep: V3,
    hues: [V3; 4],
    waves: Vec<[f32; 4]>,
}

/// A ripple's wavelength and the width of its band, as in `common.wgsl`.
const WAVELENGTH: f32 = 0.045;
const BAND: f32 = 0.07;

impl Shader {
    fn new(frame: &Frame) -> Self {
        let r = &frame.response;
        let size = v2(frame.width as f32, frame.height as f32);
        let color = |[r, g, b]: [f32; 3]| v3(r, g, b);
        Self {
            size,
            unit: size.x.min(size.y),
            time: frame.time,
            dark: if frame.palette.dark { 1. } else { 0. },
            pointer: v2(r.pointer[0], r.pointer[1]),
            presence: r.presence,
            velocity: v2(r.velocity[0], r.velocity[1]),
            sway: v2(r.sway[0], r.sway[1]),
            base: color(frame.palette.base),
            deep: color(frame.palette.deep),
            hues: frame.palette.hues.map(color),
            waves: r.waves[..r.wave_count.min(WAVES)].to_vec(),
        }
    }

    fn orbit(&self, turns: f32, start: f32) -> V2 {
        let angle = self.time * turns + start;
        v2(angle.cos(), angle.sin())
    }

    fn touch(&self, p: V2, reach: f32) -> f32 {
        let d = p - self.pointer;
        self.presence * (-d.dot(d) / (reach * reach)).exp()
    }

    /// The ripples' height at `p`, and which way they run there.
    fn ripples(&self, p: V2) -> (f32, V2) {
        let k = TAU / WAVELENGTH;
        let (mut height, mut run) = (0., v2(0., 0.));
        for &[x, y, spread, strength] in &self.waves {
            let d = p - v2(x, y);
            let distance = d.length().max(1e-4);
            let x = distance - spread;
            let u = x / BAND;
            let h = strength * (-u * u).exp() * BAND / (BAND + spread) * ease(0., BAND, distance);
            height += h * (k * x).sin();
            run = run + d * (h / distance);
        }
        (height, run)
    }

    fn light(&self, background: V3, color: V3, amount: f32) -> V3 {
        let one = v3(1., 1., 1.);
        let glow = one - (one - background) * (one - color * amount.max(0.));
        let wash = background.mix(color, amount.clamp(0., 1.));
        wash.mix(glow, self.dark)
    }

    // aurora.wgsl

    fn curtain(&self, p: V2, index: f32) -> (V3, f32) {
        let t = self.time;
        let depth = 0.55 + 0.25 * index;
        let q = p + self.sway * depth;
        let x = q.x / depth;
        let hem = 0.02
            + 0.07 * index
            + 0.06 * (x * 1.6 + t + index * 2.1).sin()
            + 0.035 * (x * 3.7 - 2. * t + index * 4.3).sin()
            + 0.015 * (x * 7.9 + 3. * t + index).sin();
        let above = hem - q.y;
        let height = 0.22 + 0.06 * index;
        let body = (-above.max(0.) / height).exp() * ease(-0.03, 0.006, above);
        let lean = 0.6 * (x * 1.2 - t + index).sin();
        let drift = 0.4 * (t + index).sin();
        let ray_at = v2((x + above * lean) * 16. + drift, index * 7.3 + above * 1.5);
        let rays = 0.3 + 1.1 * ease(0.3, 0.85, fbm(ray_at));
        let fold = 0.35 + 0.65 * ease(-0.6, 1., (x * 2.3 - t * 2. + index * 1.7).sin());
        let strength = body * rays * fold;
        let h = (above / height * 0.7).clamp(0., 1.);
        let i = index as usize;
        let color = self.hues[(3 + i * 2) % 4].mix(self.hues[(i + 1) % 4], h);
        (color, strength * (0.85 - index * 0.15))
    }

    fn sky(&self, p: V2) -> V3 {
        let horizon = ease(-0.7, AURORA_LAKE, p.y + self.sway.y * 0.2);
        let mut color = self.deep.mix(self.base, horizon);
        let at = (p + self.sway * 0.15) * 90.;
        let cell = v2(at.x.floor(), at.y.floor());
        let star = hash(cell);
        let centre = at - cell - v2(0.5, 0.5);
        let twinkle = 0.6 + 0.4 * (self.time * 3. + star * 40.).sin();
        let lit = if star >= 0.985 { 1. } else { 0. };
        let stars = lit * ease(0.18, 0., centre.length()) * twinkle * (1. - horizon);
        color = color + v3(1., 1., 1.) * (stars * self.dark * 0.7);
        for index in 0..3 {
            let (hue, amount) = self.curtain(p, index as f32);
            color = self.light(color, hue, amount);
        }
        color
    }

    fn aurora(&self, p: V2) -> V3 {
        if p.y < AURORA_LAKE {
            return self.sky(p);
        }
        let depth = p.y - AURORA_LAKE;
        let wobble = 0.004 * (depth * 140. / (0.2 + depth) + self.time * 4.).sin();
        let mirrored = self.sky(v2(p.x + wobble, AURORA_LAKE - depth * 1.15));
        let water = self.deep.mix(self.base, 0.5);
        let color = water.mix(mirrored, lerp(0.7, 0.6, self.dark) * (-depth * 1.8).exp());
        self.light(color, self.hues[3], (-depth / 0.004).exp() * 0.25)
    }

    // dunes.wgsl

    fn crest(&self, x: f32, index: f32) -> f32 {
        let t = self.time;
        let level = -0.26 + 0.16 * index;
        level
            + 0.09 * (x * (1.1 + 0.25 * index) + t + index * 1.9).sin()
            + 0.045 * (x * (2.6 - 0.2 * index) - 2. * t + index * 3.7).sin()
            + 0.015 * (x * 6.1 + 3. * t + index * 5.3).sin()
    }

    fn sheet(&self, index: f32) -> V3 {
        let i = index as usize;
        let hue = self.hues[i % 4].mix(self.hues[(i + 1) % 4], 0.35);
        let far = 1. - index / (DUNE_LAYERS - 1) as f32;
        let tone = self.deep.mix(self.base, 1. - self.dark);
        hue.mix(tone, far * 0.55 + 0.1)
    }

    fn dunes(&self, p: V2) -> V3 {
        let mut color = self.deep.mix(self.base, ease(-0.8, 0.4, p.y));
        let sun = p - v2(0.55, -0.42) + self.sway * 0.1;
        color = self.light(color, self.hues[0], (-sun.dot(sun) / 0.09).exp() * 0.45);
        let light_from = v2(0.45, -1.).normalize();
        for n in 0..DUNE_LAYERS {
            let index = n as f32;
            let depth = 0.3 + 0.18 * index;
            let q = p + self.sway * depth;
            let x = q.x / (0.8 + 0.1 * index);
            let e = 0.004;
            let y = self.crest(x, index);
            let slope = (self.crest(x + e, index) - self.crest(x - e, index)) / (2. * e);
            let below = q.y - y;
            let shadow = (-(-below).max(0.) / 0.05).exp() * ease(0.003, -0.003, below);
            color = color * (1. - shadow * lerp(0.18, 0.45, self.dark));
            let pixel = 1.5 / self.unit;
            let cover = ease(-pixel, pixel, below / (1. + slope * slope).sqrt());
            let fall = 1. - (-below.max(0.) / 0.12).exp();
            let mut silk = self.sheet(index) * (1.08 - fall * 0.35);
            let facing = v2(slope, -1.).normalize();
            let sheen = facing.dot(light_from).max(0.).powi(24) * (-below.max(0.) / 0.2).exp();
            silk = silk + v3(1., 1., 1.) * (sheen * 0.22 * (1. - fall));
            silk = silk + v3(1., 1., 1.) * ((-below.max(0.) / 0.004).exp() * 0.12);
            color = color.mix(silk, cover);
        }
        color
    }

    // bloom.wgsl

    fn bloom(&self, p: V2) -> V3 {
        let near = self.touch(p, 0.3);
        let d = p - self.pointer;
        let angle = near * (0.9 + self.velocity.length().clamp(0., 2.) * 0.5);
        let (sin, cos) = angle.sin_cos();
        let mut q = self.pointer + v2(cos * d.x - sin * d.y, sin * d.x + cos * d.y);
        q = q - self.velocity * (near * 0.06);
        let (height, run) = self.ripples(q);
        q = q - run * 0.025;
        let warp = v2(
            fbm(q * 1.3 + self.orbit(1., 0.) * 0.5),
            fbm(q * 1.3 + v2(5.2, 1.3) + self.orbit(1., 2.) * 0.5),
        );
        q = q + (warp - v2(0.5, 0.5)) * 0.6;

        let (mut sum, mut weight) = (v3(0., 0., 0.), 0.);
        for (i, cloud) in BLOOM_CLOUDS.iter().enumerate() {
            let lag = 0.6 + 0.25 * (i % 3) as f32;
            let swing = self.orbit((1 + i % 2) as f32, i as f32 * 1.7);
            let centre =
                v2(cloud[0], cloud[1]) + v2(swing.x, swing.y * 0.7) * cloud[2] - self.sway * lag;
            let r = q - centre;
            let w = (-r.dot(r) / (cloud[3] * cloud[3])).exp();
            sum = sum + self.hues[i % 4] * w;
            weight += w;
        }
        let hue = sum * (1. / weight.max(1e-4));
        let amount = ease(0., 1.3, weight);
        let mut color = self.deep.mix(self.base, ease(-0.9, 0.9, p.y - p.x * 0.3));
        color = self.light(color, hue, amount * lerp(0.8, 0.85, self.dark));
        let fold = fbm(q * 2.2 - self.orbit(2., 0.5) * 0.3);
        let sheen = ease(0.07, 0., (fold - 0.5).abs()) * 0.05 + near * 0.06;
        let white = v3(1., 1., 1.);
        color = self.light(color, white, sheen * (0.4 + 0.6 * self.dark));
        self.light(color, white, height.max(0.) * 0.15)
    }

    // caustics.wgsl

    fn net(&self, p: V2) -> f32 {
        let t = self.time;
        let mut i = p;
        let mut c = 1.;
        let intensity = 0.005;
        for (n, speed) in CAUSTIC_SPEEDS.iter().enumerate() {
            let s = t * speed + n as f32 * 1.3;
            i = p + v2(
                (s - i.x).cos() + (s + i.y).sin(),
                (s - i.y).sin() + (s + i.x).cos(),
            );
            c += 1.
                / v2(
                    p.x / ((i.x + s).sin() / intensity),
                    p.y / ((i.y + s).cos() / intensity),
                )
                .length();
        }
        let c = 1.17 - (c / 5_f32).powf(1.4);
        c.abs().powi(8)
    }

    fn caustics(&self, p: V2) -> V3 {
        let floor = p + self.sway * 0.4;
        let mut color = self.base.mix(self.deep, ease(-0.7, 0.8, floor.y));
        let tint = fbm(floor * 0.9 + self.orbit(1., 0.) * 0.15);
        let hue = self.hues[3]
            .mix(self.hues[2], ease(0.3, 0.6, tint))
            .mix(self.hues[1], ease(0.6, 0.85, tint));
        color = self.light(color, hue, lerp(0.6, 0.3, self.dark));

        let near = self.touch(floor, 0.25);
        let mut q = floor + self.sway * 0.8;
        q = self.pointer + (q - self.pointer) * (1. - near * 0.3);
        let (height, run) = self.ripples(q);
        q = q + run * 0.06;
        let lines = self.net(q * 6. - v2(250., 250.));
        let glow = lines * (0.8 + near * 0.7 + height.max(0.) * 0.5);
        let shine = hue.mix(v3(1., 1., 1.), lerp(0.85, 0.55, self.dark));
        color = self.light(color, shine, glow * lerp(0.8, 1., self.dark));

        let shaft = fbm(v2(p.x * 3.5 + p.y * 1.2 + self.time.sin() * 0.3, 0.));
        let shafts = ease(0.45, 0.8, shaft) * ease(0.6, -0.5, p.y) * 0.16;
        self.light(color, shine, shafts)
    }
}

/// As `LAKE` in `aurora.wgsl`.
const AURORA_LAKE: f32 = 0.24;

const DUNE_LAYERS: usize = 5;

/// As `CLOUDS` in `bloom.wgsl`.
const BLOOM_CLOUDS: [[f32; 4]; 5] = [
    [-0.55, -0.25, 0.22, 0.55],
    [0.5, -0.3, 0.18, 0.5],
    [0.1, 0.35, 0.25, 0.6],
    [-0.2, 0.05, 0.3, 0.42],
    [0.75, 0.3, 0.2, 0.45],
];

/// As `SPEEDS` in `caustics.wgsl`.
const CAUSTIC_SPEEDS: [f32; 5] = [-2., 1., -1., 2., 1.];

fn ease(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn fract(x: f32) -> f32 {
    x - x.floor()
}

fn hash(p: V2) -> f32 {
    let q = v2(fract(p.x * 123.34), fract(p.y * 456.21));
    let shift = q.dot(q + v2(45.32, 45.32));
    fract((q.x + shift) * (q.y + shift))
}

fn noise(p: V2) -> f32 {
    let i = v2(p.x.floor(), p.y.floor());
    let f = p - i;
    let u = v2(f.x * f.x * (3. - 2. * f.x), f.y * f.y * (3. - 2. * f.y));
    let a = hash(i);
    let b = hash(i + v2(1., 0.));
    let c = hash(i + v2(0., 1.));
    let d = hash(i + v2(1., 1.));
    lerp(lerp(a, b, u.x), lerp(c, d, u.x), u.y)
}

fn fbm(p: V2) -> f32 {
    let (mut sum, mut amplitude, mut q) = (0., 0.5, p);
    for _ in 0..4 {
        sum += amplitude * noise(q);
        q = q * 2.03 + v2(1.7, 9.2);
        amplitude *= 0.5;
    }
    sum / 0.9375
}

fn to_byte(value: f32) -> u8 {
    (value.clamp(0., 1.) * 255. + 0.5) as u8
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct V2 {
    x: f32,
    y: f32,
}

const fn v2(x: f32, y: f32) -> V2 {
    V2 { x, y }
}

impl V2 {
    fn dot(self, other: V2) -> f32 {
        self.x * other.x + self.y * other.y
    }

    fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    fn normalize(self) -> V2 {
        self * (1. / self.length().max(1e-9))
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, other: V2) -> V2 {
        v2(self.x + other.x, self.y + other.y)
    }
}

impl Sub for V2 {
    type Output = V2;
    fn sub(self, other: V2) -> V2 {
        v2(self.x - other.x, self.y - other.y)
    }
}

impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, factor: f32) -> V2 {
        v2(self.x * factor, self.y * factor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct V3 {
    x: f32,
    y: f32,
    z: f32,
}

const fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

impl V3 {
    fn mix(self, other: V3, t: f32) -> V3 {
        self + (other - self) * t
    }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, other: V3) -> V3 {
        v3(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl Sub for V3 {
    type Output = V3;
    fn sub(self, other: V3) -> V3 {
        v3(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl Mul<f32> for V3 {
    type Output = V3;
    fn mul(self, factor: f32) -> V3 {
        v3(self.x * factor, self.y * factor, self.z * factor)
    }
}

impl Mul for V3 {
    type Output = V3;
    fn mul(self, other: V3) -> V3 {
        v3(self.x * other.x, self.y * other.y, self.z * other.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Appearance, Direction, Theme};

    fn frame(phase: f32) -> Frame {
        let theme = Theme::new(Direction::Glass, Appearance::Dark);
        Frame::still(64, 40, phase, &theme)
    }

    #[test]
    fn every_scene_renders_and_loops() {
        for ambience in Ambience::ALL {
            let first = render(ambience, &frame(0.));
            let last = render(ambience, &frame(1.));
            assert_eq!(first.len(), 64 * 40 * 4);
            let differing = first
                .iter()
                .zip(&last)
                .filter(|(a, b)| a.abs_diff(**b) > 2)
                .count();
            assert!(
                differing < first.len() / 100,
                "{ambience:?} loops seamlessly"
            );
            let middle = render(ambience, &frame(0.4));
            let moved = first
                .iter()
                .zip(&middle)
                .filter(|(a, b)| a.abs_diff(**b) > 2)
                .count();
            assert!(moved > first.len() / 20, "{ambience:?} moves");
        }
    }

    #[test]
    fn only_bloom_and_caustics_answer_the_pointer() {
        for ambience in Ambience::ALL {
            let still = render(ambience, &frame(0.2));
            let mut touched = frame(0.2);
            touched.response.presence = 1.;
            touched.response.pointer = [0.1, 0.05];
            touched.response.velocity = [0.6, 0.];
            touched.response.waves[0] = [0.1, 0.05, 0.08, 1.];
            touched.response.wave_count = 1;
            let touched = render(ambience, &touched);
            assert_eq!(
                still != touched,
                ambience.answers_pointer(),
                "{ambience:?} answers the pointer only if it says so"
            );
        }
    }

    #[test]
    #[ignore = "timing; run with --release --ignored --nocapture"]
    fn frame_time() {
        let theme = Theme::new(Direction::Glass, Appearance::Dark);
        // A 1300 x 750 window at the CPU's density.
        let (width, height) = (650, 375);
        for ambience in Ambience::ALL {
            let start = std::time::Instant::now();
            for n in 0..10 {
                render(
                    ambience,
                    &Frame::still(width, height, n as f32 / 10., &theme),
                );
            }
            println!("{ambience:?}: {:?} per frame", start.elapsed() / 10);
        }
    }
}
