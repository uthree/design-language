// What every ambient backdrop shares: its parameters, the triangle that covers the frame, noise,
// the ripples the pointer leaves, and how light is laid over the background in each appearance.
// Each backdrop's own file adds `scene`, the colour at a point.
//
// Positions are in units of the frame's shorter side, from its centre, x right and y down.

struct Params {
    // The frame, in pixels.
    size: vec2f,
    // The cycle, in radians: everything moves by whole turns of it, so the loop is seamless.
    time: f32,
    // 1 in a dark appearance, 0 in a light one.
    dark: f32,
    // Where the pointer is, eased, and how present it is (0 to 1, fading in and out).
    pointer: vec2f,
    presence: f32,
    wave_count: u32,
    // How fast the pointer is moving, and how far the window's moves have left the scene behind.
    velocity: vec2f,
    sway: vec2f,
    base: vec4f,
    deep: vec4f,
    hues: array<vec4f, 4>,
    // Each ripple's centre, how far it has spread, and its strength.
    waves: array<vec4f, 8>,
    // Aurora's sky (see `stars.rs`): how many lines and shooting stars there are; each brighter
    // star's place, how awake it is and its size; each line's ends, and their strengths, four to
    // a vector; each shooting star's head, heading and age.
    line_count: u32,
    meteor_count: u32,
    stars: array<vec4f, 28>,
    lines: array<vec4f, 48>,
    strengths: array<vec4f, 12>,
    meteors: array<vec4f, 2>,
}

@group(0) @binding(0) var<uniform> params: Params;

const PI: f32 = 3.14159265;
const TAU: f32 = 6.28318531;
// A ripple's wavelength and the width of its band.
const WAVELENGTH: f32 = 0.045;
const BAND: f32 = 0.07;

@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
    // One triangle that covers the screen.
    let uv = vec2f(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4f(uv * 2. - 1., 0., 1.);
}

@fragment
fn fs(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let unit = min(params.size.x, params.size.y);
    let p = (position.xy - params.size * 0.5) / unit;
    // A little grain, a third of a step either way, so long gradients don't band.
    let grain = (hash(position.xy) - 0.5) / 255. * 1.4;
    return vec4f(scene(p) + grain, 1.);
}

// Hermite easing from `edge0` to `edge1`, either way round.
fn ease(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0., 1.);
    return t * t * (3. - 2. * t);
}

fn hash(p: vec2f) -> f32 {
    let q = fract(p * vec2f(123.34, 456.21));
    let r = q + dot(q, q + 45.32);
    return fract(r.x * r.y);
}

// A grid `cell`, repeating every `period` cells down, for patterns that scroll and loop.
fn wrap(cell: vec2f, period: f32) -> vec2f {
    return vec2f(cell.x, cell.y - floor(cell.y / period) * period);
}

// Smooth value noise, 0 to 1.
fn noise(p: vec2f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3. - 2. * f);
    let a = hash(i);
    let b = hash(i + vec2f(1., 0.));
    let c = hash(i + vec2f(0., 1.));
    let d = hash(i + vec2f(1., 1.));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Four octaves of noise, 0 to 1.
fn fbm(p: vec2f) -> f32 {
    var sum = 0.;
    var amplitude = 0.5;
    var q = p;
    for (var i = 0; i < 4; i++) {
        sum += amplitude * noise(q);
        q = q * 2.03 + vec2f(1.7, 9.2);
        amplitude *= 0.5;
    }
    return sum / 0.9375;
}

// A point that goes round a circle `turns` times a cycle.
fn orbit(turns: f32, start: f32) -> vec2f {
    let angle = params.time * turns + start;
    return vec2f(cos(angle), sin(angle));
}

// How much the pointer touches `p`: 1 under it, falling off over `reach`.
fn touch(p: vec2f, reach: f32) -> f32 {
    let d = p - params.pointer;
    return params.presence * exp(-dot(d, d) / (reach * reach));
}

// The height of the pointer's ripples at `p` (about -1 to 1), and which way they run there.
fn ripples(p: vec2f) -> vec3f {
    var result = vec3f(0.);
    let k = TAU / WAVELENGTH;
    for (var i = 0u; i < params.wave_count; i++) {
        let wave = params.waves[i];
        let d = p - wave.xy;
        let distance = max(length(d), 1e-4);
        let x = distance - wave.z;
        let u = x / BAND;
        // Rings flatten as they spread.
        let height = wave.w * exp(-u * u) * BAND / (BAND + wave.z) * ease(0., BAND, distance);
        result += vec3f(height * sin(k * x), d / distance * height);
    }
    return result;
}

// `color` laid over `background` with weight `amount`: added as light in a dark appearance,
// laid on as a wash of colour in a light one.
fn light(background: vec3f, color: vec3f, amount: f32) -> vec3f {
    let glow = 1. - (1. - background) * (1. - color * max(amount, 0.));
    let wash = mix(background, color, clamp(amount, 0., 1.));
    return mix(wash, glow, params.dark);
}

// How far `p` is from the segment from `a` to `b`.
fn segment(p: vec2f, a: vec2f, b: vec2f) -> f32 {
    let ab = b - a;
    let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-8), 0., 1.);
    return length(p - a - ab * t);
}

// `v` turned by the angle whose cosine and sine are `turn`.
fn rotate(v: vec2f, turn: vec2f) -> vec2f {
    return vec2f(v.x * turn.x - v.y * turn.y, v.x * turn.y + v.y * turn.x);
}

fn luminance(color: vec3f) -> f32 {
    return dot(color, vec3f(0.2126, 0.7152, 0.0722));
}
