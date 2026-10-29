// Jellyfish: a few jellyfish drift up through deep water, their bells pulsing slowly and their
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
