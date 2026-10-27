//! How the liquid answers the pointer and the window.
//!
//! Each drop hangs on a spring from its place on its orbit. The pointer pushes nearby drops aside
//! and drags them along as it passes, so pools stretch, part and close behind it; moving or
//! resizing the window leaves the liquid behind for a moment, and it sloshes back. A move is let
//! out over a few frames rather than all at once: during a drag macOS reports the window's place
//! in uneven steps, and taking each step whole made the liquid jump with it. Drops of
//! different sizes swing at different rates, so a pool deforms rather than moving as one. Where
//! the pointer crosses the metal it leaves ripples that run outwards and fade.
//!
//! The drops can be picked up, the way you gather the oil on a bowl of ramen. Press on the metal
//! and the drop under the pointer follows it, pulling out of its pool; let go and it stays where
//! it was put, sliding on a little if it was flung. A drop that has been handled fuses with any
//! drop it touches: the two flow into one larger drop of their combined area, so a handful of
//! drops gathered together become a single pool. Every drop keeps swinging on its orbit
//! wherever it rests. Now and then a fused drop sheds one of the drops it took, which slides
//! off on its own; a pointer brushing across it shakes one loose far more often, and one carried
//! too fast tears apart, flinging the drops it took off along its path. A double click
//! lets them all part again and go home.
//!
//! Distances here are logical pixels; rates are per second.

use std::time::Instant;

use super::{DROPS, THRESHOLD, field_at};

/// How stiffly a drop of average size is pulled back to its orbit.
const SPRING: f32 = 26.;
/// How quickly a swing dies away.
const DAMPING: f32 = 2.6;
/// How much of a move of the window the liquid is left behind by.
const LAG: f32 = 0.8;
/// How quickly a move of the window is let out into the drops, per second.
const CATCH: f32 = 14.;
/// How far the pointer reaches, in units of the box's shorter side, beyond a drop's own radius.
const POINTER_REACH: f32 = 0.16;
/// How hard the pointer pushes a drop it is on top of, in units per second squared.
const PUSH: f32 = 1.6;
/// How much of the pointer's velocity a drop it is on top of picks up each second.
const STIR: f32 = 3.;
/// The furthest a drop is let stray from its orbit, in units.
const LIMIT: f32 = 0.22;
/// Pointer travel over the metal between one ripple and the next, in units.
const WAVE_SPACING: f32 = 0.06;
/// How fast a ripple spreads, in units per second.
const WAVE_SPEED: f32 = 0.42;
/// How long a ripple lasts, in seconds.
const WAVE_LIFE: f32 = 2.4;
/// The most ripples on the surface at once.
pub(super) const WAVES: usize = 8;
/// The longest step simulated at once; a longer gap between frames is split.
const SUBSTEP: f32 = 1. / 120.;
/// How stiffly a held drop follows the hand, per second squared, and how quickly its swing
/// dies; a little short of critical, so it trails and wobbles like a liquid.
const HOLD: f32 = 420.;
const HOLD_DAMPING: f32 = 30.;
/// How much of the field's surface level the pointer must be over to take hold of a drop.
const GRIP: f32 = 0.7;
/// The fastest a drop is flung, in units per second.
const FLING: f32 = 2.5;
/// How quickly a flung drop slows, per second.
const FRICTION: f32 = 2.2;
/// How much of a bounce off the edge a flung drop keeps.
const BOUNCE: f32 = 0.5;
/// Two drops fuse when their centres are closer than this share of their radii added together.
const FUSE: f32 = 0.85;
/// How quickly a fused drop flows into the one that took it, per second.
const FLOW: f32 = 7.;
/// How quickly drops let go by a double click go home, per second.
const RETURN: f32 = 2.5;
/// How often, left alone, a fused drop sheds one of the drops it took: once in this many
/// seconds on average.
const SHED_EVERY: f32 = 20.;
/// How often a pointer brushing across a fused drop shakes one loose, per second, and how fast
/// it must be moving to, in units per second.
const BRUSH: f32 = 4.;
const BRUSH_SPEED: f32 = 0.35;
/// How fast a shed drop slides off, in units per second.
const SHED_SPEED: f32 = 1.;
/// How fast a drop in hand may be carried before it tears apart, in units per second, and how
/// often, past that, it flings off one of the drops it took, per second.
const TEAR_SPEED: f32 = 3.5;
const TEAR: f32 = 14.;
/// How widely torn-off drops scatter either side of the path, in radians.
const TEAR_SPREAD: f32 = 0.7;

/// The liquid's response so far.
pub(super) struct Sim {
    offsets: [[f32; 2]; DROPS.len()],
    velocities: [[f32; 2]; DROPS.len()],
    /// What the window's moves have yet to leave behind, per drop.
    pending: [[f32; 2]; DROPS.len()],
    /// Where each drop rests off its orbit, having been moved by hand.
    anchors: [[f32; 2]; DROPS.len()],
    /// How fast each resting place still slides after a fling.
    drift: [[f32; 2]; DROPS.len()],
    /// Each drop's area, in square units, and the area it is flowing towards.
    areas: [f32; DROPS.len()],
    targets: [f32; DROPS.len()],
    /// The drops that have been handled: they fuse with any drop they touch.
    loose: [bool; DROPS.len()],
    /// The drop each fused drop is flowing into.
    into: [Option<usize>; DROPS.len()],
    /// Pairs that were already touching when one of them was picked up: they don't fuse until
    /// they part, so a drop can be drawn out of its pool rather than swallowing it.
    joined: [[bool; DROPS.len()]; DROPS.len()],
    /// The state of the dice that decide when a fused drop sheds one.
    dice: u32,
    /// How many drops have been shed so far.
    #[cfg(test)]
    sheds: usize,
    grab: Option<Grab>,
    waves: Vec<Wave>,
    last: Option<Instant>,
    origin: Option<[f32; 2]>,
    size: Option<[f32; 2]>,
    pointer: Option<[f32; 2]>,
    travel: f32,
    /// Seconds simulated while the liquid was moving.
    clock: f32,
}

/// A drop in hand: which, and where its centre sits from the pointer.
#[derive(Clone, Copy)]
struct Grab {
    index: usize,
    hold: [f32; 2],
}

impl Default for Sim {
    fn default() -> Self {
        let areas = DROPS.map(|drop| drop.radius * drop.radius);
        Self {
            offsets: Default::default(),
            velocities: Default::default(),
            pending: Default::default(),
            anchors: Default::default(),
            drift: Default::default(),
            areas,
            targets: areas,
            loose: Default::default(),
            into: Default::default(),
            joined: Default::default(),
            dice: 0x9e37_79b9,
            #[cfg(test)]
            sheds: 0,
            grab: None,
            waves: Vec::new(),
            last: None,
            origin: None,
            size: None,
            pointer: None,
            travel: 0.,
            clock: 0.,
        }
    }
}

struct Wave {
    centre: [f32; 2],
    age: f32,
    strength: f32,
}

/// What the liquid sees this frame.
pub(super) struct Input<'a> {
    pub now: Instant,
    /// The window's position on the screen.
    pub origin: [f32; 2],
    /// The backdrop's size.
    pub size: [f32; 2],
    /// The pointer, relative to the backdrop, while it is over the window or holds a drop.
    pub pointer: Option<[f32; 2]>,
    /// Each drop's place on its orbit and its reach, before any offset.
    pub drops: &'a [(f32, f32, f32)],
}

impl Sim {
    pub fn step(&mut self, input: Input) {
        let dt = self
            .last
            .map_or(0., |last| (input.now - last).as_secs_f32().min(0.1));
        self.last = Some(input.now);
        let unit = input.size[0].min(input.size[1]).max(1.);

        // A moved window leaves the liquid behind; a resized one stretches the orbits under it,
        // and the drops catch up.
        if let Some(origin) = self.origin.replace(input.origin) {
            let moved = sub(input.origin, origin);
            for pending in &mut self.pending {
                *pending = sub(*pending, scale(moved, LAG));
            }
        }
        if let Some(size) = self.size.replace(input.size) {
            let grown = sub(input.size, size);
            for (pending, drop) in self.pending.iter_mut().zip(&DROPS) {
                *pending = sub(*pending, scale([grown[0] * drop.x, grown[1] * drop.y], LAG));
            }
        }

        // The pointer's velocity, and ripples along its path over the metal.
        let previous = std::mem::replace(&mut self.pointer, input.pointer);
        let velocity = match (previous, input.pointer) {
            (Some(from), Some(to)) if dt > 0. => scale(sub(to, from), 1. / dt),
            _ => [0., 0.],
        };
        if let (Some(from), Some(to)) = (previous, input.pointer) {
            self.travel += length(sub(to, from));
            let drops = self.drops(input.drops);
            let (field, _, _) = field_at(&drops, to[0], to[1]);
            if self.travel > WAVE_SPACING * unit && field > THRESHOLD * 0.6 {
                self.travel = 0.;
                if self.waves.len() == WAVES {
                    self.waves.remove(0);
                }
                self.waves.push(Wave {
                    centre: to,
                    age: 0.,
                    strength: (length(velocity) / (1.5 * unit)).clamp(0.2, 1.),
                });
            }
        }

        if input.pointer.is_none() {
            self.release();
        }
        let held = self.grab.map(|grab| grab.index);
        let steps = (dt / SUBSTEP).ceil().max(1.);
        let h = dt / steps;
        let taken = 1. - (-CATCH * h).exp();
        let flowed = 1. - (-FLOW * h).exp();
        let friction = (-FRICTION * h).exp();
        let returned = (-RETURN * h).exp();
        for _ in 0..steps as usize {
            for (index, drop) in DROPS.iter().enumerate() {
                let (x, y, reach) = input.drops[index];
                let step = scale(self.pending[index], taken);
                self.pending[index] = sub(self.pending[index], step);
                let mut offset = add(self.offsets[index], step);
                let mut velocity_here = self.velocities[index];
                self.areas[index] += (self.targets[index] - self.areas[index]) * flowed;

                if let Some(into) = self.into[index] {
                    // Fused: flow into the drop that took it.
                    let (tx, ty, _) = input.drops[into];
                    let there = add([tx, ty], self.offsets[into]);
                    let here = [x + offset[0], y + offset[1]];
                    self.offsets[index] = add(offset, scale(sub(there, here), flowed));
                    self.velocities[index] = [0., 0.];
                    continue;
                }

                if held == Some(index)
                    && let (Some(pointer), Some(grab)) = (input.pointer, self.grab)
                {
                    // In hand: follow the pointer, trailing a little.
                    let target = clamp(add(pointer, grab.hold), input.size);
                    let wanted = sub(target, [x, y]);
                    let acceleration = sub(
                        scale(sub(wanted, offset), HOLD),
                        scale(velocity_here, HOLD_DAMPING),
                    );
                    velocity_here = add(velocity_here, scale(acceleration, h));
                    self.velocities[index] = velocity_here;
                    self.offsets[index] = add(offset, scale(velocity_here, h));
                    continue;
                }

                // Where the drop rests: slides on after a fling and bounces off the edges once
                // handled, and goes home once let go.
                let mut anchor = self.anchors[index];
                if self.loose[index] {
                    let mut drift = self.drift[index];
                    anchor = add(anchor, scale(drift, h));
                    for axis in 0..2 {
                        let at = [x, y][axis] + anchor[axis];
                        let edge = at.clamp(0., input.size[axis]);
                        if at != edge {
                            anchor[axis] += edge - at;
                            drift[axis] *= -BOUNCE;
                        }
                    }
                    self.drift[index] = scale(drift, friction);
                } else {
                    anchor = scale(anchor, returned);
                }
                self.anchors[index] = anchor;
                let drift = self.drift[index];

                // Small drops are lighter and swing faster.
                let stiffness = SPRING * (0.12 / drop.radius).sqrt().clamp(0.7, 1.6);
                let mut acceleration = sub(
                    scale(sub(offset, anchor), -stiffness),
                    scale(sub(velocity_here, drift), DAMPING),
                );
                // While a drop is in hand the pointer leaves the others alone, so it can be
                // brought up to them.
                if let Some(pointer) = input.pointer.filter(|_| held.is_none()) {
                    let away = sub([x + offset[0], y + offset[1]], pointer);
                    let distance = length(away).max(1.);
                    let reach = POINTER_REACH * unit + reach * 0.5;
                    let falloff = (1. - distance / reach).max(0.).powi(2);
                    let push = scale(away, PUSH * unit * falloff / distance);
                    let stir = scale(velocity, STIR * falloff);
                    acceleration = add(acceleration, add(push, stir));
                }
                velocity_here = add(velocity_here, scale(acceleration, h));
                offset = add(offset, scale(velocity_here, h));
                let stray = sub(offset, anchor);
                if length(stray) > LIMIT * unit {
                    offset = add(anchor, scale(stray, LIMIT * unit / length(stray)));
                }
                self.velocities[index] = velocity_here;
                self.offsets[index] = offset;
            }
        }
        self.fuse(input.drops, unit);
        self.shed(input.drops, input.pointer, velocity, dt, unit);

        for wave in &mut self.waves {
            wave.age += dt;
        }
        self.waves.retain(|wave| wave.age < WAVE_LIFE);
        if self.moving() {
            self.clock += dt;
        }
    }

    /// Whether anything is still moving: a drop in hand, swinging, sliding, going home or
    /// flowing into another, or a ripple spreading.
    pub fn moving(&self) -> bool {
        self.grab.is_some()
            || !self.waves.is_empty()
            || self.pending.iter().any(|pending| length(*pending) > 0.5)
            || self
                .velocities
                .iter()
                .any(|velocity| length(*velocity) > 0.5)
            || self.drift.iter().any(|drift| length(*drift) > 0.5)
            || (self.anchors.iter().zip(&self.loose))
                .any(|(anchor, loose)| !loose && length(*anchor) > 0.5)
            || (self.areas.iter().zip(&self.targets))
                .any(|(area, target)| (area - target).abs() > 1e-6)
    }

    /// Whether a drop is in hand.
    pub fn holding(&self) -> bool {
        self.grab.is_some()
    }

    /// Whether `pointer` is over the metal, where a press takes hold of a drop.
    pub fn over_metal(&self, pointer: [f32; 2], drops: &[(f32, f32, f32)]) -> bool {
        let (field, _, _) = field_at(&self.drops(drops), pointer[0], pointer[1]);
        field > THRESHOLD * GRIP
    }

    /// Take hold of the drop under `pointer`, the one whose field there is strongest, if the
    /// pointer is over the metal. Whether it took hold.
    pub fn grab(&mut self, pointer: [f32; 2], drops: &[(f32, f32, f32)]) -> bool {
        if !self.over_metal(pointer, drops) {
            return false;
        }
        let strongest = self
            .placed(drops)
            .map(|(index, x, y, reach)| {
                let (field, _, _) = field_at(&[(x, y, reach)], pointer[0], pointer[1]);
                (index, [x, y], field)
            })
            .max_by(|a, b| a.2.total_cmp(&b.2));
        let Some((index, centre, _)) = strongest else {
            return false;
        };
        self.grab = Some(Grab {
            index,
            hold: sub(centre, pointer),
        });
        self.loose[index] = true;
        self.drift[index] = [0., 0.];
        let unit = self.size.map_or(1., |size| size[0].min(size[1]).max(1.));
        for other in 0..DROPS.len() {
            if other != index && self.touching(index, other, drops, unit) {
                self.joined[index][other] = true;
                self.joined[other][index] = true;
            }
        }
        true
    }

    /// Whether two drops are close enough to fuse.
    fn touching(&self, a: usize, b: usize, drops: &[(f32, f32, f32)], unit: f32) -> bool {
        let centre = |index: usize| {
            let (x, y, _) = drops[index];
            add([x, y], self.offsets[index])
        };
        let radii = (self.areas[a].sqrt() + self.areas[b].sqrt()) * unit;
        length(sub(centre(a), centre(b))) < FUSE * radii
    }

    /// Let go of the drop in hand: it rests where it is, sliding on if it was flung.
    pub fn release(&mut self) {
        let Some(Grab { index, .. }) = self.grab.take() else {
            return;
        };
        self.anchors[index] = self.offsets[index];
        let unit = self.size.map_or(1., |size| size[0].min(size[1]).max(1.));
        let velocity = self.velocities[index];
        let speed = length(velocity);
        self.drift[index] = if speed > FLING * unit {
            scale(velocity, FLING * unit / speed)
        } else {
            velocity
        };
    }

    /// Let every drop part from the ones it fused with and go home.
    pub fn scatter(&mut self) {
        self.grab = None;
        for (index, drop) in DROPS.iter().enumerate() {
            if self.into[index].take().is_some() {
                // Part from where it flowed to rather than jumping back.
                self.anchors[index] = self.offsets[index];
            }
            self.loose[index] = false;
            self.joined[index] = [false; DROPS.len()];
            self.drift[index] = [0., 0.];
            self.targets[index] = drop.radius * drop.radius;
        }
    }

    /// Each drop that is still there: its index, centre and reach, in pixels.
    fn placed<'a>(
        &'a self,
        drops: &'a [(f32, f32, f32)],
    ) -> impl Iterator<Item = (usize, f32, f32, f32)> + 'a {
        drops
            .iter()
            .zip(&self.offsets)
            .zip(DROPS.iter().zip(&self.areas))
            .enumerate()
            .map(|(index, ((&(x, y, reach), offset), (drop, area)))| {
                let grown = (area.max(0.)).sqrt() / drop.radius;
                (index, x + offset[0], y + offset[1], reach * grown)
            })
            .filter(|&(_, _, _, reach)| reach > 0.5)
    }

    /// Now and then let a fused drop shed one of the drops it took: rarely when left alone,
    /// often while the pointer brushes across it, and quickly while it is carried too fast. The
    /// shed drop slides off the way the pointer or the drop was going, or any way at all, and
    /// the two don't fuse again until they have parted.
    fn shed(
        &mut self,
        drops: &[(f32, f32, f32)],
        pointer: Option<[f32; 2]>,
        velocity: [f32; 2],
        dt: f32,
        unit: f32,
    ) {
        let held = self.grab.map(|grab| grab.index);
        let brushing = length(velocity) > BRUSH_SPEED * unit;
        for (taker, &(x, y, _)) in drops.iter().enumerate() {
            if self.into[taker].is_some() {
                continue;
            }
            let taken: Vec<_> = (0..DROPS.len())
                .filter(|&index| self.into[index] == Some(taker))
                .collect();
            if taken.is_empty() {
                continue;
            }
            let centre = add([x, y], self.offsets[taker]);
            let radius = self.areas[taker].sqrt() * unit;
            let brushed =
                brushing && pointer.is_some_and(|pointer| length(sub(pointer, centre)) < radius);
            let carried = self.velocities[taker];
            let torn = held == Some(taker) && length(carried) > TEAR_SPEED * unit;
            let rate = if torn {
                TEAR
            } else if held == Some(taker) {
                0.
            } else if brushed {
                BRUSH
            } else {
                1. / SHED_EVERY
            };
            if self.roll() >= rate * dt {
                continue;
            }
            let index = taken[(self.roll() * taken.len() as f32) as usize % taken.len()];
            let direction = if torn {
                let angle = carried[1].atan2(carried[0]) + (self.roll() - 0.5) * 2. * TEAR_SPREAD;
                [angle.cos(), angle.sin()]
            } else if brushed {
                scale(velocity, 1. / length(velocity))
            } else {
                let angle = self.roll() * std::f32::consts::TAU;
                [angle.cos(), angle.sin()]
            };
            let speed = if torn {
                (length(carried) * 0.5).min(FLING * unit)
            } else {
                SHED_SPEED * unit
            };
            let own = DROPS[index].radius * DROPS[index].radius;
            let share = own / self.targets[taker].max(own);
            #[cfg(test)]
            {
                self.sheds += 1;
            }
            self.into[index] = None;
            self.targets[index] = own;
            self.targets[taker] = (self.targets[taker] - own).max(0.);
            self.loose[index] = true;
            self.joined[index][taker] = true;
            self.joined[taker][index] = true;
            self.anchors[index] = self.offsets[index];
            self.drift[index] = scale(direction, speed);
            self.velocities[index] = self.drift[index];
            // The rest of the drop recoils a little, unless it is in hand.
            if held != Some(taker) {
                self.drift[taker] = sub(self.drift[taker], scale(direction, speed * share));
            }
        }
    }

    /// A number from 0 to 1, from a small xorshift generator.
    fn roll(&mut self) -> f32 {
        let mut x = self.dice;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.dice = x;
        (x >> 8) as f32 / (1 << 24) as f32
    }

    /// Fuse each handled drop with the drops it touches: the one in hand, or else the larger,
    /// takes the other's area and moves towards their common centre.
    fn fuse(&mut self, drops: &[(f32, f32, f32)], unit: f32) {
        let held = self.grab.map(|grab| grab.index);
        let centre = |sim: &Self, index: usize| {
            let (x, y, _) = drops[index];
            add([x, y], sim.offsets[index])
        };
        let whole = |sim: &Self, index: usize| sim.into[index].is_none() && sim.targets[index] > 0.;
        for first in 0..DROPS.len() {
            for second in 0..DROPS.len() {
                if first == second
                    || !self.loose[first]
                    || !whole(self, first)
                    || !whole(self, second)
                {
                    continue;
                }
                if !self.touching(first, second, drops, unit) {
                    self.joined[first][second] = false;
                    self.joined[second][first] = false;
                    continue;
                }
                if self.joined[first][second] {
                    continue;
                }
                let (taker, taken) = if held == Some(second)
                    || (held != Some(first) && self.targets[second] > self.targets[first])
                {
                    (second, first)
                } else {
                    (first, second)
                };
                let (a, b) = (self.targets[taker], self.targets[taken]);
                if held != Some(taker) {
                    let towards = sub(centre(self, taken), centre(self, taker));
                    self.anchors[taker] = add(self.anchors[taker], scale(towards, b / (a + b)));
                }
                self.drift[taker] = scale(
                    add(scale(self.drift[taker], a), scale(self.drift[taken], b)),
                    1. / (a + b),
                );
                self.targets[taker] = a + b;
                self.targets[taken] = 0.;
                self.loose[taker] = true;
                self.into[taken] = Some(taker);
                for into in &mut self.into {
                    if *into == Some(taken) {
                        *into = Some(taker);
                    }
                }
            }
        }
    }

    /// Seconds simulated while the liquid was moving.
    pub fn clock(&self) -> f32 {
        self.clock
    }

    /// The drops where they are now: `drops` from their orbits, moved by the response and
    /// grown or shrunk by fusing. A drop that has flowed wholly into another is left out.
    pub fn drops(&self, drops: &[(f32, f32, f32)]) -> Vec<(f32, f32, f32)> {
        self.placed(drops)
            .map(|(_, x, y, reach)| (x, y, reach))
            .collect()
    }

    /// Each ripple's centre, how far it has spread, and how strong it still is (0 to 1), in a
    /// box whose shorter side is `unit`.
    pub fn waves(&self, unit: f32) -> impl Iterator<Item = [f32; 4]> + '_ {
        self.waves.iter().map(move |wave| {
            let life = wave.age / WAVE_LIFE;
            [
                wave.centre[0],
                wave.centre[1],
                WAVE_SPEED * unit * wave.age,
                wave.strength * (1. - life) * (1. - life),
            ]
        })
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

/// `a` kept within a box of `size`.
fn clamp(a: [f32; 2], size: [f32; 2]) -> [f32; 2] {
    [a[0].clamp(0., size[0]), a[1].clamp(0., size[1])]
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::metal::drops_at;

    fn frames(
        sim: &mut Sim,
        start: Instant,
        from: usize,
        count: usize,
        input: impl Fn(usize) -> ([f32; 2], Option<[f32; 2]>),
    ) {
        let drops = drops_at(1000., 600., 0.);
        for frame in from..from + count {
            let (origin, pointer) = input(frame);
            sim.step(Input {
                now: start + Duration::from_millis(16 * frame as u64),
                origin,
                size: [1000., 600.],
                pointer,
                drops: &drops,
            });
        }
    }

    #[test]
    fn a_moved_window_sloshes_and_settles() {
        let mut sim = Sim::default();
        let start = Instant::now();
        frames(&mut sim, start, 0, 2, |_| ([0., 0.], None));
        assert!(!sim.moving());
        frames(&mut sim, start, 2, 1, |_| ([80., 0.], None));
        assert!(sim.moving(), "the liquid is left behind");
        assert!(sim.offsets.iter().all(|offset| offset[0] < 0.));
        frames(&mut sim, start, 3, 600, |_| ([80., 0.], None));
        assert!(!sim.moving(), "and settles back in under ten seconds");
    }

    #[test]
    fn a_window_moved_in_uneven_steps_sloshes_smoothly() {
        // A drag reported every fourth frame, 40 pixels at a time, as macOS can. Taking each
        // report whole would jump the drops 32 pixels in one frame and not at all in the next.
        let mut sim = Sim::default();
        let start = Instant::now();
        let mut places = Vec::new();
        for frame in 0..40 {
            frames(&mut sim, start, frame, 1, |frame| {
                ([40. * (frame / 4) as f32, 0.], None)
            });
            places.push(sim.offsets[0][0]);
        }
        let jolt = places
            .windows(3)
            .map(|w| (w[2] - 2. * w[1] + w[0]).abs())
            .fold(0., f32::max);
        assert!(
            jolt < 8.,
            "the drops' speed never changes by much at once: {jolt} px"
        );
    }

    #[test]
    fn the_pointer_pushes_drops_aside_and_leaves_ripples() {
        let mut sim = Sim::default();
        let start = Instant::now();
        let drops = drops_at(1000., 600., 0.);
        // Sweep across the large pool, upper right.
        let (x, y, _) = drops[0];
        frames(&mut sim, start, 0, 30, |frame| {
            ([0., 0.], Some([x - 150. + 10. * frame as f32, y]))
        });
        assert!(sim.moving());
        assert!(sim.waves(600.).count() > 0);
        assert!(sim.offsets.iter().any(|offset| length(*offset) > 5.));
    }

    #[test]
    fn a_drop_in_hand_follows_and_stays_where_it_is_put() {
        let mut sim = Sim::default();
        let start = Instant::now();
        let drops = drops_at(1000., 600., 0.);
        // The lone stray, upper left.
        let (x, y, _) = drops[11];
        frames(&mut sim, start, 0, 2, |_| ([0., 0.], Some([x, y])));
        assert!(sim.grab([x, y], &drops));
        assert_eq!(sim.grab.map(|grab| grab.index), Some(11));
        frames(&mut sim, start, 2, 60, |frame| {
            let t = ((frame - 2) as f32 / 30.).min(1.);
            ([0., 0.], Some([x + 200. * t, y + 100. * t]))
        });
        let place = |sim: &Sim| add([x, y], sim.offsets[11]);
        assert!(length(sub(place(&sim), [x + 200., y + 100.])) < 5.);
        sim.release();
        frames(&mut sim, start, 62, 600, |_| ([0., 0.], None));
        assert!(!sim.moving(), "it settles");
        assert!(
            length(sub(place(&sim), [x + 200., y + 100.])) < 10.,
            "where it was put, not back on its orbit"
        );
    }

    #[test]
    fn drops_brought_together_fuse_into_one() {
        let mut sim = Sim::default();
        let start = Instant::now();
        let drops = drops_at(1000., 600., 0.);
        // Carry the lone stray, upper left, onto the pool lower left.
        let (x, y, _) = drops[11];
        let (tx, ty, _) = drops[5];
        let before: f32 = sim.targets.iter().sum();
        frames(&mut sim, start, 0, 1, |_| ([0., 0.], Some([x, y])));
        assert!(sim.grab([x, y], &drops));
        frames(&mut sim, start, 1, 90, |frame| {
            let t = ((frame - 1) as f32 / 60.).min(1.);
            ([0., 0.], Some([x + (tx - x) * t, y + (ty - y) * t]))
        });
        sim.release();
        frames(&mut sim, start, 91, 600, |_| ([0., 0.], None));
        assert_eq!(
            sim.into[5],
            Some(11),
            "the pool flowed into the drop in hand"
        );
        assert!(sim.areas[5] < 1e-4);
        assert!(sim.areas[11] > 0.07f32.powi(2) + 0.17f32.powi(2) - 1e-4);
        let after: f32 = sim.targets.iter().sum();
        assert!((before - after).abs() < 1e-6, "no metal is lost");
        assert!(sim.drops(&drops).len() < DROPS.len());

        sim.scatter();
        frames(&mut sim, start, 691, 900, |_| ([0., 0.], None));
        assert!(!sim.moving());
        assert_eq!(
            sim.drops(&drops).len(),
            DROPS.len(),
            "a double click parts them"
        );
        assert!(sim.offsets.iter().all(|offset| length(*offset) < 2.));
    }

    #[test]
    fn a_press_off_the_metal_takes_nothing() {
        let mut sim = Sim::default();
        let drops = drops_at(1000., 600., 0.);
        assert!(!sim.grab([500., 300.], &drops));
        assert!(!sim.holding());
    }

    #[test]
    fn a_drop_taken_from_its_pool_does_not_swallow_it() {
        let mut sim = Sim::default();
        let start = Instant::now();
        let drops = drops_at(1000., 600., 0.);
        // The large pool, upper right.
        let (x, y, _) = drops[0];
        frames(&mut sim, start, 0, 1, |_| ([0., 0.], Some([x, y])));
        assert!(sim.grab([x, y], &drops));
        frames(&mut sim, start, 1, 30, |_| ([0., 0.], Some([x, y])));
        assert!(sim.into.iter().all(Option::is_none));
    }

    /// Carry the lone stray, upper left, onto the pool lower left and let go: a fused drop.
    fn fused(sim: &mut Sim, start: Instant) -> usize {
        let drops = drops_at(1000., 600., 0.);
        let (x, y, _) = drops[11];
        let (tx, ty, _) = drops[5];
        frames(sim, start, 0, 1, |_| ([0., 0.], Some([x, y])));
        assert!(sim.grab([x, y], &drops));
        frames(sim, start, 1, 90, |frame| {
            let t = ((frame - 1) as f32 / 60.).min(1.);
            ([0., 0.], Some([x + (tx - x) * t, y + (ty - y) * t]))
        });
        sim.release();
        frames(sim, start, 91, 120, |_| ([0., 0.], None));
        sim.into.iter().filter(|into| into.is_some()).count()
    }

    #[test]
    fn a_fused_drop_sheds_a_drop_now_and_then() {
        let mut sim = Sim::default();
        let start = Instant::now();
        let fused = fused(&mut sim, start);
        assert!(fused > 0);
        // Two minutes left alone.
        frames(&mut sim, start, 211, 7500, |_| ([0., 0.], None));
        assert!(sim.sheds > 0, "a drop came loose");
        let total: f32 = sim.targets.iter().sum();
        let whole: f32 = DROPS.iter().map(|drop| drop.radius * drop.radius).sum();
        assert!((total - whole).abs() < 1e-5, "no metal is lost or made");
    }

    #[test]
    fn brushing_a_fused_drop_shakes_one_loose() {
        let mut sim = Sim::default();
        let start = Instant::now();
        let fused = fused(&mut sim, start);
        let drops = drops_at(1000., 600., 0.);
        let taker = sim.into.iter().flatten().copied().next().unwrap();
        let (x, y, _) = drops[taker];
        let [x, y] = add([x, y], sim.offsets[taker]);
        // Back and forth across it, quickly, for a second.
        assert!(fused > 0);
        frames(&mut sim, start, 211, 60, |frame| {
            let swing = ((frame % 20) as f32 - 10.) * 8.;
            ([0., 0.], Some([x + swing, y]))
        });
        assert!(sim.sheds > 0, "a drop was shaken loose");
    }

    #[test]
    fn a_fused_drop_carried_too_fast_tears_apart() {
        let mut sim = Sim::default();
        let start = Instant::now();
        let fused = fused(&mut sim, start);
        let drops = drops_at(1000., 600., 0.);
        let taker = sim.into.iter().flatten().copied().next().unwrap();
        let (x, y, _) = drops[taker];
        let [x, y] = add([x, y], sim.offsets[taker]);
        frames(&mut sim, start, 211, 1, |_| ([0., 0.], Some([x, y])));
        assert!(sim.grab([x, y], &drops));
        assert_eq!(sim.grab.map(|grab| grab.index), Some(taker));
        // Carried gently, it holds together.
        frames(&mut sim, start, 212, 60, |frame| {
            ([0., 0.], Some([x + (frame - 212) as f32, y]))
        });
        assert!(fused > 0);
        assert_eq!(sim.sheds, 0, "carried gently, it holds together");
        // Whipped back and forth across the window, it comes apart.
        frames(&mut sim, start, 272, 40, |frame| {
            let side = if (frame / 6) % 2 == 0 { 700. } else { 100. };
            ([0., 0.], Some([side, y]))
        });
        assert!(sim.sheds > 0, "it tore");
    }
}
