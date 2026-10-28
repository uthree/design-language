//! The colours a welcome-screen backdrop — [`crate::MetalBackdrop`] or
//! [`crate::AmbientBackdrop`] — is painted in.
//!
//! By default a backdrop follows the theme: the agent gradient, warm to cool, and the accent. A
//! palette replaces those four colours, so a welcome screen can carry its own mood without
//! touching the rest of the interface. The appearance still decides how they are laid on: as
//! light over the dark, or as pastel washes over the light. The liquid metal stays metal: its body
//! is tinted only a little towards the palette, and the palette's warm and cool ends become the
//! coloured fringes at its horizon.

use std::time::{Duration, Instant};

use gpui::{Hsla, rgb};

use crate::theme::{Theme, mix};

/// The colours of a backdrop.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum BackdropPalette {
    /// The theme's agent gradient and accent.
    #[default]
    Theme,
    /// Amber to coral to violet, into a deep blue: the sky after sunset.
    Dusk,
    /// Sea glass to cyan to blue.
    Ocean,
    /// Gold to orange to crimson, into plum.
    Ember,
    /// Lime to green to teal.
    Meadow,
    /// Pearl to slate, no hue at all.
    Graphite,
    /// Four colours of your own, warm end first and the coolest last, as the presets run.
    Custom([Hsla; 4]),
}

impl BackdropPalette {
    /// The theme and every preset, in the order a picker should offer them.
    pub const ALL: [BackdropPalette; 6] = [
        BackdropPalette::Theme,
        BackdropPalette::Dusk,
        BackdropPalette::Ocean,
        BackdropPalette::Ember,
        BackdropPalette::Meadow,
        BackdropPalette::Graphite,
    ];

    /// The palette's name.
    pub fn label(self) -> &'static str {
        match self {
            BackdropPalette::Theme => "Theme",
            BackdropPalette::Dusk => "Dusk",
            BackdropPalette::Ocean => "Ocean",
            BackdropPalette::Ember => "Ember",
            BackdropPalette::Meadow => "Meadow",
            BackdropPalette::Graphite => "Graphite",
            BackdropPalette::Custom(_) => "Custom",
        }
    }

    /// The four colours under `theme`, warm end first.
    pub fn hues(self, theme: &Theme) -> [Hsla; 4] {
        let seeds = match self {
            BackdropPalette::Theme => {
                let [a, b, c] = theme.colors.agent_gradient;
                return [a, b, c, theme.colors.accent];
            }
            BackdropPalette::Custom(hues) => return hues,
            BackdropPalette::Dusk => [0xffb36b, 0xff6f91, 0x9b6bff, 0x3d7bff],
            BackdropPalette::Ocean => [0x9ff7e0, 0x4fd8ff, 0x3f8cff, 0x2a5bea],
            BackdropPalette::Ember => [0xffd36b, 0xff8a3d, 0xe8455a, 0xa8326e],
            BackdropPalette::Meadow => [0xe6f27a, 0x8fe08a, 0x3fbf9a, 0x2a8fb0],
            BackdropPalette::Graphite => [0xe8e8ec, 0xb8bac2, 0x8a8d98, 0x5c6070],
        };
        seeds.map(|seed| rgb(seed).into())
    }

    /// The liquid metal's reflection ramp (see [`crate::Colors::metal`]) in this palette: the
    /// theme's ramp, its body tinted a little towards the palette and its fringes in the
    /// palette's warm and cool ends.
    pub fn metal(self, theme: &Theme) -> [Hsla; 9] {
        let ramp = theme.colors.metal;
        if self == BackdropPalette::Theme {
            return ramp;
        }
        let hues = self.hues(theme);
        let body = mix(hues[1], hues[2], 0.5);
        let mut ramp = ramp.map(|stop| tint(stop, body, 0.18));
        // The fringes keep the theme's lightness, so the horizon stays as crisp.
        ramp[3] = with_lightness(hues[0], ramp[3].l);
        ramp[5] = with_lightness(hues[3], ramp[5].l);
        ramp
    }
}

/// `stop` moved `amount` of the way to `hue`'s colour, softened, keeping its lightness.
fn tint(stop: Hsla, hue: Hsla, amount: f32) -> Hsla {
    let soft = Hsla {
        s: hue.s.min(0.55),
        l: stop.l,
        ..hue
    };
    with_lightness(mix(stop, soft, amount), stop.l)
}

fn with_lightness(color: Hsla, l: f32) -> Hsla {
    Hsla { l, ..color }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Appearance, Direction};
    use gpui::Rgba;

    fn chroma(color: Hsla) -> f32 {
        let Rgba { r, g, b, .. } = color.into();
        r.max(g).max(b) - r.min(g).min(b)
    }

    #[test]
    fn the_theme_palette_is_the_theme() {
        let theme = Theme::new(Direction::Glass, Appearance::Dark);
        assert_eq!(BackdropPalette::Theme.metal(&theme), theme.colors.metal);
        assert_eq!(BackdropPalette::Theme.hues(&theme)[3], theme.colors.accent);
    }

    #[test]
    fn metal_in_a_palette_keeps_its_horizon_and_stays_metal() {
        for direction in Direction::ALL {
            let theme = Theme::new(direction, Appearance::Dark);
            for palette in BackdropPalette::ALL {
                let metal = palette.metal(&theme);
                let horizon = metal[4].l;
                assert!(metal.iter().all(|stop| stop.l >= horizon - 1e-4));
                for (index, stop) in metal.iter().enumerate() {
                    if index != 3 && index != 5 {
                        assert!(
                            chroma(*stop) <= 0.16,
                            "{palette:?} {direction:?}: stop {index} stays near-neutral"
                        );
                    }
                }
            }
        }
    }
}

/// Where a backdrop is in its cycle: running from a moment and a phase, or stopped at a phase
/// while its window is inactive, so it carries on from there rather than jumping.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Clock {
    Running { since: Instant, from: f32 },
    Stopped(f32),
}

impl Clock {
    /// The phase, from 0 to 1, in a cycle of `period`.
    pub(crate) fn phase(self, period: Duration) -> f32 {
        match self {
            Self::Running { since, from } => {
                (from + since.elapsed().as_secs_f32() / period.as_secs_f32()).rem_euclid(1.)
            }
            Self::Stopped(phase) => phase,
        }
    }

    /// Run `clock` while `running` and stop it where it is otherwise, starting from `rest` the
    /// first time. Returns the clock as it now is.
    pub(crate) fn advance(
        clock: &mut Option<Self>,
        running: bool,
        rest: f32,
        period: Duration,
    ) -> Self {
        let now = match (clock.unwrap_or(Self::Stopped(rest)), running) {
            (Self::Stopped(from), true) => Self::Running {
                since: Instant::now(),
                from,
            },
            (running @ Self::Running { .. }, false) => Self::Stopped(running.phase(period)),
            (clock, _) => clock,
        };
        *clock = Some(now);
        now
    }
}
