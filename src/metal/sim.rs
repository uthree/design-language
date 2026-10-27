//! How the liquid answers the pointer and the window.
//!
//! Each drop hangs on a spring from its place on its orbit. The pointer pushes nearby drops aside
//! and drags them along as it passes, so pools stretch, part and close behind it; moving or
//! resizing the window leaves the liquid behind for a moment, and it sloshes back. Drops of
//! different sizes swing at different rates, so a pool deforms rather than moving as one. Where
//! the pointer crosses the metal it leaves ripples that run outwards and fade.
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

/// The liquid's response so far.
#[derive(Default)]
pub(super) struct Sim {
    offsets: [[f32; 2]; DROPS.len()],
    velocities: [[f32; 2]; DROPS.len()],
    waves: Vec<Wave>,
    last: Option<Instant>,
    origin: Option<[f32; 2]>,
    size: Option<[f32; 2]>,
    pointer: Option<[f32; 2]>,
    travel: f32,
    /// Seconds simulated while the liquid was moving.
    clock: f32,
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
    /// The pointer, relative to the backdrop, while it is over the window.
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
            for offset in &mut self.offsets {
                *offset = sub(*offset, scale(moved, LAG));
            }
        }
        if let Some(size) = self.size.replace(input.size) {
            let grown = sub(input.size, size);
            for (offset, drop) in self.offsets.iter_mut().zip(&DROPS) {
                *offset = sub(*offset, scale([grown[0] * drop.x, grown[1] * drop.y], LAG));
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

        let steps = (dt / SUBSTEP).ceil().max(1.);
        let h = dt / steps;
        for _ in 0..steps as usize {
            for (index, drop) in DROPS.iter().enumerate() {
                let (x, y, reach) = input.drops[index];
                let offset = self.offsets[index];
                let velocity_here = self.velocities[index];
                // Small drops are lighter and swing faster.
                let stiffness = SPRING * (0.12 / drop.radius).sqrt().clamp(0.7, 1.6);
                let mut acceleration =
                    sub(scale(offset, -stiffness), scale(velocity_here, DAMPING));
                if let Some(pointer) = input.pointer {
                    let away = sub([x + offset[0], y + offset[1]], pointer);
                    let distance = length(away).max(1.);
                    let reach = POINTER_REACH * unit + reach * 0.5;
                    let falloff = (1. - distance / reach).max(0.).powi(2);
                    let push = scale(away, PUSH * unit * falloff / distance);
                    let stir = scale(velocity, STIR * falloff);
                    acceleration = add(acceleration, add(push, stir));
                }
                let velocity_here = add(velocity_here, scale(acceleration, h));
                let mut offset = add(offset, scale(velocity_here, h));
                let stray = length(offset);
                if stray > LIMIT * unit {
                    offset = scale(offset, LIMIT * unit / stray);
                }
                self.velocities[index] = velocity_here;
                self.offsets[index] = offset;
            }
        }

        for wave in &mut self.waves {
            wave.age += dt;
        }
        self.waves.retain(|wave| wave.age < WAVE_LIFE);
        if self.moving() {
            self.clock += dt;
        }
    }

    /// Whether anything is still moving: a drop swinging, or a ripple spreading.
    pub fn moving(&self) -> bool {
        !self.waves.is_empty()
            || self
                .velocities
                .iter()
                .any(|velocity| length(*velocity) > 0.5)
    }

    /// Seconds simulated while the liquid was moving.
    pub fn clock(&self) -> f32 {
        self.clock
    }

    /// The drops where they are now: `drops` from their orbits, moved by the response.
    pub fn drops(&self, drops: &[(f32, f32, f32)]) -> Vec<(f32, f32, f32)> {
        drops
            .iter()
            .zip(&self.offsets)
            .map(|(&(x, y, reach), offset)| (x + offset[0], y + offset[1], reach))
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
}
