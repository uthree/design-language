//! The backdrops on the CPU, for when no GPU adapter is available: the same scenes as the
//! shaders, line for line, without the caustics' coloured fringes, shaded across every core for
//! the one still frame the backdrop shows there.

use std::f32::consts::{PI, TAU};
use std::ops::{Add, Mul, Sub};

use super::sim::WAVES;
use super::stars::STARS;
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
        Ambience::Solar => Shader::solar,
        Ambience::Galaxy => Shader::galaxy,
        Ambience::Jellyfish => Shader::jellyfish,
        Ambience::Shoal => Shader::shoal,
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
    stars: [[f32; 4]; STARS],
    lines: Vec<([f32; 4], f32)>,
    meteors: Vec<[f32; 4]>,
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
            stars: frame.sky.stars,
            lines: frame.sky.lines[..frame.sky.line_count]
                .iter()
                .copied()
                .zip(frame.sky.strengths)
                .collect(),
            meteors: frame.sky.meteors[..frame.sky.meteor_count].to_vec(),
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
        color = self.constellations(p + self.sway * 0.15, color);
        for index in 0..3 {
            let (hue, amount) = self.curtain(p, index as f32);
            color = self.light(color, hue, amount);
        }
        color
    }

    fn constellations(&self, q: V2, color: V3) -> V3 {
        let px = 1. / self.unit;
        let ink = (self.deep.mix(self.hues[3], 0.45) * 0.6)
            .mix(v3(1., 1., 1.).mix(self.hues[0], 0.2), self.dark);
        let line_ink = ink.mix(self.hues[1], 0.35);
        let mut c = color;
        for &(line, strength) in &self.lines {
            let d = segment(q, v2(line[0], line[1]), v2(line[2], line[3]));
            let core = ease(1.3 * px, 0., d);
            let halo = (-d / (5. * px)).exp() * 0.22;
            c = self.light(c, line_ink, strength * (core * 0.6 + halo));
        }
        for (i, star) in self.stars.iter().enumerate() {
            let d = q - v2(star[0], star[1]);
            let r = d.length();
            if r > 0.09 {
                continue;
            }
            let awake = star[2];
            let twinkle = 0.75 + 0.25 * (self.time * 5. + i as f32 * 2.4).sin();
            let radius = (1. + 1.4 * star[3] + awake * 0.9) * px;
            let core = ease(radius, 0., r) * (0.45 + 0.35 * star[3] + 0.4 * awake) * twinkle;
            let halo = (-r * r / (radius * radius * 10.)).exp() * (0.12 + 0.5 * awake);
            let spikes = ((-d.x.abs() / (0.6 * px)).exp() + (-d.y.abs() / (0.6 * px)).exp())
                * (-r / (0.02 * (0.3 + awake))).exp()
                * awake
                * 0.35;
            let hue = ink.mix(self.hues[(i + 1) % 4], 0.25 * awake);
            c = self.light(c, hue, core + halo + spikes);
        }
        for meteor in &self.meteors {
            let heading = v2(meteor[2].cos(), meteor[2].sin());
            let age = meteor[3];
            let head = v2(meteor[0], meteor[1]);
            let tail = head - heading * (0.06 + 0.14 * (1. - age));
            let d = segment(q, head, tail);
            let along = ((q - tail).dot(heading) / (head - tail).length().max(1e-4)).clamp(0., 1.);
            let fade = ((age * 1.2).min(1.) * PI).sin();
            let streak =
                ease(1.6 * px, 0., d) * along * along + (-d / (4. * px)).exp() * along * 0.3;
            c = self.light(c, ink, streak * fade);
        }
        c
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

impl Shader {
    // solar.wgsl

    fn ridge(&self, x: f32, index: f32) -> f32 {
        let level = 0.17 + 0.1 * index;
        level
            + 0.05 * (x * (1.3 + 0.4 * index) + index * 2.3).sin()
            + 0.025 * (x * (3.1 + 0.7 * index) + index * 5.1).sin()
            + 0.024 * (fbm(v2(x * 4. + index * 7., index)) - 0.5)
    }

    fn solar(&self, p: V2) -> V3 {
        let t = self.time;
        let px = 1. / self.unit;
        let sun = v2(0.46, 0.08) + v2(0., 0.012 * t.sin()) - self.sway * 0.1;
        let to_sun = p - sun;
        let stir = fbm(p * 1.4 + self.orbit(1., 0.) * 0.3) - 0.5;
        let r = v2(to_sun.x, to_sun.y * 1.3).length() + stir * 0.07;
        let breath = 1. + 0.05 * (t * 2. + 1.).sin();

        let glow = lerp(1., 0.7, self.dark);
        let mut color = self.deep.mix(self.base, ease(-0.7, 0.45, p.y));
        let rings = [
            (1.7, 0.75, 0.5 * glow),
            (1.1, 0.4, 0.55 * glow),
            (0.7, 0.18, 0.65 * glow),
        ];
        for (i, (outer, inner, amount)) in rings.into_iter().enumerate() {
            let band = ease(outer * breath, inner * breath, r);
            color = self.light(color, self.hues[3 - i], band * amount);
        }
        color = self.light(color, self.hues[0], ease(0.38 * breath, 0.05, r) * 0.8);

        let direction = to_sun * (1. / to_sun.length().max(1e-4));
        let spun = rotate(direction, self.orbit(1., 0.));
        let rays = ease(0.45, 0.85, fbm(spun * 3.5 + v2(11., 11.)))
            * (-r / 0.35).exp()
            * ease(0.04, 0.1, r);
        let white = v3(1., 1., 1.);
        let hot = self.hues[0].mix(white, lerp(0.35, 0.7, self.dark));
        color = self.light(color, hot, rays * 0.35);

        for i in 0..2 {
            let k = i as f32;
            let depth = 0.25 + 0.2 * k;
            let q = p + self.sway * depth;
            let swing = self.orbit(1., k * 2.);
            let at = v2(q.x * (1.6 - 0.4 * k), q.y * 9.)
                + v2(k * 13., 0.)
                + v2(swing.x * 0.8, swing.y * 0.15);
            let height = -0.12 - 0.15 * k;
            let band = ease(0.09, 0., (q.y - height).abs() - 0.03);
            let cloud = ease(0.5, 0.8, fbm(at)) * band;
            let lit = self.hues[1].mix(self.hues[0], (-r / 0.6).exp());
            color = self.light(color, lit.mix(white, 0.25), cloud * 0.4);
        }

        let disc = ease(0.07, 0.064, to_sun.length());
        color = self.light(color, hot, disc * 0.95 + (-r / 0.05).exp() * 0.5);
        let streak = (-to_sun.y.abs() / 0.004).exp() * (-to_sun.x.abs() / 0.5).exp() * 0.18;
        color = self.light(color, hot, streak);

        let tone = (self.base.mix(self.hues[3], 0.45) * 0.9)
            .mix(self.deep.mix(self.hues[3], 0.15) * 0.65, self.dark);
        for i in 0..SOLAR_RIDGES {
            let k = i as f32;
            let depth = 0.3 + 0.3 * k;
            let q = p + self.sway * depth;
            let top = self.ridge(q.x, k);
            let below = q.y - top;
            let cover = ease(-px, px, below);
            let haze = 1. - (k + 1.) / (SOLAR_RIDGES + 1) as f32;
            let mist = ease(0., 0.18, below) * 0.15;
            let mut rock = tone.mix(color, haze * 0.75 + mist);
            let rim = (-below.max(0.) / 0.004).exp()
                * (-(q.x - sun.x).abs() / 0.5).exp()
                * (0.35 - 0.08 * k);
            rock = self.light(rock, hot, rim);
            color = color.mix(rock, cover);
        }

        let axis = self.pointer - sun;
        for (i, &[along, size]) in SOLAR_GHOSTS.iter().enumerate() {
            let d = (p - (sun + axis * along)).length();
            let ring = ease(size, size * 0.75, d) * (0.4 + 0.6 * ease(size * 0.4, size, d));
            color = self.light(color, self.hues[i], ring * 0.14 * self.presence);
        }
        self.light(color, hot, self.touch(p, 0.04) * 0.2)
    }

    // galaxy.wgsl

    fn starfield(&self, q: V2, cells: f32, share: f32) -> f32 {
        let at = q * cells;
        let cell = v2(at.x.floor(), at.y.floor());
        let seed = hash(cell);
        let spot = v2(fract(at.x), fract(at.y))
            - v2(0.5, 0.5)
            - (v2(hash(cell + v2(2.3, 2.3)), hash(cell + v2(8.9, 8.9))) - v2(0.5, 0.5)) * 0.5;
        let twinkle = 0.65 + 0.35 * (self.time * 4. + seed * 50.).sin();
        step(1. - share, seed)
            * ease(0.2, 0., spot.length())
            * twinkle
            * (0.4 + 0.6 * fract(seed * 13.))
    }

    fn galaxy(&self, p: V2) -> V3 {
        let t = self.time;
        let px = 1. / self.unit;
        let from = p - self.pointer;
        let pull = self.presence * 0.003 / (from.length() + 0.04);
        let mut q = p - from * (pull / from.length().max(1e-4));
        let (height, run) = self.ripples(q);
        q = q - run * 0.02;

        let centre = v2(0.4, -0.08);
        let mut color = self
            .deep
            .mix(self.base, ease(1.2, 0., (p - centre).length()) * 0.6);
        let far = q + self.sway * 0.1;
        let clouds = fbm(far * 1.6 + self.orbit(1., 0.) * 0.2);
        color = self.light(color, self.hues[2], ease(0.5, 0.85, clouds) * 0.18);
        color = self.light(
            color,
            self.hues[1],
            ease(0.62, 0.9, fbm(far * 2.3 + v2(4., 4.))) * 0.12,
        );
        let white = v3(1., 1., 1.);
        let ink = (self.deep.mix(self.hues[3], 0.4) * 0.55).mix(white, self.dark);
        color = self.light(
            color,
            ink,
            self.starfield(far, 70., 0.04) * lerp(0.35, 0.8, self.dark),
        );

        let d = rotate(
            q - centre + self.sway * 0.4,
            v2((-0.35_f32).cos(), (-0.35_f32).sin()),
        );
        let disc = v2(d.x, d.y / GALAXY_TILT);
        let r = disc.length();
        let spun = rotate(disc, self.orbit(-1., 0.));
        let angle = spun.y.atan2(spun.x);
        let arm = (0.5 + 0.5 * (2. * angle - r.max(1e-3).ln() * GALAXY_WIND).cos()).powi(3);
        let envelope = (-r / 0.32).exp() * ease(1.15, 0.5, r);
        let outer = self.hues[2].mix(self.hues[3], ease(0.1, 0.5, r));
        color = self.light(color, outer, arm * envelope * 0.75 + envelope * 0.12);
        let lane = (0.5 + 0.5 * (2. * angle - r.max(1e-3).ln() * GALAXY_WIND + 0.7).cos()).powi(6);
        let dust = lane * ease(0.4, 0.7, fbm(spun * 9.)) * envelope * ease(0.04, 0.12, r);
        color = color.mix(color * lerp(0.75, 0.45, self.dark), dust * 0.8);
        let knots = ease(0.62, 0.8, fbm(spun * 14. + v2(3., 3.))) * arm * envelope;
        color = self.light(color, self.hues[1], knots * 0.6);
        color = self.light(
            color,
            ink.mix(outer, 0.3),
            self.starfield(spun, 110., 0.12) * (arm * 0.8 + 0.2) * envelope * 2.2,
        );

        let core = (-r * r / 0.012).exp() * 0.65 + (-r / 0.025).exp() * 0.6;
        let hot = self.hues[0].mix(white, lerp(0.3, 0.6, self.dark));
        color = self.light(color, hot, core);

        for m in 0..GALAXY_METEORS {
            let k = m as f32;
            let age = fract(t / TAU - k / GALAXY_METEORS as f32 - 0.07) / 0.03;
            if age > 1. {
                continue;
            }
            let start = v2(hash(v2(k, 1.)) * 1.4 - 0.7, hash(v2(k, 2.)) * 0.5 - 0.45);
            let heading = if start.x > 0. { 2.7 } else { 0.45 } + 0.2 * hash(v2(k, 3.));
            let towards = v2(heading.cos(), heading.sin());
            let head = start + towards * (age * 0.7);
            let tail = head - towards * 0.16;
            let along = ((p - tail).dot(towards) / 0.16).clamp(0., 1.);
            let line = segment(p, head, tail);
            let streak =
                (ease(1.5 * px, 0., line) + (-line / (4. * px)).exp() * 0.3) * along * along;
            color = self.light(color, ink, streak * (age * PI).sin());
        }
        self.light(color, white, height.max(0.) * 0.1)
    }

    // jellyfish.wgsl

    fn jelly_at(&self, jelly: [f32; 4]) -> (V2, f32) {
        let t = self.time;
        let width = self.size.x / self.unit;
        let height = self.size.y / self.unit;
        let size = jelly[1];
        let span = height + size * 5.;
        let rise = fract(jelly[3] + jelly[2] * t / TAU);
        let y = 0.5 * height + size * 0.9 - rise * span;
        let x = jelly[0] * 0.5 * width * 0.85 + 0.04 * (t * 2. + jelly[3] * TAU).sin();
        let depth = 0.3 + 6. * size;
        let mut centre = v2(x, y) - self.sway * depth;
        let away = centre - self.pointer;
        let near = self.touch(centre, 0.25);
        centre = centre + away * (near * 0.06 / away.length().max(1e-3));
        centre = centre + self.ripples(centre).1 * 0.05;
        (centre, self.touch(centre, 0.3))
    }

    fn jellyfish(&self, p: V2) -> V3 {
        let t = self.time;
        let px = 1. / self.unit;
        let white = v3(1., 1., 1.);
        let mut color = self
            .base
            .mix(self.deep, ease(-0.75, 0.6, p.y + self.sway.y * 0.2));
        color = self.light(
            color,
            self.hues[3],
            ease(0.7, -0.6, p.y) * lerp(0.25, 0.18, self.dark),
        );
        let shaft = fbm(v2(p.x * 2.5 + p.y * 0.8 + t.sin() * 0.25, 3.));
        let shafts = ease(0.5, 0.8, shaft) * ease(0.5, -0.6, p.y) * 0.12;
        color = self.light(color, self.hues[3], shafts);

        for layer in 0..2 {
            let k = layer as f32;
            let cells = 26. + 18. * k;
            let sink = cells * (1. + k);
            let at = (p + self.sway * (0.2 + 0.4 * k)) * cells + v2(0., -t / TAU * sink);
            let cell = wrap(v2(at.x.floor(), at.y.floor()), sink);
            let seed = hash(cell + v2(k * 17., k * 17.));
            let spot = v2(fract(at.x), fract(at.y))
                - v2(0.5, 0.5)
                - (v2(hash(cell + v2(3.1, 3.1)), hash(cell + v2(5.7, 5.7))) - v2(0.5, 0.5)) * 0.6;
            let mote = step(0.9, seed) * ease(0.12, 0., spot.length()) * (0.25 + 0.2 * k);
            color = self.light(color, self.hues[2].mix(white, 0.5), mote);
        }

        for (i, &jelly) in JELLIES.iter().enumerate() {
            let size = jelly[1];
            let (centre, warm) = self.jelly_at(jelly);
            let l = p - centre;
            if l.x.abs() > size * 2.4 || l.y < -size * 2. || l.y > size * 4.6 {
                continue;
            }
            let pulse = (0.5 + 0.5 * (t * JELLY_PULSES + jelly[3] * TAU).sin()).powi(2);
            let w = size * (1. - 0.14 * pulse);
            let h = size * 0.8 * (1. + 0.12 * pulse);
            let glow = self.hues[i % 4];
            let inner = glow.mix(white, 0.45);
            let reach = lerp(0.55, 1., ease(0.05, 0.13, size)) * (0.85 + 0.6 * warm);

            let u = l.x / w;
            let rim = h * (0.12 * (1. - u * u) - 0.05 * (u * PI * 4.).sin().abs());
            let e = v2(u, l.y / h).length();
            let sdf = ((e - 1.) * h).max(l.y - rim);
            let cover = ease(1.5 * px, -1.5 * px, sdf);
            let edge = ease(-0.3 * h, 0., sdf);
            let canal = (l.x.atan2(l.y + 1.3 * h) * 8.).cos().abs().powi(40) * 0.25;
            let mut gonads = 0.;
            for g in 0..4 {
                let gx = (g as f32 - 1.5) * 0.36 * w;
                let d = l - v2(gx, -0.42 * h);
                gonads += (-d.dot(d) / (size * size * 0.018)).exp();
            }
            let fill = (0.2 + 0.55 * edge + canal) * cover;
            let halo = (-sdf.max(0.) / (0.18 * size)).exp() * 0.22;

            let mut strands = 0.;
            if l.y > 0. {
                for k in 0..JELLY_TENTACLES {
                    let f = k as f32;
                    let root = (f / (JELLY_TENTACLES - 1) as f32 * 2. - 1.) * 0.85 * w;
                    let long = size * (2.6 + 1.2 * hash(v2(f, jelly[3] * 10.)));
                    let s = l.y / long;
                    if s > 1. {
                        continue;
                    }
                    let path = root * (1. - 0.35 * s)
                        + 0.18 * size * s * (s * 7. - t * 24. + f * 1.3 + jelly[3] * 9.).sin();
                    let width = px * (1.1 - 0.5 * s);
                    let d = (l.x - path).abs();
                    strands += ease(width + px, 0., d) * (1. - s).powf(1.2) * 0.55;
                }
                for a in 0..2 {
                    let side = a as f32 * 2. - 1.;
                    let s = l.y / (size * 1.9);
                    if s < 1. {
                        let path =
                            side * 0.12 * w + 0.1 * size * (s * 9. - t * 16. + side * 2.).sin();
                        let width = 0.07 * size * (1. - s) + px;
                        let frill = 0.6 + 0.4 * (s * 40. - t * 20. + side).sin();
                        strands += ease(width, 0., (l.x - path).abs()) * (1. - s) * frill * 0.4;
                    }
                }
            }

            color = self.light(color, glow, (halo + strands) * reach);
            color = self.light(color, inner, (fill + gonads * 0.35 * cover) * reach);
        }
        self.light(color, white, self.ripples(p).0.max(0.) * 0.08)
    }

    // shoal.wgsl

    fn path(&self, s: f32, t: f32) -> (V2, V2) {
        let width = self.size.x / self.unit;
        let reach = v2(0.5 * width * 0.78, 0.14 + 0.03 * s);
        let level = -0.1 + 0.24 * s;
        let a = t + s * PI;
        let b = 2. * t + s * 1.3;
        let at = v2(reach.x * a.sin(), reach.y * b.sin() + level);
        let heading = v2(reach.x * a.cos(), 2. * reach.y * b.cos());
        (at, heading * (1. / heading.length().max(1e-4)))
    }

    fn shoal(&self, p: V2) -> V3 {
        let t = self.time;
        let px = 1. / self.unit;
        let white = v3(1., 1., 1.);
        let mut color = self
            .base
            .mix(self.deep, ease(-0.8, 0.7, p.y + self.sway.y * 0.15));
        let water = self.hues[3].mix(self.hues[2], ease(-0.5, 0.5, p.x * 0.4 - p.y));
        color = self.light(color, water, lerp(0.45, 0.3, self.dark));
        let slant = p.x * 3. + p.y * 1.1 + t.sin() * 0.3 + self.sway.x * 2.;
        let shafts = ease(0.42, 0.8, fbm(v2(slant, 0.))) * ease(0.6, -0.55, p.y) * 0.22;
        let sunlit = self.hues[0].mix(white, 0.5);
        color = self.light(color, sunlit, shafts);
        let at = (p + self.sway * 0.3) * 34. + v2(0., t / TAU * 34.);
        let cell = wrap(v2(at.x.floor(), at.y.floor()), 34.);
        let spot = v2(fract(at.x), fract(at.y))
            - v2(0.5, 0.5)
            - (v2(hash(cell + v2(1.3, 1.3)), hash(cell + v2(7.1, 7.1))) - v2(0.5, 0.5)) * 0.6;
        color = self.light(
            color,
            sunlit,
            step(0.93, hash(cell)) * ease(0.1, 0., spot.length()) * 0.25,
        );

        let silver =
            (self.deep.mix(self.hues[3], 0.35) * 0.55).mix(self.hues[3].mix(white, 0.6), self.dark);
        let belly = silver.mix(white.mix(sunlit, 0.3), lerp(0.25, 0.6, self.dark));
        for s in 0..SHOAL_SCHOOLS {
            let school = s as f32;
            let scale = lerp(0.8, 1.2, school);
            let depth = 0.3 + 0.5 * school;
            let clear = lerp(0.55, 1., school);
            for f in 0..SHOAL_FISH {
                let n = f as f32 + school * 31.;
                let lag = -hash(v2(n, 1.)) * 0.8;
                let side = (hash(v2(n, 2.)) - 0.5) * 0.13 * scale;
                let size = (0.028 + 0.014 * hash(v2(n, 3.))) * scale;
                let (course, mut heading) = self.path(school, t + lag);
                let across = v2(-heading.y, heading.x);
                let mut at = course + across * side + self.orbit(5. + (f % 3) as f32, n) * 0.008
                    - self.sway * depth;
                let reach = 2. * size + 0.14;
                if (p - at).dot(p - at) > reach * reach {
                    continue;
                }
                let away = at - self.pointer;
                let fear = self.touch(at, 0.2);
                let flight = away * (1. / away.length().max(1e-3));
                at = at + flight * (fear * 0.11);
                heading = (heading + flight * (fear * 1.6)).normalize();
                at = at + self.ripples(at).1 * 0.03;

                let l = p - at;
                if l.dot(l) > size * size * 3.5 {
                    continue;
                }
                let x = l.dot(heading) / size;
                let mut y = l.dot(v2(-heading.y, heading.x)) / size;
                let beat = t * SHOAL_BEATS + n * 1.7;
                y -= 0.1 * (beat - x * 2.).sin() * (1. - x) * (1. - x) * 0.25;
                let half = 0.26 * (1. - x * x).max(0.).sqrt() * (1. + 0.25 * x);
                let mut sdf = if x.abs() > 1. { 1. } else { y.abs() - half };
                let fan = 0.34 * ease(-0.85, -1.5, x);
                let notch = fan * 0.45 * ease(-1.25, -1.55, x);
                if x < -0.8 && x > -1.55 {
                    sdf = sdf.min((y.abs() - fan).max(notch - y.abs()));
                }
                let cover = ease(px / size, -px / size, sdf);
                if cover <= 0. {
                    continue;
                }
                let up = l.dot(v2(0., -1.)) / size;
                let mut body = belly.mix(silver, ease(-0.15, 0.2, up));
                let glint = (t * 7. + n * 2.3).sin().max(0.).powi(16);
                body = body.mix(white.mix(sunlit, 0.4), glint * lerp(0.25, 0.6, self.dark));
                let eye = ease(0.08, 0.05, v2(x - 0.62, y).length());
                body = body.mix(self.deep * 0.5, eye * 0.8);
                color = color.mix(color.mix(body, clear), cover * 0.92);
            }
        }
        color
    }
}

const SOLAR_RIDGES: usize = 3;

/// As `GHOSTS` in `solar.wgsl`.
const SOLAR_GHOSTS: [[f32; 2]; 4] = [[0.45, 0.035], [0.8, 0.02], [1.3, 0.06], [1.75, 0.03]];

/// As `TILT`, `WIND` and `METEORS` in `galaxy.wgsl`.
const GALAXY_TILT: f32 = 0.45;
const GALAXY_WIND: f32 = 4.2;
const GALAXY_METEORS: usize = 4;

/// As `SWARM`, `TENTACLES` and `PULSES` in `jellyfish.wgsl`.
const JELLIES: [[f32; 4]; 7] = [
    [-0.7, 0.055, 1., 0.1],
    [0.55, 0.06, 1., 0.62],
    [0.1, 0.07, 1., 0.35],
    [-0.3, 0.085, 1., 0.8],
    [0.8, 0.095, 1., 0.05],
    [-0.85, 0.115, 1., 0.5],
    [0.35, 0.135, 1., 0.22],
];
const JELLY_TENTACLES: usize = 7;
const JELLY_PULSES: f32 = 20.;

/// As `SCHOOLS`, `FISH` and `BEATS` in `shoal.wgsl`.
const SHOAL_SCHOOLS: usize = 2;
const SHOAL_FISH: usize = 26;
const SHOAL_BEATS: f32 = 100.;

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

fn wrap(cell: V2, period: f32) -> V2 {
    v2(cell.x, cell.y - (cell.y / period).floor() * period)
}

fn step(edge: f32, x: f32) -> f32 {
    if x >= edge { 1. } else { 0. }
}

/// How far `p` is from the segment from `a` to `b`.
fn segment(p: V2, a: V2, b: V2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.dot(ab).max(1e-8)).clamp(0., 1.);
    (p - a - ab * t).length()
}

/// `v` turned by the angle whose cosine and sine are `turn`.
fn rotate(v: V2, turn: V2) -> V2 {
    v2(v.x * turn.x - v.y * turn.y, v.x * turn.y + v.y * turn.x)
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
    fn every_scene_answers_the_pointer_only_if_it_says_so() {
        for ambience in Ambience::ALL {
            let still = render(ambience, &frame(0.2));
            let mut touched = frame(0.2);
            touched.response.presence = 1.;
            touched.response.pointer = [0.1, 0.05];
            touched.response.velocity = [0.6, 0.];
            touched.response.waves[0] = [0.1, 0.05, 0.08, 1.];
            touched.response.wave_count = 1;
            // Aurora answers through its stars alone: one awake, joined to the next.
            if ambience == Ambience::Aurora {
                assert_eq!(
                    render(ambience, &touched),
                    still,
                    "its curtains leave it alone"
                );
                let sky = &mut touched.sky;
                sky.stars[3][2] = 1.;
                let [a, b] = [sky.stars[3], sky.stars[4]];
                sky.lines[0] = [a[0], a[1], b[0], b[1]];
                sky.strengths[0] = 1.;
                sky.line_count = 1;
            }
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
