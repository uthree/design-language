// Shoal: two schools of small fish wheel through sunlit shallows in figures of eight, each fish
// flicking its tail and now and then catching the light, under shafts of light from the surface
// and drifting motes. The pointer is a shark: fish near it dart away and turn, and the school
// closes up again behind it. Moving the window leaves the schools behind, the nearer further.

const SCHOOLS: i32 = 2;
const FISH: i32 = 26;
// Tail beats a cycle.
const BEATS: f32 = 100.;

// Where school `s`'s path is at moment `t` of the cycle, and which way it runs there.
fn path(s: f32, t: f32) -> vec4f {
    let width = params.size.x / min(params.size.x, params.size.y);
    let reach = vec2f(0.5 * width * 0.78, 0.14 + 0.03 * s);
    let level = -0.1 + 0.24 * s;
    let a = t + s * PI;
    let b = 2. * t + s * 1.3;
    let at = vec2f(reach.x * sin(a), reach.y * sin(b) + level);
    let heading = vec2f(reach.x * cos(a), 2. * reach.y * cos(b));
    return vec4f(at, heading / max(length(heading), 1e-4));
}

fn scene(p: vec2f) -> vec3f {
    let t = params.time;
    let px = 1. / min(params.size.x, params.size.y);
    // The shallows: lit near the surface, deeper below.
    var color = mix(params.base.rgb, params.deep.rgb, ease(-0.8, 0.7, p.y + params.sway.y * 0.15));
    let water = mix(params.hues[3].rgb, params.hues[2].rgb, ease(-0.5, 0.5, p.x * 0.4 - p.y));
    color = light(color, water, mix(0.45, 0.3, params.dark));
    // Shafts of light slanting down from the surface, swaying.
    let slant = p.x * 3. + p.y * 1.1 + sin(t) * 0.3 + params.sway.x * 2.;
    let shafts = ease(0.42, 0.8, fbm(vec2f(slant, 0.))) * ease(0.6, -0.55, p.y) * 0.22;
    let sunlit = mix(params.hues[0].rgb, vec3f(1.), 0.5);
    color = light(color, sunlit, shafts);
    // Motes drifting up, a whole number of cells a cycle over a pattern that repeats every that
    // many, so they loop.
    let at = (p + params.sway * 0.3) * 34. + vec2f(0., t / TAU * 34.);
    let cell = wrap(floor(at), 34.);
    let spot = fract(at) - 0.5 - (vec2f(hash(cell + 1.3), hash(cell + 7.1)) - 0.5) * 0.6;
    color = light(color, sunlit, step(0.93, hash(cell)) * ease(0.1, 0., length(spot)) * 0.25);

    // Silver over the dark; in a light appearance, darker fish against the light water.
    let silver = mix(
        mix(params.deep.rgb, params.hues[3].rgb, 0.35) * 0.55,
        mix(params.hues[3].rgb, vec3f(1.), 0.6),
        params.dark,
    );
    let belly = mix(silver, mix(vec3f(1.), sunlit, 0.3), mix(0.25, 0.6, params.dark));
    for (var s = 0; s < SCHOOLS; s++) {
        let school = f32(s);
        // The farther school is smaller and paler in the water.
        let scale = mix(0.8, 1.2, school);
        let depth = 0.3 + 0.5 * school;
        let clear = mix(0.55, 1., school);
        for (var f = 0; f < FISH; f++) {
            let n = f32(f) + school * 31.;
            let lag = -hash(vec2f(n, 1.)) * 0.8;
            let side = (hash(vec2f(n, 2.)) - 0.5) * 0.13 * scale;
            let size = (0.028 + 0.014 * hash(vec2f(n, 3.))) * scale;
            let course = path(school, t + lag);
            var heading = course.zw;
            let across = vec2f(-heading.y, heading.x);
            var at = course.xy + across * side + orbit(5. + f32(f % 3), n) * 0.008
                - params.sway * depth;
            // Far from this fish, even once frightened, there is nothing more to do.
            let reach = 2. * size + 0.14;
            if dot(p - at, p - at) > reach * reach {
                continue;
            }
            // It darts away from the pointer and turns aside.
            let away = at - params.pointer;
            let fear = touch(at, 0.2);
            let flight = away / max(length(away), 1e-3);
            at += flight * fear * 0.11;
            heading = normalize(heading + flight * fear * 1.6);
            at += ripples(at).yz * 0.03;

            let l = p - at;
            if dot(l, l) > size * size * 3.5 {
                continue;
            }
            // Along the fish (head forward) and across it.
            let x = dot(l, heading) / size;
            var y = dot(l, vec2f(-heading.y, heading.x)) / size;
            // The body bends with each beat of the tail, more towards the tail.
            let beat = t * BEATS + n * 1.7;
            y -= 0.1 * sin(beat - x * 2.) * (1. - x) * (1. - x) * 0.25;
            let half = 0.26 * sqrt(max(1. - x * x, 0.)) * (1. + 0.25 * x);
            var sdf = select(abs(y) - half, 1., abs(x) > 1.);
            // A forked tail.
            let fan = 0.34 * ease(-0.85, -1.5, x);
            let notch = fan * 0.45 * ease(-1.25, -1.55, x);
            if x < -0.8 && x > -1.55 {
                sdf = min(sdf, max(abs(y) - fan, notch - abs(y)));
            }
            let cover = ease(px / size, -px / size, sdf);
            if cover <= 0. {
                continue;
            }
            // Darker along the back, bright along the belly, as fish are; now and then it turns
            // and catches the light.
            let up = dot(l, vec2f(0., -1.)) / size;
            var body = mix(belly, silver, ease(-0.15, 0.2, up));
            let glint = pow(max(sin(t * 7. + n * 2.3), 0.), 16.);
            body = mix(body, mix(vec3f(1.), sunlit, 0.4), glint * mix(0.25, 0.6, params.dark));
            // An eye.
            let eye = ease(0.08, 0.05, length(vec2f(x - 0.62, y)));
            body = mix(body, params.deep.rgb * 0.5, eye * 0.8);
            color = mix(color, mix(color, body, clear), cover * 0.92);
        }
    }
    return color;
}
