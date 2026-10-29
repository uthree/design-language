// Formation: points of light gather into one figure after another (the edges of a cube, a
// rippling sheet, a knot, a double helix), the figure turning slowly in space and crossing the
// frame as it changes, after the
// formation in bevytris. Nearer points are larger and brighter, and each streaks a little along
// its path. They are placed each frame on the CPU (see `figures.rs`). The pointer scatters the
// points near it and leaves ripples; moving the window swings the figure over a floor of faint
// lines.

// Where the floor meets the dark beyond it.
const HORIZON: f32 = 0.3;

// Where the figure stands across a frame `width` units wide at moment `t`: to one side of the
// card while it holds its shape, crossing to the other each time it turns into the next. The
// camera follows it, so the floor and the light behind it travel too. As in `figures.rs`.
fn stage(t: f32, width: f32) -> f32 {
    return 0.23 * width * clamp(sin(2. * t) * 1.6, -1., 1.);
}

fn scene(p: vec2f) -> vec3f {
    let t = params.time;
    let unit = min(params.size.x, params.size.y);
    let px = 1. / unit;
    let centre = vec2f(stage(t, params.size.x / unit), -0.02);
    // A deep backdrop, lit softly behind the figure.
    var color = mix(params.deep.rgb, params.base.rgb, ease(-0.6, 0.6, p.y) * 0.6);
    let behind = p - centre + params.sway * 0.15;
    color = light(color, params.hues[3].rgb, exp(-dot(behind, behind) / 0.2) * 0.25);
    let glint = behind - vec2f(0.1, 0.08);
    color = light(color, params.hues[1].rgb, exp(-dot(glint, glint) / 0.08) * 0.12);

    // A floor of faint lines in perspective, running towards us a whole number of lines a cycle.
    let below = p.y - HORIZON;
    if below > 0. {
        let x = (p.x - centre.x + params.sway.x * 0.6) / below * 2.;
        let z = 1.2 / below;
        let across = abs(fract(x) - 0.5);
        let along = abs(fract(z - t / TAU * 8.) - 0.5);
        // Each line a pixel or so wide, however far away.
        let wide_x = px / below * 2.;
        let wide_z = px * 1.2 / (below * below);
        let lines = max(ease(wide_x * 1.5, 0., 0.5 - across), ease(wide_z * 1.5, 0., 0.5 - along));
        let fade = ease(0., 0.15, below) * ease(0.6, 0.1, wide_z);
        color = light(color, mix(params.hues[3].rgb, params.hues[2].rgb, 0.5), lines * fade * 0.3);
    }

    // The points, each a short streak from where it was to where it is.
    for (var i = 0u; i < params.counts.x; i++) {
        let path = params.figures[2u * i];
        let point = params.figures[2u * i + 1u];
        let radius = point.x;
        let reach = radius * 6. + px;
        if any(p < min(path.xy, path.zw) - reach) || any(p > max(path.xy, path.zw) + reach) {
            continue;
        }
        let d = segment(p, path.xy, path.zw);
        // Brightest at its head.
        let tail = path.xy - path.zw;
        let head = clamp(dot(p - path.zw, tail) / max(dot(tail, tail), 1e-8), 0., 1.);
        let core = ease(radius + px, radius * 0.3, d) * (0.35 + 0.65 * head);
        let halo = exp(-d / (radius * 2.5)) * 0.35;
        let hue = params.hues[u32(point.z)].rgb;
        let ink = mix(mix(hue, params.deep.rgb * 0.5, 0.35), mix(hue, vec3f(1.), core * 0.6), params.dark);
        color = light(color, ink, (core + halo) * point.y * 0.8);
    }
    // Light caught in the ripples.
    color = light(color, vec3f(1.), max(ripples(p).x, 0.) * 0.08);
    return color;
}
