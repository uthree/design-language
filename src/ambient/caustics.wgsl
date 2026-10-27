// Caustics: light through moving water, playing in a net of bright lines over a deep floor, with
// shafts of light slanting down from the surface. Each colour bends a little differently, so the
// lines have thin coloured fringes. The pointer is a lens on the water, gathering the light under
// it; its ripples bend the net as they spread; moving the window sloshes the water.

// Speeds of the water's layers, in whole turns a cycle.
const SPEEDS: array<f32, 5> = array<f32, 5>(-2., 1., -1., 2., 1.);

// The net of light at `p`: 0 to about 1.
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
