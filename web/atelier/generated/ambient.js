// Atelier ambient backdrops, written by `atelier::web`. Do not edit: change
// src/ambient and run `ATELIER_BLESS_WEB=1 cargo test --lib web`.

export const layout = {
  waves: 36,
  waveCount: 8,
  sky: 68,
  stars: 72,
  lines: 184,
  strengths: 376,
  meteors: 424,
  counts: 432,
  length: 2484,
};

export const palettes = {
  "studio-light": { dark: false, base: [0.9137, 0.9137, 0.9216], deep: [0.7675, 0.8441, 0.9341], hues: [[0.9286, 0.6239, 0.3769], [0.8765, 0.4647, 0.7118], [0.5471, 0.5333, 0.9369], [0.3000, 0.6349, 1.0000]] },
  "studio-dark": { dark: true, base: [0.0992, 0.1471, 0.2024], deep: [0.0476, 0.0476, 0.0512], hues: [[1.0000, 0.6235, 0.2706], [0.9412, 0.3569, 0.7098], [0.4902, 0.4784, 1.0000], [0.0392, 0.5176, 1.0000]] },
  "glass-light": { dark: false, base: [0.9608, 0.9608, 0.9686], deep: [0.8071, 0.8780, 0.9561], hues: [[0.9286, 0.6239, 0.3769], [0.8765, 0.4647, 0.7118], [0.5471, 0.5333, 0.9369], [0.3000, 0.6102, 0.9231]] },
  "glass-dark": { dark: true, base: [0.0816, 0.1294, 0.1918], deep: [0.0388, 0.0388, 0.0459], hues: [[1.0000, 0.6235, 0.2706], [0.9412, 0.3569, 0.7098], [0.4902, 0.4784, 1.0000], [0.0392, 0.5176, 1.0000]] },
  "mono-light": { dark: false, base: [1.0000, 1.0000, 1.0000], deep: [0.8971, 0.8820, 0.9856], hues: [[0.6404, 0.5525, 0.9588], [0.5498, 0.4839, 0.9369], [0.4922, 0.4318, 0.8518], [0.5498, 0.4839, 0.9369]] },
  "mono-dark": { dark: true, base: [0.0757, 0.0698, 0.1176], deep: [0.0106, 0.0106, 0.0106], hues: [[0.6549, 0.5451, 0.9804], [0.5451, 0.4863, 0.9647], [0.4275, 0.3725, 0.9412], [0.5451, 0.4863, 0.9647]] },
};

export const common = `// What every ambient backdrop shares: its parameters, the triangle that covers the frame, noise,
// the ripples the pointer leaves, and how light is laid over the background in each appearance.
// Each backdrop's own file adds \`scene\`, the colour at a point.
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
    // Aurora's sky (see \`stars.rs\`): how many lines and shooting stars there are; each brighter
    // star's place, how awake it is and its size; each line's ends, and their strengths, four to
    // a vector; each shooting star's head, heading and age.
    line_count: u32,
    meteor_count: u32,
    stars: array<vec4f, 28>,
    lines: array<vec4f, 48>,
    strengths: array<vec4f, 12>,
    meteors: array<vec4f, 2>,
    // Formation's and Garden's pieces (see \`figures.rs\`): how many of each kind, and each piece
    // as two vectors.
    counts: vec4u,
    figures: array<vec4f, 512>,
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

// Hermite easing from \`edge0\` to \`edge1\`, either way round.
fn ease(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0., 1.);
    return t * t * (3. - 2. * t);
}

fn hash(p: vec2f) -> f32 {
    let q = fract(p * vec2f(123.34, 456.21));
    let r = q + dot(q, q + 45.32);
    return fract(r.x * r.y);
}

// A grid \`cell\`, repeating every \`period\` cells down, for patterns that scroll and loop.
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

// A point that goes round a circle \`turns\` times a cycle.
fn orbit(turns: f32, start: f32) -> vec2f {
    let angle = params.time * turns + start;
    return vec2f(cos(angle), sin(angle));
}

// How much the pointer touches \`p\`: 1 under it, falling off over \`reach\`.
fn touch(p: vec2f, reach: f32) -> f32 {
    let d = p - params.pointer;
    return params.presence * exp(-dot(d, d) / (reach * reach));
}

// The height of the pointer's ripples at \`p\` (about -1 to 1), and which way they run there.
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

// \`color\` laid over \`background\` with weight \`amount\`: added as light in a dark appearance,
// laid on as a wash of colour in a light one.
fn light(background: vec3f, color: vec3f, amount: f32) -> vec3f {
    let glow = 1. - (1. - background) * (1. - color * max(amount, 0.));
    let wash = mix(background, color, clamp(amount, 0., 1.));
    return mix(wash, glow, params.dark);
}

// How far \`p\` is from the segment from \`a\` to \`b\`.
fn segment(p: vec2f, a: vec2f, b: vec2f) -> f32 {
    let ab = b - a;
    let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-8), 0., 1.);
    return length(p - a - ab * t);
}

// \`v\` turned by the angle whose cosine and sine are \`turn\`.
fn rotate(v: vec2f, turn: vec2f) -> vec2f {
    return vec2f(v.x * turn.x - v.y * turn.y, v.x * turn.y + v.y * turn.x);
}

fn luminance(color: vec3f) -> f32 {
    return dot(color, vec3f(0.2126, 0.7152, 0.0722));
}
`;

export const scenes = {
  Aurora: { period: 40, shader: `// Aurora: three curtains of light hang over a night sky, folding slowly as they drift, each lit
// in fine rays that brighten towards its lower hem. Moving the window swings the curtains, the
// nearest furthest. Still water below mirrors it all. Among the stars are a few brighter ones the
// pointer wakes and joins into constellations, and lines the person draws between them, and
// shooting stars (see \`stars.rs\`); the curtains themselves leave the pointer alone.

fn curtain(p: vec2f, index: f32) -> vec4f {
    let t = params.time;
    // Nearer curtains are lower, larger and swing further.
    let depth = 0.55 + 0.25 * index;
    let q = p + params.sway * depth;

    // The hem: a slow fold with a faster ripple on it.
    let x = q.x / depth;
    let hem = 0.02 + 0.07 * index
        + 0.06 * sin(x * 1.6 + t + index * 2.1)
        + 0.035 * sin(x * 3.7 - 2. * t + index * 4.3)
        + 0.015 * sin(x * 7.9 + 3. * t + index);
    let above = hem - q.y;
    // Bright along the hem, fading up into the sky, with a little glow below it.
    let height = 0.22 + 0.06 * index;
    let body = exp(-max(above, 0.) / height) * ease(-0.03, 0.006, above);
    // Fine rays, leaning as the curtain folds and drifting along it.
    let lean = 0.6 * sin(x * 1.2 - t + index);
    let drift = 0.4 * sin(t + index);
    let ray_at = vec2f((x + above * lean) * 16. + drift, index * 7.3 + above * 1.5);
    let rays = 0.3 + 1.1 * ease(0.3, 0.85, fbm(ray_at));
    // Where the curtain folds towards us it is brighter.
    let fold = 0.35 + 0.65 * ease(-0.6, 1., sin(x * 2.3 - t * 2. + index * 1.7));
    let strength = body * rays * fold;
    // Cool at the hem, warmer towards the top.
    let h = clamp(above / height * 0.7, 0., 1.);
    let low = params.hues[(3u + u32(index) * 2u) % 4u].rgb;
    let high = params.hues[(u32(index) + 1u) % 4u].rgb;
    let color = mix(low, high, h);
    return vec4f(color, strength * (0.85 - index * 0.15));
}

// The sky and its curtains at \`p\`.
fn sky(p: vec2f) -> vec3f {
    // Deep overhead, lighter towards the horizon.
    let horizon = ease(-0.7, LAKE, p.y + params.sway.y * 0.2);
    var color = mix(params.deep.rgb, params.base.rgb, horizon);
    // Stars, in a dark appearance, far enough away to barely move.
    let at = (p + params.sway * 0.15) * 90.;
    let star = hash(floor(at));
    let centre = fract(at) - 0.5;
    let twinkle = 0.6 + 0.4 * sin(params.time * 3. + star * 40.);
    let stars = step(0.985, star) * ease(0.18, 0., length(centre)) * twinkle * (1. - horizon);
    color += vec3f(stars * params.dark * 0.7);
    color = constellations(p + params.sway * 0.15, color);

    for (var i = 0; i < 3; i++) {
        let c = curtain(p, f32(i));
        color = light(color, c.rgb, c.a);
    }
    return color;
}

// The brighter stars at \`q\` (in the sky's own frame), the lines between them and the shooting
// stars, over \`color\`.
fn constellations(q: vec2f, color: vec3f) -> vec3f {
    let px = 1. / min(params.size.x, params.size.y);
    // Starlight over the dark; in a light appearance, ink.
    let ink = mix(
        mix(params.deep.rgb, params.hues[3].rgb, 0.45) * 0.6,
        mix(vec3f(1.), params.hues[0].rgb, 0.2),
        params.dark,
    );
    let line_ink = mix(ink, params.hues[1].rgb, 0.35);
    var c = color;
    for (var i = 0u; i < params.line_count; i++) {
        let line = params.lines[i];
        let strength = params.strengths[i / 4u][i % 4u];
        let d = segment(q, line.xy, line.zw);
        let core = ease(1.3 * px, 0., d);
        let halo = exp(-d / (5. * px)) * 0.22;
        c = light(c, line_ink, strength * (core * 0.6 + halo));
    }
    for (var i = 0u; i < 28u; i++) {
        let star = params.stars[i];
        let d = q - star.xy;
        let r = length(d);
        if r > 0.09 {
            continue;
        }
        let awake = star.z;
        let twinkle = 0.75 + 0.25 * sin(params.time * 5. + f32(i) * 2.4);
        let radius = (1. + 1.4 * star.w + awake * 0.9) * px;
        let core = ease(radius, 0., r) * (0.45 + 0.35 * star.w + 0.4 * awake) * twinkle;
        let halo = exp(-r * r / (radius * radius * 10.)) * (0.12 + 0.5 * awake);
        // Four thin spikes while it is awake.
        let spikes = (exp(-abs(d.x) / (0.6 * px)) + exp(-abs(d.y) / (0.6 * px)))
            * exp(-r / (0.02 * (0.3 + awake))) * awake * 0.35;
        c = light(c, mix(ink, params.hues[(i + 1u) % 4u].rgb, 0.25 * awake), core + halo + spikes);
    }
    for (var i = 0u; i < params.meteor_count; i++) {
        let meteor = params.meteors[i];
        let heading = vec2f(cos(meteor.z), sin(meteor.z));
        let age = meteor.w;
        let tail = meteor.xy - heading * (0.06 + 0.14 * (1. - age));
        let d = segment(q, meteor.xy, tail);
        // Brighter towards the head.
        let along = clamp(dot(q - tail, heading) / max(length(meteor.xy - tail), 1e-4), 0., 1.);
        let fade = sin(min(age * 1.2, 1.) * PI);
        let streak = ease(1.6 * px, 0., d) * along * along + exp(-d / (4. * px)) * along * 0.3;
        c = light(c, ink, streak * fade);
    }
    return c;
}

// Where the sky meets still water, which mirrors it.
const LAKE: f32 = 0.24;

fn scene(p: vec2f) -> vec3f {
    if p.y < LAKE {
        return sky(p);
    }
    // The water mirrors the sky, darker, barely stirring.
    let depth = p.y - LAKE;
    let wobble = 0.004 * sin(depth * 140. / (0.2 + depth) + params.time * 4.);
    let mirrored = sky(vec2f(p.x + wobble, LAKE - depth * 1.15));
    let water = mix(params.deep.rgb, params.base.rgb, 0.5);
    let color = mix(water, mirrored, mix(0.7, 0.6, params.dark) * exp(-depth * 1.8));
    // A thin line of light where they meet.
    return light(color, params.hues[3].rgb, exp(-depth / 0.004) * 0.25);
}
` },
  Dunes: { period: 48, shader: `// Dunes: sheets of silk lie in layers, one in front of another, their crests rolling slowly.
// Each is lit from above, with a sheen along its slopes and a soft shadow where it falls on the
// one behind. Moving the window slides the layers apart, the nearest furthest. It leaves the
// pointer alone.

const LAYERS: i32 = 5;

// The crest of layer \`index\` at \`x\`.
fn crest(x: f32, index: f32) -> f32 {
    let t = params.time;
    let level = -0.26 + 0.16 * index;
    return level
        + 0.09 * sin(x * (1.1 + 0.25 * index) + t + index * 1.9)
        + 0.045 * sin(x * (2.6 - 0.2 * index) - 2. * t + index * 3.7)
        + 0.015 * sin(x * 6.1 + 3. * t + index * 5.3);
}

fn sheet(index: f32) -> vec3f {
    // Back to front from the far end of the palette, darker and nearer the sky further back.
    let i = u32(index);
    let hue = mix(params.hues[i % 4u].rgb, params.hues[(i + 1u) % 4u].rgb, 0.35);
    let far = 1. - index / f32(LAYERS - 1);
    let tone = mix(params.deep.rgb, params.base.rgb, 1. - params.dark);
    return mix(hue, tone, far * 0.55 + 0.1);
}

fn scene(p: vec2f) -> vec3f {
    // The sky, with a soft sun above right.
    var color = mix(params.deep.rgb, params.base.rgb, ease(-0.8, 0.4, p.y));
    let sun = p - vec2f(0.55, -0.42) + params.sway * 0.1;
    color = light(color, params.hues[0].rgb, exp(-dot(sun, sun) / 0.09) * 0.45);

    for (var n = 0; n < LAYERS; n++) {
        let index = f32(n);
        let depth = 0.3 + 0.18 * index;
        let q = p + params.sway * depth;
        let x = q.x / (0.8 + 0.1 * index);
        let e = 0.004;
        let y = crest(x, index);
        let slope = (crest(x + e, index) - crest(x - e, index)) / (2. * e);
        let below = q.y - y;
        // The shadow this sheet casts on what is behind it, just above its crest.
        let shadow = exp(-max(-below, 0.) / 0.05) * ease(0.003, -0.003, below);
        color = color * (1. - shadow * mix(0.18, 0.45, params.dark));
        // The sheet, its edge a pixel and a half wide.
        let pixel = 1.5 / min(params.size.x, params.size.y);
        let cover = ease(-pixel, pixel, below * inverseSqrt(1. + slope * slope));
        // Lit from above: brightest just under the crest, darker as the silk falls away.
        let fall = 1. - exp(-max(below, 0.) / 0.12);
        var silk = sheet(index) * (1.08 - fall * 0.35);
        // A sheen where the slope faces the light.
        let facing = normalize(vec2f(slope, -1.));
        let sheen = pow(max(dot(facing, normalize(vec2f(0.45, -1.))), 0.), 24.) * exp(-max(below, 0.) / 0.2);
        silk = silk + vec3f(sheen * 0.22) * (1. - fall);
        // A thin bright line along the crest.
        silk += vec3f(exp(-max(below, 0.) / 0.004) * 0.12);
        color = mix(color, silk, cover);
    }
    return color;
}
` },
  Bloom: { period: 36, shader: `// Bloom: a few great clouds of colour drift and melt into one another, their edges stirred by
// slow eddies, under a faint frosted sheen. The pointer stirs the colour round itself and drags
// it along; its ripples spread through it; moving the window leaves the clouds behind, each by
// its own amount, so the colour sloshes back.

// Each cloud: where its orbit is centred, how far it swings and how fast (whole turns a cycle),
// and its size.
const CLOUDS: array<vec4f, 5> = array<vec4f, 5>(
    vec4f(-0.55, -0.25, 0.22, 0.55),
    vec4f(0.5, -0.3, 0.18, 0.5),
    vec4f(0.1, 0.35, 0.25, 0.6),
    vec4f(-0.2, 0.05, 0.3, 0.42),
    vec4f(0.75, 0.3, 0.2, 0.45),
);

fn scene(p: vec2f) -> vec3f {
    let t = params.time;
    // The pointer stirs the colour round itself and drags it along.
    let near = touch(p, 0.3);
    let d = p - params.pointer;
    let angle = near * (0.9 + clamp(length(params.velocity), 0., 2.) * 0.5);
    var q = params.pointer + vec2f(cos(angle) * d.x - sin(angle) * d.y, sin(angle) * d.x + cos(angle) * d.y);
    q -= params.velocity * near * 0.06;
    // The ripples push the colour outwards as they pass.
    let wave = ripples(q);
    q -= wave.yz * 0.025;
    // Slow eddies.
    let warp = vec2f(
        fbm(q * 1.3 + orbit(1., 0.) * 0.5),
        fbm(q * 1.3 + vec2f(5.2, 1.3) + orbit(1., 2.) * 0.5),
    );
    q += (warp - 0.5) * 0.6;

    // The clouds, each a soft field of one hue.
    var clouds = CLOUDS;
    var sum = vec3f(0.);
    var weight = 0.;
    for (var i = 0; i < 5; i++) {
        let cloud = clouds[i];
        let lag = 0.6 + 0.25 * f32(i % 3);
        let centre = cloud.xy + cloud.z * orbit(f32(1 + i % 2), f32(i) * 1.7) * vec2f(1., 0.7)
            - params.sway * lag;
        let r = q - centre;
        let w = exp(-dot(r, r) / (cloud.w * cloud.w));
        sum += params.hues[i % 4].rgb * w;
        weight += w;
    }
    let hue = sum / max(weight, 1e-4);
    let amount = ease(0., 1.3, weight);
    var color = mix(params.deep.rgb, params.base.rgb, ease(-0.9, 0.9, p.y - p.x * 0.3));
    color = light(color, hue, amount * mix(0.8, 0.85, params.dark));

    // A frosted sheen: faint bright folds where the eddies run fast.
    let fold = fbm(q * 2.2 - orbit(2., 0.5) * 0.3);
    let sheen = ease(0.07, 0., abs(fold - 0.5)) * 0.05 + near * 0.06;
    color = light(color, vec3f(1.), sheen * (0.4 + 0.6 * params.dark));
    // Light caught in the ripples.
    color = light(color, vec3f(1.), max(wave.x, 0.) * 0.15);
    return color;
}
` },
  Caustics: { period: 32, shader: `// Caustics: light through moving water, playing in a net of bright lines over a deep floor, with
// shafts of light slanting down from the surface. Each colour bends a little differently, so the
// lines have thin coloured fringes. The pointer is a lens on the water, gathering the light under
// it; its ripples bend the net as they spread; moving the window sloshes the water.

// Speeds of the water's layers, in whole turns a cycle.
const SPEEDS: array<f32, 5> = array<f32, 5>(-2., 1., -1., 2., 1.);

// The net of light at \`p\`: 0 to about 1.
fn net(p: vec2f) -> f32 {
    var speeds = SPEEDS;
    let t = params.time;
    var i = p;
    var c = 1.;
    let intensity = 0.005;
    for (var n = 0; n < 5; n++) {
        let s = t * speeds[n] + f32(n) * 1.3;
        i = p + vec2f(cos(s - i.x) + sin(s + i.y), sin(s - i.y) + cos(s + i.x));
        c += 1. / length(vec2f(p.x / (sin(i.x + s) / intensity), p.y / (cos(i.y + s) / intensity)));
    }
    c = 1.17 - pow(c / 5., 1.4);
    return pow(abs(c), 8.);
}

fn scene(p: vec2f) -> vec3f {
    // The floor, nearer the surface towards the top.
    let floor = p + params.sway * 0.4;
    var color = mix(params.base.rgb, params.deep.rgb, ease(-0.7, 0.8, floor.y));
    // The water's colour drifts slowly over the floor.
    let tint_at = floor * 0.9 + orbit(1., 0.) * 0.15;
    let tint = fbm(tint_at);
    let hue = mix(
        mix(params.hues[3].rgb, params.hues[2].rgb, ease(0.3, 0.6, tint)),
        params.hues[1].rgb,
        ease(0.6, 0.85, tint),
    );
    color = light(color, hue, mix(0.6, 0.3, params.dark));

    // The pointer is a lens on the water; the ripples bend the light as they spread; moving the
    // window sloshes the water further than the floor.
    let near = touch(floor, 0.25);
    var q = floor + params.sway * 0.8;
    q = params.pointer + (q - params.pointer) * (1. - near * 0.3);
    let wave = ripples(q);
    q += wave.yz * 0.06;
    let cell = q * 6.;
    let spread = 0.012;
    let lines = vec3f(
        net(cell * (1. + spread) - 250.),
        net(cell - 250.),
        net(cell * (1. - spread) - 250.),
    );
    let glow = lines * (0.8 + near * 0.7 + max(wave.x, 0.) * 0.5);
    let shine = mix(hue, vec3f(1.), mix(0.85, 0.55, params.dark));
    // Each channel on its own, for the fringes.
    let strength = glow * mix(0.8, 1., params.dark);
    color = vec3f(
        light(color, shine, strength.r).r,
        light(color, shine, strength.g).g,
        light(color, shine, strength.b).b,
    );

    // Shafts of light from the surface, fading with depth.
    let shaft = fbm(vec2f(p.x * 3.5 + p.y * 1.2 + sin(params.time) * 0.3, 0.));
    let shafts = ease(0.45, 0.8, shaft) * ease(0.6, -0.5, p.y) * 0.16;
    color = light(color, shine, shafts);
    return color;
}
` },
  Solar: { period: 60, shader: `// Solar: a low sun in a sky of graded light, after the sunset in bevytris. Wide bands of colour
// ring the sun, warmest nearest, and breathe slowly; rays turn in its corona; soft clouds drift
// in the warm light; and three ridges stand in haze below it, the nearest darkest, each with a
// rim of light along its crest. The pointer is a lens: ghosts of the sun gather along the line
// from it to the pointer. Moving the window slides the ridges apart, the nearest furthest.

const SUN: vec2f = vec2f(0.46, 0.08);
const RIDGES: i32 = 3;
// Each lens ghost: how far along the line from the sun to the pointer it lies, and its size.
const GHOSTS: array<vec2f, 4> = array<vec2f, 4>(
    vec2f(0.45, 0.035),
    vec2f(0.8, 0.02),
    vec2f(1.3, 0.06),
    vec2f(1.75, 0.03),
);

// The crest of ridge \`index\` (0 farthest) at \`x\`.
fn ridge(x: f32, index: f32) -> f32 {
    let level = 0.17 + 0.1 * index;
    return level
        + 0.05 * sin(x * (1.3 + 0.4 * index) + index * 2.3)
        + 0.025 * sin(x * (3.1 + 0.7 * index) + index * 5.1)
        + 0.024 * (fbm(vec2f(x * 4. + index * 7., index)) - 0.5);
}

fn scene(p: vec2f) -> vec3f {
    let t = params.time;
    let px = 1. / min(params.size.x, params.size.y);
    let sun = SUN + vec2f(0., 0.012 * sin(t)) - params.sway * 0.1;
    let to_sun = p - sun;
    // The bands ring the sun, a little wider than tall, their edges stirred.
    let stir = fbm(p * 1.4 + orbit(1., 0.) * 0.3) - 0.5;
    let r = length(to_sun * vec2f(1., 1.3)) + stir * 0.07;
    let breath = 1. + 0.05 * sin(t * 2. + 1.);

    // From the deep sky in to the sun: the coolest colour farthest away, the warmest nearest.
    // Over the dark it stays a dusk, so the card above it still stands out.
    let glow = mix(1., 0.7, params.dark);
    var color = mix(params.deep.rgb, params.base.rgb, ease(-0.7, 0.45, p.y));
    color = light(color, params.hues[3].rgb, ease(1.7 * breath, 0.75 * breath, r) * 0.5 * glow);
    color = light(color, params.hues[2].rgb, ease(1.1 * breath, 0.4 * breath, r) * 0.55 * glow);
    color = light(color, params.hues[1].rgb, ease(0.7 * breath, 0.18 * breath, r) * 0.65 * glow);
    color = light(color, params.hues[0].rgb, ease(0.38 * breath, 0.05, r) * 0.8);

    // Rays turning in the corona, a turn a cycle.
    let direction = to_sun / max(length(to_sun), 1e-4);
    let spun = rotate(direction, orbit(1., 0.));
    let rays = ease(0.45, 0.85, fbm(spun * 3.5 + 11.)) * exp(-r / 0.35) * ease(0.04, 0.1, r);
    let hot = mix(params.hues[0].rgb, vec3f(1.), mix(0.35, 0.7, params.dark));
    color = light(color, hot, rays * 0.35);

    // Soft clouds in two bands, lit warm on the side towards the sun.
    for (var i = 0; i < 2; i++) {
        let k = f32(i);
        let depth = 0.25 + 0.2 * k;
        let q = p + params.sway * depth;
        let at = vec2f(q.x * (1.6 - 0.4 * k), q.y * 9.) + vec2f(k * 13., 0.)
            + orbit(1., k * 2.) * vec2f(0.8, 0.15);
        let height = -0.12 - 0.15 * k;
        let band = ease(0.09, 0., abs(q.y - height) - 0.03);
        let cloud = ease(0.5, 0.8, fbm(at)) * band;
        let lit = mix(params.hues[1].rgb, params.hues[0].rgb, exp(-r / 0.6));
        color = light(color, mix(lit, vec3f(1.), 0.25), cloud * 0.4);
    }

    // The sun: a bright disc with a soft glow, and a thin streak of light through it.
    let disc = ease(0.07, 0.064, length(to_sun));
    color = light(color, hot, disc * 0.95 + exp(-r / 0.05) * 0.5);
    let streak = exp(-abs(to_sun.y) / 0.004) * exp(-abs(to_sun.x) / 0.5) * 0.18;
    color = light(color, hot, streak);

    // The ridges, the farthest palest in the haze.
    let tone = mix(
        mix(params.base.rgb, params.hues[3].rgb, 0.45) * 0.9,
        mix(params.deep.rgb, params.hues[3].rgb, 0.15) * 0.65,
        params.dark,
    );
    for (var i = 0; i < RIDGES; i++) {
        let k = f32(i);
        let depth = 0.3 + 0.3 * k;
        let q = p + params.sway * depth;
        let top = ridge(q.x, k);
        let below = q.y - top;
        let cover = ease(-px, px, below);
        let haze = 1. - (k + 1.) / f32(RIDGES + 1);
        // Hazier at the foot of each ridge, where mist lies.
        let mist = ease(0., 0.18, below) * 0.15;
        var rock = mix(tone, color, haze * 0.75 + mist);
        // Light along the crest, strongest towards the sun.
        let rim = exp(-max(below, 0.) / 0.004) * exp(-abs(q.x - sun.x) / 0.5) * (0.35 - 0.08 * k);
        rock = light(rock, hot, rim);
        color = mix(color, rock, cover);
    }

    // The pointer's lens: ghosts of the sun along the line to it, and a glint under it.
    let axis = params.pointer - sun;
    var ghosts = GHOSTS;
    for (var i = 0; i < 4; i++) {
        let ghost = ghosts[i];
        let d = length(p - (sun + axis * ghost.x));
        let ring = ease(ghost.y, ghost.y * 0.75, d) * (0.4 + 0.6 * ease(ghost.y * 0.4, ghost.y, d));
        color = light(color, params.hues[i].rgb, ring * 0.14 * params.presence);
    }
    color = light(color, hot, touch(p, 0.04) * 0.2);
    return color;
}
` },
  Galaxy: { period: 90, shader: `// Galaxy: a spiral galaxy turns slowly in deep space, after the galaxy in bevytris: a bright core,
// two arms of stars wound round it with lanes of dust and glowing knots of gas along them, on a
// disc tilted so it reads as a wheel seen at an angle, over a field of stars and faint clouds,
// with now and then a shooting star in front. The pointer bends the light round itself like a
// great mass and leaves ripples; moving the window swings the galaxy against the stars behind.

// Where the galaxy lies, how far its disc is tilted, and how its arms wind.
const CENTRE: vec2f = vec2f(0.4, -0.08);
const TILT: f32 = 0.45;
const WIND: f32 = 4.2;
// Shooting stars a cycle.
const METEORS: i32 = 4;

// The stars at \`q\`, \`cells\` to a unit: one in a few cells, of \`share\` of them, twinkling.
fn starfield(q: vec2f, cells: f32, share: f32) -> f32 {
    let at = q * cells;
    let cell = floor(at);
    let seed = hash(cell);
    let spot = fract(at) - 0.5 - (vec2f(hash(cell + 2.3), hash(cell + 8.9)) - 0.5) * 0.5;
    let twinkle = 0.65 + 0.35 * sin(params.time * 4. + seed * 50.);
    return step(1. - share, seed) * ease(0.2, 0., length(spot)) * twinkle * (0.4 + 0.6 * fract(seed * 13.));
}

fn scene(p: vec2f) -> vec3f {
    let t = params.time;
    let px = 1. / min(params.size.x, params.size.y);
    // The pointer bends the light round itself, and its ripples ring out.
    let lens = p - params.pointer;
    let pull = params.presence * 0.003 / (length(lens) + 0.04);
    var q = p - lens / max(length(lens), 1e-4) * pull;
    let wave = ripples(q);
    q -= wave.yz * 0.02;

    // Deep space, a little lighter round the galaxy, with faint clouds and far stars.
    var color = mix(params.deep.rgb, params.base.rgb, ease(1.2, 0., length(p - CENTRE)) * 0.6);
    let far = q + params.sway * 0.1;
    let clouds = fbm(far * 1.6 + orbit(1., 0.) * 0.2);
    color = light(color, params.hues[2].rgb, ease(0.5, 0.85, clouds) * 0.18);
    color = light(color, params.hues[1].rgb, ease(0.62, 0.9, fbm(far * 2.3 + 4.)) * 0.12);
    // Starlight over the dark; in a light appearance, ink.
    let ink = mix(mix(params.deep.rgb, params.hues[3].rgb, 0.4) * 0.55, vec3f(1.), params.dark);
    color = light(color, ink, starfield(far, 70., 0.04) * mix(0.35, 0.8, params.dark));

    // The disc, untilted, and turned with the cycle: a turn a cycle.
    let d = rotate(q - CENTRE + params.sway * 0.4, vec2f(cos(-0.35), sin(-0.35)));
    let disc = vec2f(d.x, d.y / TILT);
    let r = length(disc);
    let spun = rotate(disc, orbit(-1., 0.));
    let angle = atan2(spun.y, spun.x);
    let arm = pow(0.5 + 0.5 * cos(2. * angle - log(max(r, 1e-3)) * WIND), 3.);
    let envelope = exp(-r / 0.32) * ease(1.15, 0.5, r);
    // Young stars light the arms, bluer outwards.
    let outer = mix(params.hues[2].rgb, params.hues[3].rgb, ease(0.1, 0.5, r));
    color = light(color, outer, arm * envelope * 0.75 + envelope * 0.12);
    // Lanes of dust along the inner edge of each arm.
    let lane = pow(0.5 + 0.5 * cos(2. * angle - log(max(r, 1e-3)) * WIND + 0.7), 6.);
    let dust = lane * ease(0.4, 0.7, fbm(spun * 9.)) * envelope * ease(0.04, 0.12, r);
    color = mix(color, color * mix(0.75, 0.45, params.dark), dust * 0.8);
    // Glowing knots of gas, and the arms' own stars.
    let knots = ease(0.62, 0.8, fbm(spun * 14. + 3.)) * arm * envelope;
    color = light(color, params.hues[1].rgb, knots * 0.6);
    color = light(color, mix(ink, outer, 0.3), starfield(spun, 110., 0.12) * (arm * 0.8 + 0.2) * envelope * 2.2);

    // The core: a warm bulge and a bright heart.
    let core = exp(-r * r / 0.012) * 0.65 + exp(-r / 0.025) * 0.6;
    let hot = mix(params.hues[0].rgb, vec3f(1.), mix(0.3, 0.6, params.dark));
    color = light(color, hot, core);

    // Shooting stars, each a moment of the cycle, crossing in front.
    for (var m = 0; m < METEORS; m++) {
        let k = f32(m);
        let age = (fract(t / TAU - k / f32(METEORS) - 0.07) ) / 0.03;
        if age > 1. {
            continue;
        }
        let start = vec2f(hash(vec2f(k, 1.)) * 1.4 - 0.7, hash(vec2f(k, 2.)) * 0.5 - 0.45);
        let heading = select(0.45, 2.7, start.x > 0.) + 0.2 * hash(vec2f(k, 3.));
        let towards = vec2f(cos(heading), sin(heading));
        let head = start + towards * age * 0.7;
        let tail = head - towards * 0.16;
        let along = clamp(dot(p - tail, towards) / 0.16, 0., 1.);
        let line = segment(p, head, tail);
        let streak = (ease(1.5 * px, 0., line) + exp(-line / (4. * px)) * 0.3) * along * along;
        color = light(color, ink, streak * sin(age * PI));
    }
    // Light caught in the ripples.
    color = light(color, vec3f(1.), max(wave.x, 0.) * 0.1);
    return color;
}
` },
  Jellyfish: { period: 48, shader: `// Jellyfish: a few jellyfish drift up through deep water, their bells pulsing slowly and their
// tentacles trailing, lit from within, with marine snow sinking past and faint light from the
// surface. The pointer warms the jellyfish near it, which glow brighter and shy away from it; its
// ripples push them aside as they spread. Moving the window leaves them behind, the nearest
// furthest.

const JELLIES: i32 = 7;
// Each jellyfish, farthest first: where it drifts up (across the frame, -1 to 1), its size, how
// many times it rises through the frame a cycle, and where in the cycle it starts.
const SWARM: array<vec4f, 7> = array<vec4f, 7>(
    vec4f(-0.7, 0.055, 1., 0.1),
    vec4f(0.55, 0.06, 1., 0.62),
    vec4f(0.1, 0.07, 1., 0.35),
    vec4f(-0.3, 0.085, 1., 0.8),
    vec4f(0.8, 0.095, 1., 0.05),
    vec4f(-0.85, 0.115, 1., 0.5),
    vec4f(0.35, 0.135, 1., 0.22),
);
const TENTACLES: i32 = 7;
// Pulses a cycle.
const PULSES: f32 = 20.;

// Where a jellyfish's bell is centred, and how much the pointer warms it.
fn jelly_at(jelly: vec4f) -> vec3f {
    let t = params.time;
    let width = params.size.x / min(params.size.x, params.size.y);
    let height = params.size.y / min(params.size.x, params.size.y);
    let size = jelly.y;
    // It rises from below the frame until its tentacles have left the top.
    let span = height + size * 5.;
    let rise = fract(jelly.w + jelly.z * t / TAU);
    let y = 0.5 * height + size * 0.9 - rise * span;
    let x = jelly.x * 0.5 * width * 0.85 + 0.04 * sin(t * 2. + jelly.w * TAU);
    let depth = 0.3 + 6. * size;
    var centre = vec2f(x, y) - params.sway * depth;
    // It shies away from the pointer and is pushed by the ripples.
    let away = centre - params.pointer;
    let near = touch(centre, 0.25);
    centre += away / max(length(away), 1e-3) * near * 0.06;
    centre += ripples(centre).yz * 0.05;
    return vec3f(centre, touch(centre, 0.3));
}

fn scene(p: vec2f) -> vec3f {
    let t = params.time;
    let px = 1. / min(params.size.x, params.size.y);
    // Lighter towards the surface, the deep water tinged with the coolest colour.
    var color = mix(params.base.rgb, params.deep.rgb, ease(-0.75, 0.6, p.y + params.sway.y * 0.2));
    color = light(color, params.hues[3].rgb, ease(0.7, -0.6, p.y) * mix(0.25, 0.18, params.dark));
    // Faint light from the surface.
    let shaft = fbm(vec2f(p.x * 2.5 + p.y * 0.8 + sin(t) * 0.25, 3.));
    let shafts = ease(0.5, 0.8, shaft) * ease(0.5, -0.6, p.y) * 0.12;
    color = light(color, params.hues[3].rgb, shafts);

    // Marine snow, in two layers, sinking a whole number of cells a cycle, over a pattern that
    // repeats every that many cells, so it loops.
    for (var layer = 0; layer < 2; layer++) {
        let k = f32(layer);
        let cells = 26. + 18. * k;
        let sink = cells * (1. + k);
        let at = (p + params.sway * (0.2 + 0.4 * k)) * cells + vec2f(0., -t / TAU * sink);
        let cell = wrap(floor(at), sink);
        let seed = hash(cell + k * 17.);
        let spot = fract(at) - 0.5 - (vec2f(hash(cell + 3.1), hash(cell + 5.7)) - 0.5) * 0.6;
        let mote = step(0.9, seed) * ease(0.12, 0., length(spot)) * (0.25 + 0.2 * k);
        color = light(color, mix(params.hues[2].rgb, vec3f(1.), 0.5), mote);
    }

    var swarm = SWARM;
    for (var i = 0; i < JELLIES; i++) {
        let jelly = swarm[i];
        let size = jelly.y;
        let at = jelly_at(jelly);
        let l = p - at.xy;
        // Only near the jellyfish: its bell above the centre, its tentacles hanging below.
        if abs(l.x) > size * 2.4 || l.y < -size * 2. || l.y > size * 4.6 {
            continue;
        }
        let warm = at.z;
        let pulse = pow(0.5 + 0.5 * sin(t * PULSES + jelly.w * TAU), 2.);
        let w = size * (1. - 0.14 * pulse);
        let h = size * 0.8 * (1. + 0.12 * pulse);
        let glow = params.hues[i % 4].rgb;
        let inner = mix(glow, vec3f(1.), 0.45);
        // Farther ones are fainter.
        let reach = mix(0.55, 1., ease(0.05, 0.13, size)) * (0.85 + 0.6 * warm);

        // The bell: a dome with a scalloped rim.
        let u = l.x / w;
        let rim = h * (0.12 * (1. - u * u) - 0.05 * abs(sin(u * PI * 4.)));
        let e = length(vec2f(u, l.y / h));
        let sdf = max((e - 1.) * h, l.y - rim);
        let cover = ease(1.5 * px, -1.5 * px, sdf);
        // Brightest at its edge, as light catches the thin rim.
        let edge = ease(-0.3 * h, 0., sdf);
        // Radial canals and four soft gonads within.
        let canal = pow(abs(cos(atan2(l.x, l.y + 1.3 * h) * 8.)), 40.) * 0.25;
        var gonads = 0.;
        for (var g = 0; g < 4; g++) {
            let gx = (f32(g) - 1.5) * 0.36 * w;
            let d = l - vec2f(gx, -0.42 * h);
            gonads += exp(-dot(d, d) / (size * size * 0.018));
        }
        let fill = (0.2 + 0.55 * edge + canal) * cover;
        let halo = exp(-max(sdf, 0.) / (0.18 * size)) * 0.22;

        // Tentacles trailing from the rim, waving down their length.
        var strands = 0.;
        if l.y > 0. {
            for (var k = 0; k < TENTACLES; k++) {
                let f = f32(k);
                let root = (f / f32(TENTACLES - 1) * 2. - 1.) * 0.85 * w;
                let long = size * (2.6 + 1.2 * hash(vec2f(f, jelly.w * 10.)));
                let s = l.y / long;
                if s > 1. {
                    continue;
                }
                let path = root * (1. - 0.35 * s)
                    + 0.18 * size * s * sin(s * 7. - t * 24. + f * 1.3 + jelly.w * 9.);
                let width = px * (1.1 - 0.5 * s);
                let d = abs(l.x - path);
                strands += ease(width + px, 0., d) * pow(1. - s, 1.2) * 0.55;
            }
            // Two frilled oral arms down the middle.
            for (var a = 0; a < 2; a++) {
                let side = f32(a) * 2. - 1.;
                let s = l.y / (size * 1.9);
                if s < 1. {
                    let path = side * 0.12 * w + 0.1 * size * sin(s * 9. - t * 16. + side * 2.);
                    let width = 0.07 * size * (1. - s) + px;
                    let frill = 0.6 + 0.4 * sin(s * 40. - t * 20. + side);
                    strands += ease(width, 0., abs(l.x - path)) * (1. - s) * frill * 0.4;
                }
            }
        }

        color = light(color, glow, (halo + strands) * reach);
        color = light(color, inner, (fill + gonads * 0.35 * cover) * reach);
    }
    // Light caught in the ripples.
    color = light(color, vec3f(1.), max(ripples(p).x, 0.) * 0.08);
    return color;
}
` },
};
