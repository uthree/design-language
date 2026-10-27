//! How an ambient backdrop answers the pointer and the window.
//!
//! The scene hangs on a spring from where it belongs: moving or resizing the window leaves it
//! behind for a moment and it swings back, which each backdrop reads as parallax, nearer layers
//! swinging further. A move is let out over a few frames rather than all at once: during a drag
//! macOS reports the window's place in uneven steps, and taking each step whole made the scene
//! jump with it. The pointer is followed with a little lag and fades in and out as it
//! enters and leaves, so whatever it touches eases rather than jumps; its speed is kept, smoothed,
//! for what it drags along; and as it travels it leaves ripples that spread and fade.
//!
//! Distances here are logical pixels; rates are per second.

use std::time::Instant;

/// How stiffly the scene is pulled back after the window moves.
const SPRING: f32 = 18.;
/// How quickly its swing dies away.
const DAMPING: f32 = 4.2;
/// How much of a move of the window the scene is left behind by.
const LAG: f32 = 0.6;
/// How quickly a move of the window is let out into the swing, per second.
const CATCH: f32 = 14.;
/// The furthest the scene is let swing, in units of the box's shorter side.
const LIMIT: f32 = 0.2;
/// How quickly the eased pointer catches up with the real one.
const FOLLOW: f32 = 7.;
/// How quickly the pointer fades in and out.
const FADE: f32 = 2.5;
/// How quickly the pointer's smoothed speed follows its real one.
const SPEED_FOLLOW: f32 = 5.;
/// Pointer travel between one ripple and the next, in units.
const WAVE_SPACING: f32 = 0.09;
/// How fast a ripple spreads, in units per second.
const WAVE_SPEED: f32 = 0.32;
/// How long a ripple lasts, in seconds.
const WAVE_LIFE: f32 = 3.;
/// The most ripples at once.
pub(super) const WAVES: usize = 8;
/// The longest step simulated at once; a longer gap between frames is split.
const SUBSTEP: f32 = 1. / 120.;

/// The backdrop's response so far.
#[derive(Default)]
pub(super) struct Sim {
    sway: [f32; 2],
    swing: [f32; 2],
    /// What the window's moves have yet to leave behind.
    pending: [f32; 2],
    focus: [f32; 2],
    presence: f32,
    velocity: [f32; 2],
    waves: Vec<Wave>,
    last: Option<Instant>,
    origin: Option<[f32; 2]>,
    size: Option<[f32; 2]>,
    pointer: Option<[f32; 2]>,
    travel: f32,
    /// Seconds simulated while the backdrop was moving.
    clock: f32,
}

struct Wave {
    centre: [f32; 2],
    age: f32,
    strength: f32,
}

/// What the backdrop sees this frame.
pub(super) struct Input {
    pub now: Instant,
    /// The window's position on the screen.
    pub origin: [f32; 2],
    /// The backdrop's size.
    pub size: [f32; 2],
    /// The pointer, relative to the backdrop, while it is over it.
    pub pointer: Option<[f32; 2]>,
}

/// The response as the shaders take it, in units of the box's shorter side from its centre.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Response {
    pub pointer: [f32; 2],
    pub presence: f32,
    pub velocity: [f32; 2],
    pub sway: [f32; 2],
    /// Each ripple's centre, how far it has spread and how strong it still is.
    pub waves: [[f32; 4]; WAVES],
    pub wave_count: usize,
}

impl Sim {
    pub fn step(&mut self, input: Input) {
        let dt = self
            .last
            .map_or(0., |last| (input.now - last).as_secs_f32().min(0.1));
        self.last = Some(input.now);
        let unit = input.size[0].min(input.size[1]).max(1.);

        // A moved window leaves the scene behind; a resized one leaves it off centre.
        if let Some(origin) = self.origin.replace(input.origin) {
            self.pending = sub(self.pending, scale(sub(input.origin, origin), LAG));
        }
        if let Some(size) = self.size.replace(input.size) {
            self.pending = sub(self.pending, scale(sub(input.size, size), 0.5 * LAG));
        }

        // The pointer: its speed, the ripples along its path, and where it is, eased.
        let previous = std::mem::replace(&mut self.pointer, input.pointer);
        let speed = match (previous, input.pointer) {
            (Some(from), Some(to)) if dt > 0. => scale(sub(to, from), 1. / dt),
            _ => [0., 0.],
        };
        if let (Some(from), Some(to)) = (previous, input.pointer) {
            self.travel += length(sub(to, from));
            if self.travel > WAVE_SPACING * unit {
                self.travel = 0.;
                if self.waves.len() == WAVES {
                    self.waves.remove(0);
                }
                self.waves.push(Wave {
                    centre: to,
                    age: 0.,
                    strength: (length(speed) / (1.2 * unit)).clamp(0.25, 1.),
                });
            }
        }
        if let Some(pointer) = input.pointer {
            if self.presence < 0.01 {
                self.focus = pointer;
            }
            self.focus = approach(self.focus, pointer, FOLLOW, dt);
        }
        let present = if input.pointer.is_some() { 1. } else { 0. };
        self.presence += (present - self.presence) * (1. - (-FADE * dt).exp());
        if (present - self.presence).abs() < 0.002 {
            self.presence = present;
        }
        self.velocity = approach(self.velocity, speed, SPEED_FOLLOW, dt);
        if length(self.velocity) < 1. {
            self.velocity = [0., 0.];
        }

        // The swing back.
        let steps = (dt / SUBSTEP).ceil().max(1.);
        let h = dt / steps;
        let taken = 1. - (-CATCH * h).exp();
        for _ in 0..steps as usize {
            let step = scale(self.pending, taken);
            self.pending = sub(self.pending, step);
            self.sway = add(self.sway, step);
            let acceleration = sub(scale(self.sway, -SPRING), scale(self.swing, DAMPING));
            self.swing = add(self.swing, scale(acceleration, h));
            self.sway = add(self.sway, scale(self.swing, h));
        }
        let stray = length(self.sway);
        if stray > LIMIT * unit {
            self.sway = scale(self.sway, LIMIT * unit / stray);
        }
        if length(self.sway) < 0.2 && length(self.swing) < 0.5 && length(self.pending) < 0.2 {
            self.sway = [0., 0.];
            self.swing = [0., 0.];
            self.pending = [0., 0.];
        }

        for wave in &mut self.waves {
            wave.age += dt;
        }
        self.waves.retain(|wave| wave.age < WAVE_LIFE);
        if self.moving() {
            self.clock += dt;
        }
    }

    /// Whether anything is still moving: the scene swinging, the pointer easing or fading, or a
    /// ripple spreading.
    pub fn moving(&self) -> bool {
        !self.waves.is_empty()
            || self.sway != [0., 0.]
            || self.pending != [0., 0.]
            || self.velocity != [0., 0.]
            || (self.presence > 0. && self.presence < 1.)
            || self
                .pointer
                .is_some_and(|pointer| length(sub(pointer, self.focus)) > 0.5)
    }

    /// Seconds simulated while the backdrop was moving.
    pub fn clock(&self) -> f32 {
        self.clock
    }

    /// The response for a box of `size`, in the shaders' units.
    pub fn response(&self, size: [f32; 2]) -> Response {
        let unit = size[0].min(size[1]).max(1.);
        let place = |point: [f32; 2]| {
            [
                (point[0] - size[0] * 0.5) / unit,
                (point[1] - size[1] * 0.5) / unit,
            ]
        };
        let mut response = Response {
            pointer: place(self.focus),
            presence: self.presence,
            velocity: scale(self.velocity, 1. / unit),
            sway: scale(self.sway, 1. / unit),
            ..Default::default()
        };
        for (slot, wave) in response.waves.iter_mut().zip(&self.waves) {
            let life = wave.age / WAVE_LIFE;
            let [x, y] = place(wave.centre);
            *slot = [
                x,
                y,
                WAVE_SPEED * wave.age,
                wave.strength * (1. - life) * (1. - life),
            ];
        }
        response.wave_count = self.waves.len();
        response
    }
}

/// `from` moved towards `to` at `rate` for `dt` seconds.
fn approach(from: [f32; 2], to: [f32; 2], rate: f32, dt: f32) -> [f32; 2] {
    add(from, scale(sub(to, from), 1. - (-rate * dt).exp()))
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

    fn frames(
        sim: &mut Sim,
        start: Instant,
        from: usize,
        count: usize,
        input: impl Fn(usize) -> ([f32; 2], Option<[f32; 2]>),
    ) {
        for frame in from..from + count {
            let (origin, pointer) = input(frame);
            sim.step(Input {
                now: start + Duration::from_millis(16 * frame as u64),
                origin,
                size: [1000., 600.],
                pointer,
            });
        }
    }

    #[test]
    fn a_moved_window_swings_and_settles() {
        let mut sim = Sim::default();
        let start = Instant::now();
        frames(&mut sim, start, 0, 2, |_| ([0., 0.], None));
        assert!(!sim.moving());
        frames(&mut sim, start, 2, 1, |_| ([80., 0.], None));
        assert!(sim.moving(), "the scene is left behind");
        assert!(sim.response([1000., 600.]).sway[0] < 0.);
        frames(&mut sim, start, 3, 600, |_| ([80., 0.], None));
        assert!(!sim.moving(), "and settles back in under ten seconds");
    }

    #[test]
    fn a_window_moved_in_uneven_steps_swings_smoothly() {
        // A drag reported every fourth frame, 40 pixels at a time, as macOS can. Taking each
        // report whole would jump the scene 24 pixels in one frame and not at all in the next.
        let mut sim = Sim::default();
        let start = Instant::now();
        let mut places = Vec::new();
        for frame in 0..40 {
            frames(&mut sim, start, frame, 1, |frame| {
                ([40. * (frame / 4) as f32, 0.], None)
            });
            places.push(sim.response([1000., 600.]).sway[0] * 600.);
        }
        let jolt = places
            .windows(3)
            .map(|w| (w[2] - 2. * w[1] + w[0]).abs())
            .fold(0., f32::max);
        assert!(
            jolt < 8.,
            "the scene's speed never changes by much at once: {jolt} px"
        );
    }

    #[test]
    fn the_pointer_fades_in_leaves_ripples_and_settles() {
        let mut sim = Sim::default();
        let start = Instant::now();
        frames(&mut sim, start, 0, 40, |frame| {
            ([0., 0.], Some([200. + 12. * frame as f32, 300.]))
        });
        let response = sim.response([1000., 600.]);
        assert!(sim.moving());
        assert!(response.presence > 0.5);
        assert!(response.wave_count > 0);
        assert!(response.velocity[0] > 0.5, "it keeps the pointer's speed");
        // Resting over the backdrop, it comes to rest too.
        frames(&mut sim, start, 40, 600, |_| ([0., 0.], Some([680., 300.])));
        assert!(!sim.moving());
        let response = sim.response([1000., 600.]);
        assert!((response.pointer[0] - 0.3).abs() < 0.01);
        // And it fades out once the pointer leaves.
        frames(&mut sim, start, 640, 600, |_| ([0., 0.], None));
        assert_eq!(sim.response([1000., 600.]).presence, 0.);
        assert!(!sim.moving());
    }
}
