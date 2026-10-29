// Aurora: three curtains of light hang over a night sky, folding slowly as they drift, each lit
// in fine rays that brighten towards its lower hem. Moving the window swings the curtains, the
// nearest furthest. Still water below mirrors it all. Among the stars are a few brighter ones the
// pointer wakes and joins into constellations, and lines the person draws between them, and
// shooting stars (see `stars.rs`); the curtains themselves leave the pointer alone.

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

// The sky and its curtains at `p`.
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

// The brighter stars at `q` (in the sky's own frame), the lines between them and the shooting
// stars, over `color`.
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
