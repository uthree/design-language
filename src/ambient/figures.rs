//! The shapes the Formation and Garden scenes draw, laid out on the CPU each frame and handed to
//! their shaders, as Aurora's stars are: a few hundred moving pieces are cheap to place once a
//! frame and far too many for each pixel to work out for itself.
//!
//! - Formation: points of light that gather into one figure after another (the edges of a cube, a
//!   rippling sheet, a knot, a double helix), the figure turning in space and crossing from one
//!   side of the frame to the other as it changes, after the formation behind the boards of
//!   bevytris. Each point carries where it was a moment before, so it is
//!   drawn as a short streak along its path. The pointer scatters the points near it.
//! - Garden: a row of plants that grow, branch, leaf and bloom over the cycle, then let their
//!   petals go and fade, swaying in the wind, after bevytris's garden. They lean away from the
//!   pointer.
//!
//! Positions are in the shaders' units, from the frame's centre, y down. Every piece is two
//! vectors; what they hold is up to its scene (see `formation.wgsl` and `garden.wgsl`).

use std::f32::consts::{PI, TAU};

use super::Ambience;
use super::sim::Response;

/// The most pieces a frame can hold.
pub(super) const FIGURES: usize = 256;

/// The pieces of one frame, and how many of each kind there are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Figures {
    pub items: [[[f32; 4]; 2]; FIGURES],
    /// Formation: the points. Garden: the stem segments, the leaves and the blooms, in that order.
    pub counts: [u32; 4],
}

impl Default for Figures {
    fn default() -> Self {
        Self {
            items: [[[0.; 4]; 2]; FIGURES],
            counts: [0; 4],
        }
    }
}

impl Figures {
    fn push(&mut self, kind: usize, item: [[f32; 4]; 2]) -> bool {
        let used: u32 = self.counts.iter().sum();
        if used as usize == FIGURES {
            return false;
        }
        self.items[used as usize] = item;
        self.counts[kind] += 1;
        true
    }
}

/// The pieces `ambience` draws at moment `time` of its cycle (in radians), in a box of `size`
/// (in pixels), answering `response`; nothing for the scenes that draw none.
pub(super) fn figures(
    ambience: Ambience,
    time: f32,
    size: [f32; 2],
    response: &Response,
) -> Figures {
    let unit = size[0].min(size[1]).max(1.);
    let frame = [size[0] / unit, size[1] / unit];
    match ambience {
        Ambience::Formation => formation(time, frame, response),
        Ambience::Garden => garden(time, frame, response),
        _ => Figures::default(),
    }
}

// Formation

/// Points in a figure.
const POINTS: usize = 256;
/// Figures a cycle, each held a quarter of it.
const SHAPES: usize = 4;
/// The share of a figure's quarter spent turning into it from the one before.
const MORPH: f32 = 0.22;
/// How far back along its path a point's streak reaches, in radians of the cycle.
const STREAK: f32 = 0.012;

/// Point `i` of figure `shape`, about the origin with a radius of about 1, at moment `t`.
fn figure(shape: usize, i: usize, t: f32) -> [f32; 3] {
    let f = i as f32 / POINTS as f32;
    match shape % SHAPES {
        // The edges of a cube.
        0 => {
            const CORNERS: [[f32; 3]; 8] = [
                [-1., -1., -1.],
                [1., -1., -1.],
                [1., 1., -1.],
                [-1., 1., -1.],
                [-1., -1., 1.],
                [1., -1., 1.],
                [1., 1., 1.],
                [-1., 1., 1.],
            ];
            const EDGES: [(usize, usize); 12] = [
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 0),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 4),
                (0, 4),
                (1, 5),
                (2, 6),
                (3, 7),
            ];
            // Spread evenly over all twelve edges, a share of a point each where they don't divide.
            let along = (i * EDGES.len()) as f32 / POINTS as f32;
            let (a, b) = EDGES[along as usize];
            let k = along.fract();
            let [a, b] = [CORNERS[a], CORNERS[b]];
            [0, 1, 2].map(|axis| (a[axis] + (b[axis] - a[axis]) * k) * 0.72)
        }
        // A sheet with ripples running across it.
        1 => {
            let x = (i % 16) as f32 / 15. * 2. - 1.;
            let z = (i / 16) as f32 / 15. * 2. - 1.;
            let y = 0.35 * (x * 3.4 + t * 6.).sin() + 0.25 * (z * 3. - t * 4.).cos();
            [x, y * 0.9, z]
        }
        // A knot, slowly turning over.
        2 => {
            let s = f * TAU;
            [
                (3. * s + t * 2.).sin() * 0.85,
                (4. * s).sin() * 0.68,
                (5. * s).cos() * 0.85,
            ]
        }
        // A double helix with rungs between its strands.
        _ => {
            let k = (f * 2.) % 1.;
            let turn = k * TAU * 2.6;
            if i % 9 == 8 {
                let across = (i as f32 * 0.618).fract() * 2. - 1.;
                [
                    turn.sin() * across * 0.55,
                    k * 2. - 1.,
                    turn.cos() * across * 0.55,
                ]
            } else {
                let a = turn + (i % 2) as f32 * PI;
                [a.sin() * 0.55, k * 2. - 1., a.cos() * 0.55]
            }
        }
    }
}

/// Where the figure stands across the frame at moment `t`, `width` units wide: to one side of
/// the middle, where the card is, while it holds its shape, crossing to the other side each time
/// it turns into the next figure. As `stage` in `formation.wgsl`.
pub(super) fn stage(t: f32, width: f32) -> f32 {
    0.23 * width * ((2. * t).sin() * 1.6).clamp(-1., 1.)
}

/// Point `i` at moment `t` of the cycle: where it shows on the screen (before the pointer moves
/// it) and how near it is (the perspective scale).
fn formation_point(i: usize, t: f32, frame: [f32; 2], response: &Response) -> ([f32; 2], f32) {
    let quarter = (t / TAU).rem_euclid(1.) * SHAPES as f32;
    let shape = quarter as usize;
    let morph = (quarter.fract() / MORPH).min(1.);
    let morph = morph * morph * (3. - 2. * morph);
    let a = figure(shape + SHAPES - 1, i, t);
    let b = figure(shape, i, t);
    let p = [0, 1, 2].map(|axis| a[axis] + (b[axis] - a[axis]) * morph);
    // Turned about the vertical three times a cycle, tipped towards us and back once.
    let (sy, cy) = (t * 3.).sin_cos();
    let (sx, cx) = (0.45 + 0.25 * t.sin()).sin_cos();
    let p = [p[0] * cy + p[2] * sy, p[1], -p[0] * sy + p[2] * cy];
    let p = [p[0], p[1] * cx - p[2] * sx, p[1] * sx + p[2] * cx];
    let near = 2.6 / (2.6 + p[2]);
    let centre = [stage(t, frame[0]), -0.02];
    let depth = 0.3 + 0.4 * near;
    (
        [
            centre[0] + p[0] * 0.24 * near - response.sway[0] * depth,
            centre[1] - p[1] * 0.24 * near - response.sway[1] * depth,
        ],
        near,
    )
}

/// The pointer scatters the points near it.
fn scatter(at: [f32; 2], response: &Response) -> ([f32; 2], f32) {
    let d = [at[0] - response.pointer[0], at[1] - response.pointer[1]];
    let distance = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-3);
    let near = response.presence * (-distance * distance / (0.14 * 0.14)).exp();
    let push = near * 0.07 / distance;
    ([at[0] + d[0] * push, at[1] + d[1] * push], near)
}

fn formation(t: f32, frame: [f32; 2], response: &Response) -> Figures {
    let mut figures = Figures::default();
    for i in 0..POINTS {
        let (at, near) = formation_point(i, t, frame, response);
        let (before, _) = formation_point(i, t - STREAK, frame, response);
        let (at, stirred) = scatter(at, response);
        let (before, _) = scatter(before, response);
        let radius = (0.0035 + 0.003 * near) * (1. + stirred * 0.6);
        let bright = ((near - 0.55) * 1.6).clamp(0.15, 1.) * (1. + stirred);
        figures.push(
            0,
            [
                [at[0], at[1], before[0], before[1]],
                [radius, bright, (i % 4) as f32, 0.],
            ],
        );
    }
    figures
}

// Garden

/// Plants in the row.
const PLANTS: usize = 6;
/// The most stem segments, leaves and blooms; together they fill `FIGURES`.
const SEGMENTS: usize = 170;
const LEAVES: usize = 60;
const BLOOMS: usize = 26;
/// The length of a stem's step, in units.
const STEP: f32 = 0.045;

/// A number from 0 to 1, the same every time for `a`, `b` and `c`.
fn hash(a: usize, b: usize, c: usize) -> f32 {
    let mut x = (a as u32).wrapping_mul(0x9e37_79b9)
        ^ (b as u32).wrapping_mul(0x85eb_ca6b)
        ^ (c as u32).wrapping_mul(0xc2b2_ae35);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x & 0xffff) as f32 / 65535.
}

/// One point along a stem, relative to its plant's root: when in the cycle's growth it is
/// reached (0 to 1), and how far out along the plant it is (0 at the root, 1 at the tips).
#[derive(Clone, Copy)]
struct Node {
    at: [f32; 2],
    birth: f32,
    reach: f32,
}

/// A plant's stems as runs of nodes, its leaves (the node they grow from, their angle and side)
/// and its blooms (the tips).
#[derive(Default)]
struct Plant {
    stems: Vec<Vec<Node>>,
    leaves: Vec<(Node, f32)>,
    blooms: Vec<Node>,
}

/// Plant `index`, `height` units tall at most: a stem that wanders up, branching now and then,
/// leafing along the branches and blooming at every tip.
fn plant(index: usize, height: f32) -> Plant {
    let mut plant = Plant::default();
    let length = height * (0.6 + 0.4 * hash(index, 0, 0));
    let mut draws = 0;
    let mut next = || {
        draws += 1;
        hash(index, draws, 1)
    };
    let start = Node {
        at: [0., 0.],
        birth: 0.,
        reach: 0.,
    };
    let mut stack = vec![(start, -PI / 2. + (next() - 0.5) * 0.2, length, 0u32, 0f32)];
    while let Some((from, mut angle, long, level, travelled)) = stack.pop() {
        let mut run = vec![from];
        let mut at = from.at;
        let steps = ((long / STEP) as usize).max(2);
        for step in 0..steps {
            at = [at[0] + angle.cos() * STEP, at[1] + angle.sin() * STEP];
            angle += (next() - 0.5) * 0.2;
            let distance = travelled + (step + 1) as f32 * STEP;
            let reach = (distance / (length * 1.5)).min(1.);
            let node = Node {
                at,
                birth: reach * 0.9,
                reach,
            };
            run.push(node);
            if level >= 1 && next() < 0.3 {
                let side = if next() < 0.5 { 1. } else { -1. };
                plant.leaves.push((node, angle + side * 1.1));
            }
            if level < 2 && long > 3. * STEP && next() < 0.16 {
                let side = if next() < 0.5 { 1. } else { -1. };
                stack.push((
                    node,
                    angle + side * (0.35 + 0.35 * next()),
                    long * 0.55,
                    level + 1,
                    distance,
                ));
            }
        }
        let tip = run[run.len() - 1];
        plant.blooms.push(Node {
            birth: ((travelled + long) / (length * 1.5) * 0.92).min(0.98),
            ..tip
        });
        plant.stems.push(run);
    }
    plant
}

/// How far the garden has grown at moment `t` (0 to 1), and how much of it is left (1 until it
/// withers, then down to 0), and how open its blooms are.
fn season(t: f32) -> (f32, f32, f32) {
    let phase = (t / TAU).rem_euclid(1.);
    let ease = |x: f32| {
        let x = x.clamp(0., 1.);
        x * x * (3. - 2. * x)
    };
    let grown = ease(phase / 0.5);
    let left = 1. - ease((phase - 0.8) / 0.15);
    let open = ease((phase - 0.4) / 0.15) * (1. - ease((phase - 0.78) / 0.1));
    (grown, left, open)
}

fn garden(t: f32, frame: [f32; 2], response: &Response) -> Figures {
    let mut figures = Figures::default();
    let (grown, left, open) = season(t);
    if left <= 0. {
        return figures;
    }
    let ground = 0.5 * frame[1];
    let height = frame[1] * 0.62;
    let mut segments = Vec::new();
    let mut leaves = Vec::new();
    let mut blooms = Vec::new();
    for index in 0..PLANTS {
        let plant = plant(index, height);
        // Spread along the row, with room for the card in the middle.
        let slot = (index as f32 + 0.5) / PLANTS as f32;
        let x = (slot - 0.5) * frame[0] * 0.92 + (hash(index, 0, 2) - 0.5) * 0.08;
        let depth = 0.35 + 0.3 * hash(index, 0, 3);
        let root = [
            x - response.sway[0] * depth,
            ground - response.sway[1] * depth * 0.3,
        ];
        // The wind, and a lean away from the pointer, more at the tips.
        let gust = (t * 3. + x * 2.).sin() * 0.05 + (t * 7. + index as f32).sin() * 0.015;
        let away = root[0] - response.pointer[0];
        let rise = response.pointer[1] - (root[1] - height * 0.5);
        let push = response.presence
            * (-away * away / (0.3 * 0.3) - rise * rise / (0.4 * 0.4)).exp()
            * away.signum()
            * 0.35;
        let lean = gust + push + response.velocity[0] * response.presence * 0.08;
        let place = |node: Node| {
            let (s, c) = (lean * node.reach).sin_cos();
            [
                root[0] + node.at[0] * c - node.at[1] * s,
                root[1] + node.at[0] * s + node.at[1] * c,
            ]
        };
        let shade = (index % 4) as f32;
        for run in &plant.stems {
            for pair in run.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if grown <= a.birth {
                    break;
                }
                let span = (b.birth - a.birth).max(1e-4);
                let along = ((grown - a.birth) / span).min(1.);
                let (from, to) = (place(a), place(b));
                let to = [
                    from[0] + (to[0] - from[0]) * along,
                    from[1] + (to[1] - from[1]) * along,
                ];
                let width = |reach: f32| 0.0045 * (1. - reach * 0.7) + 0.001;
                segments.push([
                    [from[0], from[1], to[0], to[1]],
                    [width(a.reach), width(b.reach), shade, left],
                ]);
            }
        }
        for &(node, angle) in &plant.leaves {
            let pop = ((grown - node.birth - 0.03) / 0.06).clamp(0., 1.);
            if pop <= 0. {
                continue;
            }
            let at = place(node);
            leaves.push([
                [at[0], at[1], angle + lean * node.reach, 0.045 * pop],
                [0.015 * pop, shade, left, 0.],
            ]);
        }
        for (n, &node) in plant.blooms.iter().enumerate() {
            let pop = ((grown - node.birth) / 0.06).clamp(0., 1.);
            if pop <= 0. {
                continue;
            }
            let at = place(node);
            let hue = ((index + n) % 4) as f32;
            let size = 0.024 + 0.016 * hash(index, n, 4);
            blooms.push([
                [at[0], at[1], size * pop, open],
                [hue, t * 2. + n as f32, left, 0.],
            ]);
        }
    }
    for (kind, items, most) in [
        (0, segments, SEGMENTS),
        (1, leaves, LEAVES),
        (2, blooms, BLOOMS),
    ] {
        for item in items.into_iter().take(most) {
            figures.push(kind, item);
        }
    }
    figures
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: [f32; 2] = [1000., 600.];

    #[test]
    fn the_formation_loops_and_morphs() {
        let still = Response::default();
        let first = figures(Ambience::Formation, 0., FRAME, &still);
        let last = figures(Ambience::Formation, TAU, FRAME, &still);
        assert_eq!(first.counts[0] as usize, POINTS);
        for (a, b) in first.items.iter().zip(&last.items) {
            for (x, y) in a[0].iter().zip(&b[0]) {
                assert!((x - y).abs() < 1e-3, "it loops seamlessly");
            }
        }
        // Each figure's points lie apart from the next figure's.
        let cube = figures(Ambience::Formation, TAU * 0.2, FRAME, &still);
        let sheet = figures(Ambience::Formation, TAU * 0.45, FRAME, &still);
        assert_ne!(cube.items[40][0], sheet.items[40][0]);
    }

    #[test]
    fn every_edge_of_the_cube_is_drawn_whole() {
        // On each edge, points within a twelfth of an edge of each other from one end to the
        // other: none missing, none short.
        let mut along = [const { Vec::new() }; 12];
        for i in 0..POINTS {
            let at = figure(0, i, 0.);
            let fixed: Vec<usize> = (0..3).filter(|&axis| at[axis].abs() > 0.7).collect();
            // A corner lies on three edges; the rest on one.
            if fixed.len() == 3 {
                continue;
            }
            assert_eq!(fixed.len(), 2, "each point lies on an edge");
            let free = 3 - fixed[0] - fixed[1];
            let edge = free * 4 + (at[fixed[0]] > 0.) as usize * 2 + (at[fixed[1]] > 0.) as usize;
            along[edge].push(at[free] / 0.72);
        }
        for mut edge in along {
            edge.sort_by(f32::total_cmp);
            assert!(edge.len() >= POINTS / 12);
            assert!(edge[0] < -0.8 && edge[edge.len() - 1] > 0.8);
            assert!(edge.windows(2).all(|w| w[1] - w[0] < 0.2));
        }
    }

    #[test]
    fn the_pointer_scatters_the_formation() {
        let still = Response::default();
        let calm = figures(Ambience::Formation, 1., FRAME, &still);
        let at = calm.items[10][0];
        let touched = Response {
            pointer: [at[0] + 0.02, at[1]],
            presence: 1.,
            ..Default::default()
        };
        let stirred = figures(Ambience::Formation, 1., FRAME, &touched);
        assert!(
            stirred.items[10][0][0] < at[0] - 0.02,
            "the point flees the pointer"
        );
    }

    #[test]
    fn the_garden_grows_blooms_and_fades_within_its_budget() {
        let still = Response::default();
        let bare = figures(Ambience::Garden, 0., FRAME, &still);
        assert_eq!(bare.counts.iter().sum::<u32>(), 0, "it starts bare");
        let full = figures(Ambience::Garden, TAU * 0.6, FRAME, &still);
        assert!(full.counts[0] > 60, "stems: {}", full.counts[0]);
        assert!(full.counts[1] > 10, "leaves: {}", full.counts[1]);
        assert!(
            full.counts[2] >= PLANTS as u32,
            "blooms: {}",
            full.counts[2]
        );
        // Every plant fits the budget whole.
        let plants: Vec<Plant> = (0..PLANTS).map(|i| plant(i, 0.62)).collect();
        let segments: usize = plants
            .iter()
            .flat_map(|p| &p.stems)
            .map(|run| run.len() - 1)
            .sum();
        let leaves: usize = plants.iter().map(|p| p.leaves.len()).sum();
        let blooms: usize = plants.iter().map(|p| p.blooms.len()).sum();
        assert!(
            segments <= SEGMENTS && leaves <= LEAVES && blooms <= BLOOMS,
            "{segments} segments, {leaves} leaves, {blooms} blooms"
        );
        let gone = figures(Ambience::Garden, TAU * 0.97, FRAME, &still);
        assert_eq!(
            gone.counts.iter().sum::<u32>(),
            0,
            "and it is bare again before it loops"
        );
    }

    #[test]
    fn the_garden_leans_away_from_the_pointer() {
        let still = Response::default();
        let calm = figures(Ambience::Garden, TAU * 0.6, FRAME, &still);
        // The first plant's first bloom, after the stems and the leaves.
        let tip = |f: &Figures| f.items[(f.counts[0] + f.counts[1]) as usize][0];
        let root = calm.items[0][0];
        let touched = Response {
            pointer: [root[0] + 0.1, root[1] - 0.2],
            presence: 1.,
            ..Default::default()
        };
        let pushed = figures(Ambience::Garden, TAU * 0.6, FRAME, &touched);
        assert!(tip(&pushed)[0] < tip(&calm)[0] - 0.005);
    }
}
