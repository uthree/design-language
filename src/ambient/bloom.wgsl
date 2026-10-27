// Bloom: a few great clouds of colour drift and melt into one another, their edges stirred by
// slow eddies, under a faint frosted sheen. The pointer stirs the colour round itself and drags
// it along; its ripples spread through it; moving the window leaves the clouds behind, each by
// its own amount, so the colour sloshes back.

// Each cloud: where its orbit is centred, how far it swings and how fast (whole turns a cycle),
// and its size.
const CLOUDS: array<vec4f, 5> = array<vec4f, 5>(
    vec4f(-0.55, -0.25, 0.22, 0.55),
    vec4f(0.5, -0.3, 0.18, 0.5),
    vec4f(0.1, 0.35, 0.25, 0.6),
    vec4f(-0.2, 0.05, 0.3, 0.42),
    vec4f(0.75, 0.3, 0.2, 0.45),
);

fn scene(p: vec2f) -> vec3f {
    let t = params.time;
    // The pointer stirs the colour round itself and drags it along.
    let near = touch(p, 0.3);
    let d = p - params.pointer;
    let angle = near * (0.9 + clamp(length(params.velocity), 0., 2.) * 0.5);
    var q = params.pointer + vec2f(cos(angle) * d.x - sin(angle) * d.y, sin(angle) * d.x + cos(angle) * d.y);
    q -= params.velocity * near * 0.06;
    // The ripples push the colour outwards as they pass.
    let wave = ripples(q);
    q -= wave.yz * 0.025;
    // Slow eddies.
    let warp = vec2f(
        fbm(q * 1.3 + orbit(1., 0.) * 0.5),
        fbm(q * 1.3 + vec2f(5.2, 1.3) + orbit(1., 2.) * 0.5),
    );
    q += (warp - 0.5) * 0.6;

    // The clouds, each a soft field of one hue.
    var clouds = CLOUDS;
    var sum = vec3f(0.);
    var weight = 0.;
    for (var i = 0; i < 5; i++) {
        let cloud = clouds[i];
        let lag = 0.6 + 0.25 * f32(i % 3);
        let centre = cloud.xy + cloud.z * orbit(f32(1 + i % 2), f32(i) * 1.7) * vec2f(1., 0.7)
            - params.sway * lag;
        let r = q - centre;
        let w = exp(-dot(r, r) / (cloud.w * cloud.w));
        sum += params.hues[i % 4].rgb * w;
        weight += w;
    }
    let hue = sum / max(weight, 1e-4);
    let amount = ease(0., 1.3, weight);
    var color = mix(params.deep.rgb, params.base.rgb, ease(-0.9, 0.9, p.y - p.x * 0.3));
    color = light(color, hue, amount * mix(0.8, 0.85, params.dark));

    // A frosted sheen: faint bright folds where the eddies run fast.
    let fold = fbm(q * 2.2 - orbit(2., 0.5) * 0.3);
    let sheen = ease(0.07, 0., abs(fold - 0.5)) * 0.05 + near * 0.06;
    color = light(color, vec3f(1.), sheen * (0.4 + 0.6 * params.dark));
    // Light caught in the ripples.
    color = light(color, vec3f(1.), max(wave.x, 0.) * 0.15);
    return color;
}
