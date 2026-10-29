// Garden: a row of plants grows over the cycle, after the garden in bevytris: stems wander up and
// branch, leaves unfold along them and every tip blooms; then the petals let go and drift down,
// the plants fade, and the garden grows again. They sway in the wind over a dusk of soft,
// out-of-focus lights, with motes of pollen rising. The plants are laid out each frame on the CPU
// (see `figures.rs`), where they lean away from the pointer; its ripples catch the light, and
// moving the window leaves the plants behind, each by its own amount.

fn scene(p: vec2f) -> vec3f {
    let t = params.time;
    let phase = fract(t / TAU);
    let unit = min(params.size.x, params.size.y);
    let px = 1. / unit;
    let height = params.size.y / unit;

    // Dusk over the ground, warmest low down.
    var color = mix(params.deep.rgb, params.base.rgb, ease(-0.6, 0.45, p.y));
    let low = p.y - 0.4 * height;
    color = light(color, params.hues[0].rgb, exp(-low * low / 0.05) * 0.22);
    // Soft out-of-focus lights drifting behind.
    let drift = p * 5. + params.sway * 0.5 + orbit(1., 0.) * 0.4;
    let cell = floor(drift);
    let seed = hash(cell + 7.7);
    let spot = fract(drift) - 0.5 - (vec2f(hash(cell + 1.9), hash(cell + 4.3)) - 0.5) * 0.4;
    let size = 0.18 + 0.14 * hash(cell + 9.1);
    let bokeh = step(0.6, seed) * ease(size, size - 0.05, length(spot)) * (0.6 + 0.4 * ease(size - 0.08, size, length(spot)));
    color = light(color, params.hues[u32(seed * 40.) % 4u].rgb, bokeh * mix(0.12, 0.1, params.dark));

    // The ground along the bottom.
    let ground = 0.5 * height - 0.02 + 0.006 * sin(p.x * 9.) + 0.004 * sin(p.x * 23. + 1.);
    let soil = mix(
        mix(params.base.rgb, params.hues[3].rgb, 0.35) * 0.9,
        mix(params.deep.rgb, params.hues[3].rgb, 0.15) * 0.6,
        params.dark,
    );
    color = mix(color, soil, ease(-px, px, p.y - ground + params.sway.y * 0.1));

    let stem_ink = mix(
        mix(params.deep.rgb, params.hues[3].rgb, 0.4) * 0.55,
        mix(params.hues[3].rgb, params.hues[2].rgb, 0.5),
        params.dark,
    );
    let segments = params.counts.x;
    let leaves = params.counts.y;
    let blooms = params.counts.z;
    // The stems, each segment tapering from one end to the other.
    for (var i = 0u; i < segments; i++) {
        let line = params.figures[2u * i];
        let shape = params.figures[2u * i + 1u];
        let reach = shape.x * 4. + px;
        if any(p < min(line.xy, line.zw) - reach) || any(p > max(line.xy, line.zw) + reach) {
            continue;
        }
        let ab = line.zw - line.xy;
        let s = clamp(dot(p - line.xy, ab) / max(dot(ab, ab), 1e-8), 0., 1.);
        let d = length(p - line.xy - ab * s);
        let w = mix(shape.x, shape.y, s);
        let cover = ease(w + px * 0.75, w - px * 0.75, d);
        let stem = mix(stem_ink, params.hues[u32(shape.z)].rgb, 0.2);
        color = mix(color, stem, cover * shape.w);
        color = light(color, stem, exp(-d / (w * 3.)) * 0.12 * params.dark * shape.w);
    }
    // The leaves: each a narrow lens from its node, with a paler rib.
    for (var i = segments; i < segments + leaves; i++) {
        let leaf = params.figures[2u * i];
        let shape = params.figures[2u * i + 1u];
        let l = p - leaf.xy;
        if dot(l, l) > leaf.w * leaf.w * 1.2 {
            continue;
        }
        let along = vec2f(cos(leaf.z), sin(leaf.z));
        let u = dot(l, along);
        let v = dot(l, vec2f(-along.y, along.x));
        let half = shape.x * sin(PI * clamp(u / max(leaf.w, 1e-4), 0., 1.));
        let sdf = select(abs(v) - half, 1., u < 0. || u > leaf.w);
        let cover = ease(px, -px, sdf);
        let green = mix(stem_ink, params.hues[u32(shape.y)].rgb, 0.3);
        let rib = ease(px, 0., abs(v)) * 0.35;
        color = mix(color, mix(green, vec3f(1.), rib * params.dark), cover * shape.z);
    }
    // The blooms: petals round a bright heart, opening and closing with the season.
    for (var i = segments + leaves; i < segments + leaves + blooms; i++) {
        let bloom = params.figures[2u * i];
        let shape = params.figures[2u * i + 1u];
        let l = p - bloom.xy;
        let r = length(l);
        let size = bloom.z;
        if r > size * 2. + px {
            continue;
        }
        let open = bloom.w;
        let angle = atan2(l.y, l.x) + shape.y * 0.1;
        let edge = size * (0.5 + 0.5 * pow(abs(cos(angle * 2.5)), 0.6)) * (0.4 + 0.6 * open);
        let cover = ease(edge + px, edge - px, r);
        let hue = params.hues[u32(shape.x)].rgb;
        let petal = mix(mix(hue, params.deep.rgb * 0.5, 0.15), hue, params.dark);
        let heart = ease(size * 0.22 + px, size * 0.22 - px, r);
        var flower = mix(petal, mix(params.hues[0].rgb, vec3f(1.), 0.5), heart);
        // Paler towards the petals' tips.
        flower = mix(flower, vec3f(1.), ease(edge * 0.4, edge, r) * 0.25 * params.dark);
        color = mix(color, flower, cover * shape.z);
        color = light(color, hue, exp(-r / size) * 0.3 * open * params.dark * shape.z);
    }

    // Petals let go and drift down as the garden withers.
    let falling = ease(0.74, 0.84, phase) * (1. - ease(0.93, 0.99, phase));
    if falling > 0. {
        let at = (p + vec2f(0.05 * sin(t * 6. + p.y * 8.), -phase * 1.2)) * 9.;
        let cell = floor(at);
        let seed = hash(cell + 3.3);
        let spin = seed * TAU + t * 5.;
        let local = rotate(fract(at) - 0.5 - (vec2f(hash(cell + 2.1), hash(cell + 6.2)) - 0.5) * 0.5, vec2f(cos(spin), sin(spin)));
        let petal = ease(0.12, 0.08, length(local * vec2f(1., 2.2)));
        color = mix(color, params.hues[u32(seed * 40.) % 4u].rgb, step(0.7, seed) * petal * falling * 0.8);
    }
    // Motes of pollen rising, a whole number of cells a cycle, flickering.
    let rise = p * 14. + vec2f(0., t / TAU * 28.);
    let mote_cell = wrap(floor(rise), 28.);
    let mote = fract(rise) - 0.5 - (vec2f(hash(mote_cell + 5.5), hash(mote_cell + 8.8)) - 0.5) * 0.6;
    let flicker = 0.5 + 0.5 * sin(t * 9. + hash(mote_cell) * 60.);
    let motes = step(0.93, hash(mote_cell)) * ease(0.09, 0., length(mote)) * flicker * mix(0.25, 0.7, params.dark);
    color = light(color, mix(params.hues[0].rgb, vec3f(1.), 0.4), motes);
    // Light caught in the ripples.
    color = light(color, vec3f(1.), max(ripples(p).x, 0.) * 0.08);
    return color;
}
