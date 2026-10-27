// A 3-D plot's contents: the surface, its curves and its points, seen through the plot's
// orthographic camera. The axes, walls and labels stay with gpui, behind and in front.
//
// Positions are in the plot's box: x and y from −1 to 1, z from −0.75 to 0.75. Screen
// positions are logical pixels from the plot's top left, x right and y down. Colours are sRGB
// with straight alpha; the frame is written premultiplied and straightened after read-back.

struct Params {
    // cos and sin of the azimuth, cos and sin of the elevation.
    view: vec4f,
    // Pixels per box unit, the view's offset (right, up), and device pixels per logical pixel.
    frame: vec4f,
    // The box centre on screen, and the frame's size, in logical pixels.
    screen: vec4f,
    // The light's direction in the box, and the surface's shading: 0 faceted, 1 smooth, 2 mesh.
    light: vec4f,
    // Facet edges, and the face of a mesh.
    edge: vec4f,
    sunken: vec4f,
}

@group(0) @binding(0) var<uniform> params: Params;

// A box point on screen, with its depth (larger is farther).
fn project(p: vec3f) -> vec3f {
    let c = params.view;
    let right = p.x * c.x + p.y * c.y;
    let away = -p.x * c.y + p.y * c.x;
    let up = p.z * c.z + away * c.w;
    let depth = away * c.z - p.z * c.w;
    let scale = params.frame.x;
    return vec3f(
        params.screen.x + (right - params.frame.y) * scale,
        params.screen.y - (up - params.frame.z) * scale,
        depth,
    );
}

fn clip(screen: vec3f) -> vec4f {
    let size = params.screen.zw;
    // The box's depth stays within ±1.6, so this keeps it well inside 0 to 1.
    return vec4f(screen.x / size.x * 2. - 1., 1. - screen.y / size.y * 2., 0.5 + screen.z * 0.2, 1.);
}

// The direction towards the viewer, in the box.
fn toward_viewer() -> vec3f {
    let c = params.view;
    return vec3f(c.y * c.z, -c.x * c.z, c.w);
}

fn premultiply(color: vec4f) -> vec4f {
    return vec4f(color.rgb * color.a, color.a);
}

// --- surface --- //

struct SurfaceOut {
    @builtin(position) position: vec4f,
    @location(0) normal: vec3f,
    @location(1) color: vec4f,
    // The point's column and row in the grid, so cell edges fall on whole numbers.
    @location(2) grid: vec2f,
}

@vertex
fn vs_surface(
    @location(0) at: vec3f,
    @location(1) normal: vec3f,
    @location(2) color: vec4f,
    @location(3) grid: vec2f,
) -> SurfaceOut {
    var out: SurfaceOut;
    out.position = clip(project(at));
    out.normal = normal;
    out.color = color;
    out.grid = grid;
    return out;
}

@fragment
fn fs_surface(input: SurfaceOut) -> @location(0) vec4f {
    // How far the pixel is from the nearest cell edge, in device pixels.
    let f = fract(input.grid);
    let across = min(f, 1. - f) / max(fwidth(input.grid), vec2f(1e-5));
    let distance = min(across.x, across.y);
    let density = params.frame.w;

    let view = toward_viewer();
    var n = normalize(input.normal);
    if dot(n, view) < 0. {
        n = -n;
    }
    let light = normalize(params.light.xyz);
    // As on the CPU: darker as the facet turns from the light, either side lit.
    let lambert = abs(dot(n, light));
    var face = mix(input.color.rgb, vec3f(0.), (1. - lambert) * 0.35);
    // And a soft sheen where the facet mirrors the light towards the viewer.
    let lit = select(-light, light, dot(n, light) > 0.);
    let sheen = pow(max(dot(n, normalize(lit + view)), 0.), 36.);
    face += vec3f(sheen * 0.16);

    let shading = u32(params.light.w);
    if shading == 0u {
        let coverage = clamp(0.375 * density + 0.5 - distance, 0., 1.);
        face = mix(face, params.edge.rgb, params.edge.a * coverage);
    } else if shading == 2u {
        let coverage = clamp(0.5 * density + 0.5 - distance, 0., 1.);
        face = mix(params.sunken.rgb, input.color.rgb, coverage);
    }
    return vec4f(face, 1.);
}

// --- curves --- //

struct LineOut {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
}

// One segment as a quad `width` logical pixels across. Segments meet end to end without
// overlapping, so a translucent curve is even along its length and brighter only where it
// crosses itself, as it is drawn on the CPU.
@vertex
fn vs_line(
    @builtin(vertex_index) index: u32,
    @location(0) a: vec4f,
    @location(1) b: vec4f,
    @location(2) color_a: vec4f,
    @location(3) color_b: vec4f,
) -> LineOut {
    var corners = array(
        vec2f(0., -1.), vec2f(1., -1.), vec2f(0., 1.),
        vec2f(0., 1.), vec2f(1., -1.), vec2f(1., 1.),
    );
    let corner = corners[index];
    let start = project(a.xyz);
    let end = project(b.xyz);
    let delta = end.xy - start.xy;
    let length = length(delta);
    let along = select(vec2f(1., 0.), delta / length, length > 1e-4);
    let side = vec2f(-along.y, along.x);
    let half = a.w * 0.5;
    let base = mix(start, end, corner.x);
    let at = base.xy + side * corner.y * half;
    var out: LineOut;
    out.position = clip(vec3f(at, base.z));
    out.color = mix(color_a, color_b, corner.x);
    return out;
}

@fragment
fn fs_line(input: LineOut) -> @location(0) vec4f {
    return premultiply(input.color);
}

// --- points --- //

struct PointOut {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
    // Where in the point the pixel is, in radii from its centre.
    @location(1) local: vec2f,
    // The radius in device pixels.
    @location(2) @interpolate(flat) radius: f32,
}

// How far a particle's glow reaches, in radii.
const GLOW_REACH: f32 = 4.;

fn point_vertex(index: u32, at: vec4f, color: vec4f, reach: f32) -> PointOut {
    var corners = array(
        vec2f(-1., -1.), vec2f(1., -1.), vec2f(-1., 1.),
        vec2f(-1., 1.), vec2f(1., -1.), vec2f(1., 1.),
    );
    let corner = corners[index];
    let centre = project(at.xyz);
    let radius = at.w;
    var out: PointOut;
    out.position = clip(vec3f(centre.xy + corner * reach, centre.z));
    out.color = color;
    out.local = corner * reach / radius;
    out.radius = radius * params.frame.w;
    return out;
}

@vertex
fn vs_marker(@builtin(vertex_index) index: u32, @location(0) at: vec4f, @location(1) color: vec4f) -> PointOut {
    // A device pixel beyond the edge, for its antialiasing.
    return point_vertex(index, at, color, at.w + 1. / params.frame.w);
}

// A marker: a small bead, lit from above left, with a soft edge one device pixel wide.
@fragment
fn fs_marker(input: PointOut) -> @location(0) vec4f {
    let r = length(input.local);
    let coverage = clamp((1. - r) * input.radius + 0.5, 0., 1.);
    let rise = sqrt(max(1. - r * r, 0.));
    let normal = vec3f(input.local, rise);
    let highlight = pow(max(dot(normal, normalize(vec3f(-0.45, -0.55, 0.7))), 0.), 12.);
    let rgb = input.color.rgb * (0.8 + 0.2 * rise) + vec3f(highlight * 0.28);
    return premultiply(vec4f(min(rgb, vec3f(1.)), input.color.a * coverage));
}

@vertex
fn vs_particle(@builtin(vertex_index) index: u32, @location(0) at: vec4f, @location(1) color: vec4f) -> PointOut {
    return point_vertex(index, at, color, at.w * GLOW_REACH);
}

// A particle: a bright core in a wide, faint glow.
@fragment
fn fs_particle(input: PointOut) -> @location(0) vec4f {
    let q = dot(input.local, input.local);
    let glow = exp(-2. * q) + 0.22 * exp(-0.35 * q);
    return premultiply(vec4f(input.color.rgb, min(input.color.a * glow, 1.)));
}
