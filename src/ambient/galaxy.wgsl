// Galaxy: a spiral galaxy turns slowly in deep space, after the galaxy in bevytris: a bright core,
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

// The stars at `q`, `cells` to a unit: one in a few cells, of `share` of them, twinkling.
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
