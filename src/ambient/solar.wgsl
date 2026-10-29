// Solar: a low sun in a sky of graded light, after the sunset in bevytris. Wide bands of colour
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

// The crest of ridge `index` (0 farthest) at `x`.
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
