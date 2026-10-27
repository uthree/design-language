// Dunes: sheets of silk lie in layers, one in front of another, their crests rolling slowly.
// Each is lit from above, with a sheen along its slopes and a soft shadow where it falls on the
// one behind. The pointer lifts the silk under it; its ripples run along the crests; moving the
// window slides the layers apart, the nearest furthest.

const LAYERS: i32 = 5;

// The crest of layer `index` at `x`, and where its sheet would be without the pointer.
fn crest(x: f32, index: f32) -> f32 {
    let t = params.time;
    let level = -0.26 + 0.16 * index;
    var y = level
        + 0.09 * sin(x * (1.1 + 0.25 * index) + t + index * 1.9)
        + 0.045 * sin(x * (2.6 - 0.2 * index) - 2. * t + index * 3.7)
        + 0.015 * sin(x * 6.1 + 3. * t + index * 5.3);
    // The pointer lifts the sheet nearest it.
    let d = vec2f(x, y) - params.pointer;
    let lift = params.presence * exp(-d.x * d.x / 0.06) * exp(-d.y * d.y / 0.02);
    y -= lift * 0.07 + params.velocity.y * lift * 0.01;
    y += ripples(vec2f(x, y)).x * 0.006;
    return y;
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
