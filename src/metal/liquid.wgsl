// The liquid-metal backdrop, shaded per pixel. The same liquid and studio as the CPU fallback in
// mod.rs, with four samples a pixel, dispersion, ripples on the surface, the ripples the pointer
// leaves, and a second softbox.
//
// Coordinates are pixels, x right and y down; directions are x right, y down, z towards the
// viewer.

struct Params {
    size: vec2f,
    // Ripple time, in radians: whole turns per cycle so the loop is seamless.
    time: f32,
    relief: f32,
    shadow_offset: f32,
    shadow_scale: f32,
    shadow: f32,
    count: u32,
    // The pointer's ripples: how many, their wavelength and the width of their band in pixels,
    // and their height relative to the relief.
    wave_count: u32,
    wavelength: f32,
    band: f32,
    wave_height: f32,
    background: vec4f,
    behind: vec4f,
    front: vec4f,
    ramp: array<vec4f, 9>,
    // Each drop's centre and reach, in pixels.
    drops: array<vec4f, 16>,
    // Each ripple's centre, how far it has spread, and its strength.
    waves: array<vec4f, 8>,
}

@group(0) @binding(0) var<uniform> params: Params;

const THRESHOLD: f32 = 0.25;
const PI: f32 = 3.14159265;
const SOFTBOX: vec3f = vec3f(-0.43, -0.62, 0.656);
const FILL_BOX: vec3f = vec3f(0.6, -0.55, 0.58);
const STRIP: vec3f = vec3f(0.78, -0.3, -0.55);
// How far apart the three channels bend: thin coloured fringes where the reflection changes fast.
const DISPERSION: f32 = 0.035;
// The ripples: how high, relative to the relief, and how many to a unit of field.
const RIPPLE: f32 = 0.0018;
const RIPPLE_FREQUENCY: f32 = 24.;

@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
    // One triangle that covers the screen.
    let uv = vec2f(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4f(uv * 2. - 1., 0., 1.);
}

// The field at `p`, and its gradient, from drops shifted down by `offset` and widened by `scale`.
fn field(p: vec2f, offset: f32, scale: f32) -> vec3f {
    var result = vec3f(0.);
    for (var i = 0u; i < params.count; i++) {
        let drop = params.drops[i];
        let d = p - vec2f(drop.x, drop.y + offset);
        let reach = drop.z * scale;
        let inv = 1. / (reach * reach);
        let q = dot(d, d) * inv;
        if q < 1. {
            let k = 1. - q;
            result += vec3f(k * k * k, -6. * k * k * inv * d);
        }
    }
    return result;
}

// The slope the pointer's ripples add at `p`: rings that spread from where it crossed the metal.
fn waves(p: vec2f) -> vec2f {
    var slope = vec2f(0.);
    let k = 2. * PI / params.wavelength;
    for (var i = 0u; i < params.wave_count; i++) {
        let wave = params.waves[i];
        let d = p - wave.xy;
        let distance = max(length(d), 1e-3);
        let x = distance - wave.z;
        let u = x / params.band;
        let envelope = exp(-u * u);
        // Rings close to their centre are taller; they flatten as they spread.
        let height = params.wave_height * params.relief * wave.w
            * params.band / (params.band + wave.z);
        let rise = height * envelope * (k * cos(k * x) - 2. * x / (params.band * params.band) * sin(k * x));
        // Faded out at the centre, where the rings have no direction.
        slope += rise * d / distance * ease(0., params.band, distance);
    }
    return slope;
}

// Hermite easing from `edge0` to `edge1`, either way round.
fn ease(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0., 1.);
    return t * t * (3. - 2. * t);
}

fn ramp_at(t: f32) -> vec3f {
    var stops = array<f32, 9>(0., 0.24, 0.4, 0.45, 0.5, 0.55, 0.64, 0.84, 1.);
    if t <= stops[0] {
        return params.ramp[0].rgb;
    }
    for (var i = 1u; i < 9u; i++) {
        if t <= stops[i] {
            let f = (t - stops[i - 1u]) / (stops[i] - stops[i - 1u]);
            return mix(params.ramp[i - 1u].rgb, params.ramp[i].rgb, f);
        }
    }
    return params.ramp[8].rgb;
}

// A soft-edged rectangle of light facing along `axis`, `width` by `height` in the light's frame.
fn softbox(r: vec3f, axis: vec3f, width: f32, height: f32) -> f32 {
    let across = normalize(cross(axis, vec3f(0., 1., 0.)));
    let up = cross(across, axis);
    if dot(r, axis) <= 0. {
        return 0.;
    }
    let e = vec2f(abs(dot(r, across)), abs(dot(r, up)));
    return ease(width, width * 0.76, e.x) * ease(height, height * 0.62, e.y);
}

// The colour the studio shows along the reflected ray `r`.
fn look(r: vec3f) -> vec3f {
    // Sky to ground by elevation.
    let elevation = asin(clamp(-r.y, -1., 1.));
    var color = ramp_at(0.5 - elevation / PI);
    // A soft fill wall behind the viewer, seen in the faces of the drops.
    color = mix(color, params.front.rgb, ease(0.45, 1., r.z) * 0.6);
    // Behind the metal the studio is dark: every silhouette darkens.
    color = mix(color, params.behind.rgb, ease(-0.15, -0.9, r.z) * 0.85);
    // The key softbox above left, a smaller fill above right, a strip rim behind right.
    color = mix(color, vec3f(1.), softbox(r, SOFTBOX, 0.34, 0.16) * 0.95);
    color = mix(color, vec3f(1.), softbox(r, normalize(FILL_BOX), 0.12, 0.2) * 0.55);
    return mix(color, vec3f(1.), ease(0.93, 0.985, dot(r, STRIP)) * 0.8);
}

// The view, straight into the screen, reflected off the surface with normal `n`.
fn reflect_view(n: vec3f) -> vec3f {
    return vec3f(2. * n.z * n.x, 2. * n.z * n.y, 2. * n.z * n.z - 1.);
}

fn shade(p: vec2f) -> vec3f {
    let shadow = field(p, params.shadow_offset, params.shadow_scale).x;
    let ground = params.background.rgb * (1. - clamp(shadow / 0.45, 0., 1.) * params.shadow);

    let f = field(p, 0., 1.);
    let gradient = max(length(f.yz), 1e-6);
    // Signed distance to the surface over this sample's quarter of the pixel.
    let coverage = clamp((f.x - THRESHOLD) / gradient * 2. + 0.5, 0., 1.);
    if coverage <= 0. {
        return ground;
    }
    let lift = max(f.x - THRESHOLD, 1e-4);
    var slope = f.yz * (params.relief / (2. * sqrt(lift)));
    // Ripples that follow the contours of the liquid and run outwards from its deepest parts,
    // fading out before the edge so the silhouette stays clean.
    let ripple = RIPPLE * params.relief * RIPPLE_FREQUENCY
        * cos(f.x * RIPPLE_FREQUENCY - params.time)
        * ease(0.02, 0.25, lift);
    slope += f.yz * ripple + waves(p) * ease(0.005, 0.12, lift);
    // Each channel bends a little differently.
    let red = look(reflect_view(normalize(vec3f(-slope * (1. + DISPERSION), 1.)))).r;
    let green = look(reflect_view(normalize(vec3f(-slope, 1.)))).g;
    let blue = look(reflect_view(normalize(vec3f(-slope * (1. - DISPERSION), 1.)))).b;
    return mix(ground, vec3f(red, green, blue), coverage);
}

@fragment
fn fs(@builtin(position) position: vec4f) -> @location(0) vec4f {
    // Four samples on a rotated grid.
    let p = position.xy;
    let color = shade(p + vec2f(-0.125, -0.375)) + shade(p + vec2f(0.375, -0.125))
        + shade(p + vec2f(0.125, 0.375)) + shade(p + vec2f(-0.375, 0.125));
    return vec4f(color * 0.25, 1.);
}
