// Aurora: three curtains of light hang over a night sky, folding slowly as they drift, each lit
// in fine rays that brighten towards its lower hem. The pointer parts the curtains and lights
// them where it passes; its ripples run along the hems; moving the window swings the curtains,
// the nearest furthest. Still water below mirrors it all.

fn curtain(p: vec2f, index: f32) -> vec4f {
    let t = params.time;
    // Nearer curtains are lower, larger and swing further.
    let depth = 0.55 + 0.25 * index;
    var q = p + params.sway * depth;
    // The pointer parts the curtains and drags them along.
    let reach = 0.28;
    let near = touch(q, reach);
    q = q - (q - params.pointer) * near * 0.35 - params.velocity * near * 0.05;

    // The hem: a slow fold with a faster ripple on it, and the pointer's ripples running along.
    let x = q.x / depth;
    var hem = 0.02 + 0.07 * index
        + 0.06 * sin(x * 1.6 + t + index * 2.1)
        + 0.035 * sin(x * 3.7 - 2. * t + index * 4.3)
        + 0.015 * sin(x * 7.9 + 3. * t + index);
    hem += ripples(q).x * 0.05;
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
    let strength = body * rays * fold * (1. + near * 0.9);
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

    for (var i = 0; i < 3; i++) {
        let c = curtain(p, f32(i));
        color = light(color, c.rgb, c.a);
    }
    return color;
}

// Where the sky meets still water, which mirrors it.
const LAKE: f32 = 0.24;

fn scene(p: vec2f) -> vec3f {
    var color: vec3f;
    if p.y < LAKE {
        color = sky(p);
    } else {
        // The water mirrors the sky, darker, broken by slow ripples and the pointer's.
        let depth = p.y - LAKE;
        let wobble = 0.004 * sin(depth * 140. / (0.2 + depth) + params.time * 4.)
            + ripples(p).x * 0.01;
        let mirrored = sky(vec2f(p.x + wobble, LAKE - depth * 1.15));
        let water = mix(params.deep.rgb, params.base.rgb, 0.5);
        color = mix(water, mirrored, mix(0.7, 0.6, params.dark) * exp(-depth * 1.8));
        // A thin line of light where they meet.
        color = light(color, params.hues[3].rgb, exp(-depth / 0.004) * 0.25);
    }
    // A faint glow where the pointer is.
    return light(color, params.hues[3].rgb, touch(p, 0.2) * 0.1);
}
