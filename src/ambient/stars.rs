//! Aurora's stars: a few brighter stars among the field that the pointer wakes and joins.
//!
//! Stars near the pointer brighten, and those awake are joined to their nearest neighbours, so a
//! constellation gathers round the pointer as it travels and fades behind it. Pressing on a star
//! and letting go over another draws a line between them that stays (drawing it again takes it
//! away), so the person can make constellations of their own; pressing on the open sky sends a
//! shooting star from there, and a double click wipes the drawn lines away.
//!
//! Positions are in the shaders' units, in the sky's own frame: the sky is drawn at a point `p`
//! plus a little of the window's sway, so a star at `s` shows where `p + sway * DEPTH` is `s`.

use std::time::Instant;

/// The brighter stars.
pub(super) const STARS: usize = 28;
/// The most lines drawn at once: the constellation round the pointer, the drawn ones and the one
/// being drawn.
pub(super) const LINES: usize = 48;
/// The most shooting stars at once.
pub(super) const METEORS: usize = 2;

/// How the stars lie: a grid of this many columns and rows over the sky, each star somewhere in
/// its cell.
const COLUMNS: usize = 7;
const ROWS: usize = 4;
/// How far the sky moves with the window's sway, as in `aurora.wgsl`.
const DEPTH: f32 = 0.15;
/// Where the sky meets the water, as `LAKE` in `aurora.wgsl`.
const LAKE: f32 = 0.24;
/// How far from the pointer stars wake.
const REACH: f32 = 0.17;
/// How near a press must be to a star to take it.
const HIT: f32 = 0.045;
/// How quickly a star wakes, and how slowly it falls asleep again, per second.
const WAKE: f32 = 5.;
const SLEEP: f32 = 0.9;
/// How bright the stars on a drawn line stay.
const PINNED: f32 = 0.8;
/// The longest line of a constellation round the pointer.
const NEIGHBOUR: f32 = 0.4;
/// How quickly a drawn line grows from one star to the other, and fades once wiped, per second.
const GROW: f32 = 2.5;
const FADE: f32 = 1.4;
/// The most lines the person can draw; the oldest goes first.
const DRAWN: usize = 24;
/// How long a shooting star lasts, in seconds.
const METEOR_LIFE: f32 = 1.1;
/// How long a star flares when pressed, in seconds.
const FLARE: f32 = 0.7;

/// The stars and the lines between them.
#[derive(Default)]
pub(super) struct Sky {
    size: [f32; 2],
    stars: Vec<Star>,
    /// Each star's two nearest neighbours, as pairs, without repeats.
    pairs: Vec<(usize, usize)>,
    links: Vec<Link>,
    /// The star a line is being drawn from.
    drawing: Option<usize>,
    meteors: Vec<Meteor>,
    pointer: Option<[f32; 2]>,
    sway: [f32; 2],
    last: Option<Instant>,
    /// Seconds simulated while the sky was moving.
    clock: f32,
}

struct Star {
    at: [f32; 2],
    size: f32,
    /// How awake the pointer has made it.
    glow: f32,
    /// How bright being on a drawn line keeps it, 0 to 1.
    pin: f32,
    flare: f32,
}

#[derive(Clone, Copy)]
struct Link {
    a: usize,
    b: usize,
    /// How far the line has grown from `a` to `b`, 0 to 1.
    grown: f32,
    /// 1 while it stays; falls to 0 once wiped.
    life: f32,
    wiped: bool,
}

struct Meteor {
    from: [f32; 2],
    heading: f32,
    age: f32,
}

/// The sky as the shader takes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Marks {
    /// Each star's place, how awake it is (0 asleep, up to 2 while it flares) and its size.
    pub stars: [[f32; 4]; STARS],
    /// Each line's ends, and how strong it is.
    pub lines: [[f32; 4]; LINES],
    pub strengths: [f32; LINES],
    pub line_count: usize,
    /// Each shooting star's head, its heading and how far through its life it is.
    pub meteors: [[f32; 4]; METEORS],
    pub meteor_count: usize,
}

impl Default for Marks {
    fn default() -> Self {
        Self {
            stars: [[0.; 4]; STARS],
            lines: [[0.; 4]; LINES],
            strengths: [0.; LINES],
            line_count: 0,
            meteors: [[0.; 4]; METEORS],
            meteor_count: 0,
        }
    }
}

impl Sky {
    /// Lay the stars out over a box of `size`, keeping how awake they are.
    fn fit(&mut self, size: [f32; 2]) {
        if self.size == size && !self.stars.is_empty() {
            return;
        }
        self.size = size;
        let unit = size[0].min(size[1]).max(1.);
        let (width, height) = (size[0] / unit, size[1] / unit);
        let top = -0.5 * height + 0.05;
        let bottom = LAKE - 0.07;
        let stars = (0..STARS).map(|index| {
            let (column, row) = (index % COLUMNS, index / COLUMNS);
            let fx = (column as f32 + 0.15 + 0.7 * hash(index, 1)) / COLUMNS as f32;
            let fy = (row as f32 + 0.15 + 0.7 * hash(index, 2)) / ROWS as f32;
            let at = [(fx - 0.5) * width * 0.92, top + fy * (bottom - top)];
            let (glow, pin, flare) = self
                .stars
                .get(index)
                .map_or((0., 0., 0.), |star| (star.glow, star.pin, star.flare));
            Star {
                at,
                size: 0.4 + 0.6 * hash(index, 3),
                glow,
                pin,
                flare,
            }
        });
        self.stars = stars.collect();
        self.pairs.clear();
        for i in 0..STARS {
            let mut nearest: Vec<usize> = (0..STARS).filter(|&j| j != i).collect();
            nearest.sort_by(|&a, &b| {
                let da = distance(self.stars[i].at, self.stars[a].at);
                let db = distance(self.stars[i].at, self.stars[b].at);
                da.total_cmp(&db)
            });
            for &j in nearest.iter().take(2) {
                let pair = (i.min(j), i.max(j));
                if distance(self.stars[i].at, self.stars[j].at) < NEIGHBOUR
                    && !self.pairs.contains(&pair)
                {
                    self.pairs.push(pair);
                }
            }
        }
    }

    /// `pointer`, in pixels from the box's corner, in the sky's units.
    fn place(&self, pointer: [f32; 2]) -> [f32; 2] {
        let unit = self.size[0].min(self.size[1]).max(1.);
        [
            (pointer[0] - 0.5 * self.size[0]) / unit + self.sway[0] * DEPTH,
            (pointer[1] - 0.5 * self.size[1]) / unit + self.sway[1] * DEPTH,
        ]
    }

    /// The star under `pointer` (in pixels), if any.
    fn star_at(&self, pointer: [f32; 2]) -> Option<usize> {
        let at = self.place(pointer);
        self.stars
            .iter()
            .enumerate()
            .map(|(index, star)| (index, distance(star.at, at)))
            .filter(|(_, d)| *d < HIT)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }

    /// How awake star `index` is on its way to being, as near as it is to the pointer; and
    /// whether it is on a drawn line or a line is being drawn from it.
    fn target(&self, index: usize) -> (f32, f32) {
        let near = self.pointer.map_or(0., |at| {
            let d = distance(self.stars[index].at, at);
            (-d * d / (REACH * REACH)).exp()
        });
        let pinned = self.drawing == Some(index)
            || self
                .links
                .iter()
                .any(|link| !link.wiped && (link.a == index || link.b == index));
        (near, if pinned { 1. } else { 0. })
    }

    /// Move on to `now`, for a box of `size`, with the pointer at `pointer` (in pixels from the
    /// box's corner) while it is over the sky, and the scene swayed by `sway` (in units).
    pub fn step(
        &mut self,
        now: Instant,
        size: [f32; 2],
        pointer: Option<[f32; 2]>,
        sway: [f32; 2],
    ) {
        let dt = self
            .last
            .map_or(0., |last| (now - last).as_secs_f32().min(0.1));
        self.last = Some(now);
        self.fit(size);
        self.sway = sway;
        self.pointer = pointer.map(|pointer| self.place(pointer));

        for index in 0..self.stars.len() {
            let (target, pinned) = self.target(index);
            let star = &mut self.stars[index];
            let rate = if target > star.glow { WAKE } else { SLEEP };
            star.glow += (target - star.glow) * (1. - (-rate * dt).exp());
            if (target - star.glow).abs() < 0.003 {
                star.glow = target;
            }
            star.pin = if pinned > 0. {
                (star.pin + GROW * dt).min(1.)
            } else {
                (star.pin - FADE * dt).max(0.)
            };
            star.flare = (star.flare - dt / FLARE).max(0.);
        }
        for link in &mut self.links {
            link.grown = (link.grown + GROW * dt).min(1.);
            if link.wiped {
                link.life = (link.life - FADE * dt).max(0.);
            }
        }
        self.links.retain(|link| link.life > 0.);
        for meteor in &mut self.meteors {
            meteor.age += dt / METEOR_LIFE;
        }
        self.meteors.retain(|meteor| meteor.age < 1.);
        if self.moving() {
            self.clock += dt;
        }
    }

    /// A press at `pointer` (in pixels): take the star under it to draw a line from, or send a
    /// shooting star across the open sky. Returns whether the sky took the press.
    pub fn press(&mut self, pointer: [f32; 2]) -> bool {
        if let Some(index) = self.star_at(pointer) {
            self.drawing = Some(index);
            self.stars[index].flare = 1.;
            return true;
        }
        let from = self.place(pointer);
        if from[1] > LAKE {
            return false;
        }
        if self.meteors.len() == METEORS {
            self.meteors.remove(0);
        }
        // Away from the middle and down, as shooting stars fall towards the horizon.
        let heading = if from[0] > 0. { 2.55 } else { 0.6 } + 0.25 * hash(self.meteors.len(), 9);
        self.meteors.push(Meteor {
            from,
            heading,
            age: 0.,
        });
        true
    }

    /// Let go at `pointer` (in pixels, if over the sky): over another star, join it to the one the
    /// line was drawn from, or take the line away if they were joined already.
    pub fn release(&mut self, pointer: Option<[f32; 2]>) {
        let Some(from) = self.drawing.take() else {
            return;
        };
        let Some(to) = pointer.and_then(|pointer| self.star_at(pointer)) else {
            return;
        };
        if to == from {
            return;
        }
        self.stars[to].flare = 1.;
        let joined = self.links.iter().position(|link| {
            !link.wiped && ((link.a, link.b) == (from, to) || (link.a, link.b) == (to, from))
        });
        match joined {
            Some(index) => self.links[index].wiped = true,
            None => {
                if self.links.iter().filter(|link| !link.wiped).count() == DRAWN
                    && let Some(oldest) = self.links.iter_mut().find(|link| !link.wiped)
                {
                    oldest.wiped = true;
                }
                self.links.push(Link {
                    a: from,
                    b: to,
                    grown: 0.,
                    life: 1.,
                    wiped: false,
                });
            }
        }
    }

    /// Wipe every drawn line away.
    pub fn wipe(&mut self) {
        self.drawing = None;
        for link in &mut self.links {
            link.wiped = true;
        }
    }

    /// Whether a line is being drawn.
    pub fn drawing(&self) -> bool {
        self.drawing.is_some()
    }

    /// Whether `pointer` (in pixels) is over a star.
    pub fn over_star(&self, pointer: [f32; 2]) -> bool {
        self.star_at(pointer).is_some()
    }

    /// Whether anything in the sky is still changing.
    pub fn moving(&self) -> bool {
        self.drawing.is_some()
            || !self.meteors.is_empty()
            || self.links.iter().any(|link| link.grown < 1. || link.wiped)
            || (0..self.stars.len()).any(|index| {
                let star = &self.stars[index];
                let (glow, pin) = self.target(index);
                star.flare > 0. || star.glow != glow || star.pin != pin
            })
    }

    /// Seconds simulated while the sky was moving.
    pub fn clock(&self) -> f32 {
        self.clock
    }

    /// The sky for a box of `size`, as the shader takes it.
    pub fn marks(&mut self, size: [f32; 2]) -> Marks {
        self.fit(size);
        let mut marks = Marks::default();
        for (slot, star) in marks.stars.iter_mut().zip(&self.stars) {
            let awake = star.glow.max(star.pin * PINNED) + star.flare;
            *slot = [star.at[0], star.at[1], awake, star.size];
        }
        let mut lines = Vec::new();
        // The constellation round the pointer: stars it has woken together are joined.
        for &(a, b) in &self.pairs {
            let strength = self.stars[a].glow.min(self.stars[b].glow).powf(1.5) * 0.75;
            if strength > 0.01 {
                lines.push((self.stars[a].at, self.stars[b].at, strength));
            }
        }
        for link in &self.links {
            let (from, to) = (self.stars[link.a].at, self.stars[link.b].at);
            let grown = link.grown * link.grown * (3. - 2. * link.grown);
            let end = [
                from[0] + (to[0] - from[0]) * grown,
                from[1] + (to[1] - from[1]) * grown,
            ];
            lines.push((from, end, link.life));
        }
        if let (Some(from), Some(pointer)) = (self.drawing, self.pointer) {
            lines.push((self.stars[from].at, pointer, 0.6));
        }
        // The drawn lines matter most: keep them if there are too many.
        let skip = lines.len().saturating_sub(LINES);
        for (index, (from, to, strength)) in lines.into_iter().skip(skip).enumerate() {
            marks.lines[index] = [from[0], from[1], to[0], to[1]];
            marks.strengths[index] = strength;
            marks.line_count = index + 1;
        }
        for (slot, meteor) in marks.meteors.iter_mut().zip(&self.meteors) {
            let travel = 0.75 * (1. - (1. - meteor.age).powi(2));
            *slot = [
                meteor.from[0] + meteor.heading.cos() * travel,
                meteor.from[1] + meteor.heading.sin() * travel,
                meteor.heading,
                meteor.age,
            ];
        }
        marks.meteor_count = self.meteors.len();
        marks
    }

    /// Where star `index` is in a box of `size`, in pixels from its corner, unswayed.
    #[cfg(test)]
    pub fn star(&mut self, index: usize, size: [f32; 2]) -> [f32; 2] {
        self.fit(size);
        let unit = size[0].min(size[1]);
        let at = self.stars[index].at;
        [at[0] * unit + 0.5 * size[0], at[1] * unit + 0.5 * size[1]]
    }
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// A number from 0 to 1 for `index` and `salt`, the same every time.
fn hash(index: usize, salt: u32) -> f32 {
    let mut x = (index as u32)
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(salt.wrapping_mul(0x85eb_ca6b));
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x & 0xffff) as f32 / 65535.
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const SIZE: [f32; 2] = [1000., 600.];

    fn run(sky: &mut Sky, start: Instant, from: usize, count: usize, pointer: Option<[f32; 2]>) {
        for frame in from..from + count {
            sky.step(
                start + Duration::from_millis(16 * frame as u64),
                SIZE,
                pointer,
                [0., 0.],
            );
        }
    }

    #[test]
    fn the_stars_lie_in_the_sky_and_fill_it() {
        let mut sky = Sky::default();
        sky.fit(SIZE);
        assert_eq!(sky.stars.len(), STARS);
        for star in &sky.stars {
            assert!(star.at[1] < LAKE && star.at[1] > -0.5, "{:?}", star.at);
            assert!(star.at[0].abs() < 0.5 * SIZE[0] / 600.);
        }
        assert!(sky.pairs.len() >= STARS, "most stars have neighbours");
    }

    #[test]
    fn the_pointer_wakes_a_constellation_that_fades_behind_it() {
        let mut sky = Sky::default();
        let start = Instant::now();
        run(&mut sky, start, 0, 1, None);
        assert_eq!(sky.marks(SIZE).line_count, 0);
        assert!(!sky.moving());
        let over = sky.star(10, SIZE);
        run(&mut sky, start, 1, 120, Some(over));
        let marks = sky.marks(SIZE);
        assert!(marks.stars[10][2] > 0.95, "the star under it is awake");
        assert!(marks.line_count > 0, "and joined to its neighbours");
        assert!(!sky.moving(), "and it settles while the pointer rests");
        run(&mut sky, start, 121, 600, None);
        assert_eq!(
            sky.marks(SIZE).line_count,
            0,
            "it fades once the pointer leaves"
        );
        assert!(!sky.moving());
    }

    #[test]
    fn a_line_drawn_between_two_stars_stays_until_wiped() {
        let mut sky = Sky::default();
        let start = Instant::now();
        run(&mut sky, start, 0, 1, None);
        let (a, b) = (sky.star(3, SIZE), sky.star(4, SIZE));
        assert!(sky.press(a));
        assert!(sky.drawing());
        run(&mut sky, start, 1, 20, Some(b));
        sky.release(Some(b));
        run(&mut sky, start, 21, 600, None);
        let marks = sky.marks(SIZE);
        assert_eq!(marks.line_count, 1, "the drawn line stays");
        assert!((marks.lines[0][2] - sky.stars[4].at[0]).abs() < 1e-4);
        assert!(marks.stars[3][2] >= PINNED - 0.01);
        assert!(!sky.moving(), "and nothing is left moving");
        // Drawn again, it goes away.
        sky.press(a);
        sky.release(Some(b));
        run(&mut sky, start, 621, 120, None);
        assert_eq!(sky.marks(SIZE).line_count, 0);
        // Drawn and wiped.
        sky.press(a);
        sky.release(Some(b));
        run(&mut sky, start, 741, 60, None);
        sky.wipe();
        run(&mut sky, start, 801, 120, None);
        assert_eq!(sky.marks(SIZE).line_count, 0);
    }

    #[test]
    fn a_press_on_the_open_sky_sends_a_shooting_star() {
        let mut sky = Sky::default();
        let start = Instant::now();
        run(&mut sky, start, 0, 1, None);
        // Between stars, high in the sky.
        let open = (0..40)
            .map(|n| [25. * n as f32, 40.])
            .find(|&at| !sky.over_star(at))
            .unwrap();
        assert!(sky.press(open));
        run(&mut sky, start, 1, 10, None);
        assert_eq!(sky.marks(SIZE).meteor_count, 1);
        assert!(sky.moving());
        run(&mut sky, start, 11, 120, None);
        assert_eq!(sky.marks(SIZE).meteor_count, 0);
        // The water doesn't take presses.
        assert!(!sky.press([500., 560.]));
    }
}
