//! Shoal's fish, and the food the person scatters for them.
//!
//! The two schools wheel along their paths as `shoal.wgsl` once laid them out itself; here they
//! are laid out on the CPU instead, so they can leave their paths. A press on the water scatters a
//! pinch of food there with a splash: the pellets sink slowly, rocking, and the fish near them
//! break from their school and dart in to snap them up, each catching the light as it eats, then
//! drift back into the school. Pellets nobody takes sink out of the bottom of the frame.
//!
//! Positions are in the shaders' units, from the frame's centre, y down. The water has no frame of
//! its own: everything is where it shows, with the window's sway already taken off.

use std::f32::consts::PI;
use std::time::Instant;

use super::figures::{Figures, hash};
use super::sim::Response;

pub(super) const SCHOOLS: usize = 2;
pub(super) const FISH: usize = 26;
/// The most pellets in the water at once; the oldest goes first.
pub(super) const PELLETS: usize = 24;

/// Pellets in a pinch, and how widely they scatter.
const PINCH: usize = 5;
const SCATTER: f32 = 0.035;
/// How fast a pellet sinks, in units a second, and how far it rocks from side to side.
const SINK: f32 = 0.04;
const ROCK: f32 = 0.012;
/// How far a fish notices food from, and how many fish go after one pellet.
const SMELL: f32 = 0.32;
const CHASERS: usize = 3;
/// How quickly a fish closes on its food, per second, and its top speed, in units a second.
const DART: f32 = 2.2;
const SPEED: f32 = 0.32;
/// How quickly a fed fish drifts back to its school, per second.
const RETURN: f32 = 1.1;
/// How quickly a fish turns, per second.
const TURN: f32 = 7.;
/// How near its mouth must come to a pellet to eat it.
const BITE: f32 = 0.03;
/// How long a fish shines after eating, in seconds.
const SHINE: f32 = 0.9;

/// The fish and the food in the water.
#[derive(Default)]
pub(super) struct Pond {
    pellets: Vec<Pellet>,
    /// Each fish's way off its path, and which way it faces while off it.
    fish: Vec<Fish>,
    last: Option<Instant>,
    /// A splash of food to scatter on the next step, where it lands (in pixels).
    pending: Vec<[f32; 2]>,
    /// Seconds simulated while anything in the water was changing.
    clock: f32,
    seeds: usize,
}

#[derive(Clone, Copy)]
struct Pellet {
    at: [f32; 2],
    age: f32,
    seed: usize,
}

#[derive(Clone, Copy, Default)]
struct Fish {
    offset: [f32; 2],
    /// Which way it faces while hunting, and how much that wins over its path.
    facing: [f32; 2],
    hunting: f32,
    shine: f32,
}

/// Where a fish swims at moment `t` before it notices anything: where it is, which way it heads,
/// and its size and seed.
struct Course {
    at: [f32; 2],
    heading: [f32; 2],
    size: f32,
    seed: f32,
}

/// Fish `index` of the schools at moment `t`, in a frame `width` units wide, the scene swayed by
/// `sway`.
fn course(index: usize, t: f32, width: f32, sway: [f32; 2]) -> Course {
    let (school, f) = (index / FISH, index % FISH);
    let s = school as f32;
    // The farther school is smaller and paler in the water.
    let scale = 0.8 + 0.4 * s;
    let depth = 0.3 + 0.5 * s;
    let lag = -hash(index, 1, 7) * 0.8;
    let side = (hash(index, 2, 7) - 0.5) * 0.13 * scale;
    let size = (0.028 + 0.014 * hash(index, 3, 7)) * scale;
    // Each school wheels through the water in a figure of eight.
    let t = t + lag;
    let reach = [0.5 * width * 0.78, 0.14 + 0.03 * s];
    let level = -0.1 + 0.24 * s;
    let (a, b) = (t + s * PI, 2. * t + s * 1.3);
    let heading = normalize([reach[0] * a.cos(), 2. * reach[1] * b.cos()]);
    let across = [-heading[1], heading[0]];
    let turn = (5. + (f % 3) as f32) * (t - lag) + index as f32;
    let at = [
        reach[0] * a.sin() + across[0] * side + turn.cos() * 0.008 - sway[0] * depth,
        reach[1] * b.sin() + level + across[1] * side + turn.sin() * 0.008 - sway[1] * depth,
    ];
    Course {
        at,
        heading,
        size,
        seed: index as f32,
    }
}

impl Pond {
    /// Scatter a pinch of food where the person pressed, at `pointer` (in pixels).
    pub fn feed(&mut self, pointer: [f32; 2]) {
        self.pending.push(pointer);
    }

    /// Move on to `now` and moment `t` of the cycle, for a box of `size` (in pixels), as the
    /// scene answers `response`.
    pub fn step(&mut self, now: Instant, t: f32, size: [f32; 2], response: &Response) {
        let dt = self
            .last
            .map_or(0., |last| (now - last).as_secs_f32().min(0.1));
        self.last = Some(now);
        let unit = size[0].min(size[1]).max(1.);
        let (width, height) = (size[0] / unit, size[1] / unit);
        self.fish.resize(SCHOOLS * FISH, Fish::default());

        for pointer in std::mem::take(&mut self.pending) {
            let at = [
                (pointer[0] - 0.5 * size[0]) / unit,
                (pointer[1] - 0.5 * size[1]) / unit,
            ];
            for _ in 0..PINCH {
                self.seeds += 1;
                let seed = self.seeds;
                let angle = hash(seed, 1, 3) * 2. * PI;
                let r = SCATTER * hash(seed, 2, 3).sqrt();
                if self.pellets.len() == PELLETS {
                    self.pellets.remove(0);
                }
                self.pellets.push(Pellet {
                    at: [at[0] + angle.cos() * r, at[1] + angle.sin() * r],
                    age: 0.,
                    seed,
                });
            }
        }
        for pellet in &mut self.pellets {
            pellet.age += dt;
            let speed = SINK * (0.75 + 0.5 * hash(pellet.seed, 4, 3));
            // It settles into its sinking, rocking as it goes.
            let rock = (pellet.age * 2.4 + hash(pellet.seed, 5, 3) * 6.).cos() * 2.4 * ROCK;
            pellet.at[0] += rock * dt;
            pellet.at[1] += speed * (1. - (-pellet.age * 3.).exp()) * dt;
        }
        self.pellets
            .retain(|pellet| pellet.at[1] < 0.5 * height + 0.05);

        let rate = |rate: f32| 1. - (-rate * dt).exp();
        // Each fish makes for the nearest food in reach, but only the few nearest each pellet
        // go after it; the rest keep to their school.
        let paths: Vec<Course> = (0..self.fish.len())
            .map(|index| course(index, t, width, response.sway))
            .collect();
        let mut food: Vec<Option<(usize, f32)>> = (0..self.fish.len())
            .map(|index| {
                let at = add(paths[index].at, self.fish[index].offset);
                self.pellets
                    .iter()
                    .enumerate()
                    .map(|(which, pellet)| (which, distance(pellet.at, at)))
                    .filter(|&(_, d)| d < SMELL)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
            })
            .collect();
        for which in 0..self.pellets.len() {
            let mut claims: Vec<usize> = (0..food.len())
                .filter(|&index| food[index].is_some_and(|(pellet, _)| pellet == which))
                .collect();
            claims.sort_by(|&a, &b| food[a].unwrap().1.total_cmp(&food[b].unwrap().1));
            for &index in claims.iter().skip(CHASERS) {
                food[index] = None;
            }
        }
        let mut eaten = vec![false; self.pellets.len()];
        for (index, path) in paths.iter().enumerate() {
            let fish = &mut self.fish[index];
            fish.shine = (fish.shine - dt / SHINE).max(0.);
            let Some((which, _)) = food[index].filter(|&(which, _)| !eaten[which]) else {
                fish.offset = scale(fish.offset, 1. - rate(RETURN));
                fish.hunting *= 1. - rate(RETURN * 2.);
                if length(fish.offset) < 1e-4 && fish.hunting < 1e-3 {
                    fish.offset = [0., 0.];
                    fish.hunting = 0.;
                }
                continue;
            };
            let at = add(path.at, fish.offset);
            let pellet = self.pellets[which].at;
            // It comes at the pellet head first, a little to one side of the others.
            let towards = normalize(sub(pellet, at));
            let aside = (hash(index, 6, 7) - 0.5) * 0.02;
            let goal = add(
                sub(pellet, scale(towards, path.size * 0.6)),
                [-towards[1] * aside, towards[0] * aside],
            );
            let mut step = scale(sub(goal, at), rate(DART));
            let most = SPEED * dt;
            if length(step) > most {
                step = scale(normalize(step), most);
            }
            fish.offset = add(fish.offset, step);
            fish.hunting += (1. - fish.hunting) * rate(TURN);
            let facing = if fish.facing == [0., 0.] {
                towards
            } else {
                fish.facing
            };
            fish.facing = normalize(add(facing, scale(sub(towards, facing), rate(TURN))));
            // A mouth on the pellet eats it.
            let mouth = add(
                add(path.at, fish.offset),
                scale(fish.facing, path.size * 0.9),
            );
            if distance(mouth, pellet) < BITE {
                fish.shine = 1.;
                eaten[which] = true;
            }
        }
        let mut which = 0;
        self.pellets.retain(|_| {
            which += 1;
            !eaten[which - 1]
        });
        if self.moving() {
            self.clock += dt;
        }
    }

    /// Whether anything in the water is still changing: food sinking or fish off their paths.
    pub fn moving(&self) -> bool {
        !self.pellets.is_empty()
            || !self.pending.is_empty()
            || self
                .fish
                .iter()
                .any(|fish| fish.offset != [0., 0.] || fish.hunting > 0. || fish.shine > 0.)
    }

    /// Seconds simulated while anything in the water was changing.
    pub fn clock(&self) -> f32 {
        self.clock
    }

    /// The fish and the food at moment `t` of the cycle in a box of `size` (in pixels), as
    /// `shoal.wgsl` takes them: each fish where it swims (before the pointer frightens it) and
    /// which way it heads, its size, its seed, how intent it is on food and how much it shines; then
    /// each pellet, where it is and how far it has settled.
    pub fn figures(&self, t: f32, size: [f32; 2], response: &Response) -> Figures {
        let unit = size[0].min(size[1]).max(1.);
        let mut figures = Figures::default();
        for index in 0..SCHOOLS * FISH {
            let path = course(index, t, size[0] / unit, response.sway);
            let fish = self.fish.get(index).copied().unwrap_or_default();
            let at = add(path.at, fish.offset);
            let heading = if fish.hunting > 0. {
                normalize(add(
                    path.heading,
                    scale(sub(fish.facing, path.heading), fish.hunting),
                ))
            } else {
                path.heading
            };
            figures.push(
                0,
                [
                    [at[0], at[1], heading[0], heading[1]],
                    [path.size, path.seed, fish.hunting, fish.shine],
                ],
            );
        }
        for pellet in &self.pellets {
            let settled = (pellet.age * 3.).min(1.);
            figures.push(
                1,
                [
                    [pellet.at[0], pellet.at[1], settled, hash(pellet.seed, 7, 3)],
                    [0.; 4],
                ],
            );
        }
        figures
    }
}

fn add(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

fn sub(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn scale(a: [f32; 2], factor: f32) -> [f32; 2] {
    [a[0] * factor, a[1] * factor]
}

fn length(a: [f32; 2]) -> f32 {
    (a[0] * a[0] + a[1] * a[1]).sqrt()
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    length(sub(a, b))
}

fn normalize(a: [f32; 2]) -> [f32; 2] {
    let l = length(a);
    if l < 1e-6 { [1., 0.] } else { scale(a, 1. / l) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const SIZE: [f32; 2] = [1000., 600.];

    fn run(pond: &mut Pond, start: Instant, from: usize, count: usize) {
        for frame in from..from + count {
            let now = start + Duration::from_secs_f32(frame as f32 / 60.);
            pond.step(now, 1., SIZE, &Response::default());
        }
    }

    #[test]
    fn an_untouched_pond_keeps_the_fish_on_their_paths() {
        let still = Pond::default().figures(1., SIZE, &Response::default());
        let mut pond = Pond::default();
        run(&mut pond, Instant::now(), 0, 30);
        assert!(!pond.moving());
        assert_eq!(pond.figures(1., SIZE, &Response::default()), still);
        assert_eq!(still.counts, [(SCHOOLS * FISH) as u32, 0, 0, 0]);
    }

    #[test]
    fn the_fish_eat_the_food_and_go_back_to_their_school() {
        let mut pond = Pond::default();
        let start = Instant::now();
        run(&mut pond, start, 0, 1);
        // Food dropped beside a fish.
        let fish = Pond::default()
            .figures(1., SIZE, &Response::default())
            .items[3][0];
        let unit = SIZE[1];
        pond.feed([
            fish[0] * unit + 0.5 * SIZE[0] + 30.,
            fish[1] * unit + 0.5 * SIZE[1],
        ]);
        run(&mut pond, start, 1, 2);
        assert!(!pond.pellets.is_empty());
        run(&mut pond, start, 3, 20);
        assert!(
            pond.fish.iter().any(|fish| fish.offset != [0., 0.]),
            "they go after it"
        );
        run(&mut pond, start, 23, 60 * 4 - 20);
        assert!(pond.pellets.is_empty(), "and eat it all");
        run(&mut pond, start, 3 + 60 * 4, 60 * 12);
        assert!(!pond.moving(), "and swam back");
    }

    #[test]
    fn food_left_alone_sinks_away_and_the_water_settles() {
        let mut pond = Pond::default();
        let start = Instant::now();
        pond.feed([10., SIZE[1] - 20.]);
        run(&mut pond, start, 0, 2);
        assert!(pond.moving());
        run(&mut pond, start, 2, 60 * 16);
        assert!(pond.pellets.is_empty());
        assert!(!pond.moving());
    }
}
