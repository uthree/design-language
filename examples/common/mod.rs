//! Start-up shared by the examples: the theme from the command line, a custom-framed window,
//! and the direction/appearance switch each title bar carries.
//!
//! Every example accepts `--direction glass|studio|mono`, `--appearance dark|light` and
//! `--size WIDTHxHEIGHT` for the window's starting size.

#![allow(dead_code)]

use atelier::{Appearance, Assets, ControlSize, Direction, Segment, SegmentedControl, Theme};
use gpui::{
    App, AppContext, Application, Bounds, Context, IntoElement, ParentElement, Render, Styled,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, px, size,
};

/// The smallest window every example lays out without overlapping or cutting off its controls.
pub const MIN_WIDTH: f32 = 900.;
/// See [`MIN_WIDTH`].
pub const MIN_HEIGHT: f32 = 600.;

/// The value after `name` on the command line.
pub fn argument(name: &str) -> Option<String> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }
    None
}

/// The direction and appearance asked for on the command line, Glass dark by default.
pub fn choice_from_args() -> (Direction, Appearance) {
    let direction = argument("--direction")
        .and_then(|id| Direction::from_id(&id))
        .unwrap_or_default();
    let appearance = match argument("--appearance").as_deref() {
        Some("light") => Appearance::Light,
        _ => Appearance::Dark,
    };
    (direction, appearance)
}

/// The window size asked for with `--size WIDTHxHEIGHT`, or `width` × `height`.
pub fn size_from_args(width: f32, height: f32) -> (f32, f32) {
    argument("--size")
        .and_then(|value| {
            let (width, height) = value.split_once('x')?;
            Some((width.parse().ok()?, height.parse().ok()?))
        })
        .unwrap_or((width, height))
}

/// A view whose theme the title-bar switch can change.
pub trait Themed: Render + Sized + 'static {
    /// The current choice.
    fn choice(&mut self) -> &mut (Direction, Appearance);
}

/// Install the theme for `choice` and let the window follow its material.
pub fn apply(choice: (Direction, Appearance), window: &mut Window, cx: &mut App) {
    let theme = Theme::new(choice.0, choice.1);
    atelier::set_window_background(window, &theme);
    theme.install(cx);
}

/// Two small segmented controls: the direction and the appearance.
pub fn theme_switch<V: Themed>(view: &mut V, cx: &mut Context<V>) -> impl IntoElement + use<V> {
    let (direction, appearance) = *view.choice();
    let index = Direction::ALL
        .iter()
        .position(|candidate| *candidate == direction)
        .unwrap_or(0);
    div()
        .flex()
        .gap(px(6.))
        .child(
            SegmentedControl::new(
                "theme-direction",
                Direction::ALL.map(|direction| Segment::label(direction.label())),
                index,
            )
            .size(ControlSize::Small)
            .on_select(cx.listener(|this: &mut V, index: &usize, window, cx| {
                this.choice().0 = Direction::ALL[*index];
                apply(*this.choice(), window, cx);
            })),
        )
        .child(
            SegmentedControl::new(
                "theme-appearance",
                [Segment::label("Light"), Segment::label("Dark")],
                if appearance.is_dark() { 1 } else { 0 },
            )
            .size(ControlSize::Small)
            .on_select(cx.listener(|this: &mut V, index: &usize, window, cx| {
                this.choice().1 = Appearance::ALL[*index];
                apply(*this.choice(), window, cx);
            })),
        )
}

/// Run an application with one custom-framed window holding the view `build` makes.
pub fn run<V: Themed>(
    title: &'static str,
    width: f32,
    height: f32,
    build: impl FnOnce((Direction, Appearance), &mut Context<V>) -> V + 'static,
) {
    let choice = choice_from_args();
    let (width, height) = size_from_args(width, height);
    Application::new()
        .with_assets(Assets::default())
        .run(move |cx: &mut App| {
            let theme = Theme::new(choice.0, choice.1);
            let background = theme.window_background();
            atelier::init(cx, theme);
            let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            let options = atelier::custom_frame(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(MIN_WIDTH), px(MIN_HEIGHT))),
                    window_background: background,
                    titlebar: Some(TitlebarOptions {
                        title: Some(title.into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                &Theme::new(choice.0, choice.1),
            );
            cx.open_window(options, |window, cx| {
                atelier::set_window_background(window, &Theme::new(choice.0, choice.1));
                cx.new(|cx| build(choice, cx))
            })
            .expect("failed to open the window");
            cx.activate(true);
        });
}

/// Readouts for the sample sliders and knobs, from their value of 0 to 1. Demo data: an
/// application formats its own units.
pub mod readout {
    use gpui::SharedString;

    /// "62 %".
    pub fn percent(value: f32) -> SharedString {
        format!("{:.0} %", value * 100.).into()
    }

    /// "0.62".
    pub fn fraction(value: f32) -> SharedString {
        format!("{value:.2}").into()
    }

    /// A fader: 0 dB at three quarters, −∞ at the bottom, +6 dB at the top.
    pub fn volume_db(value: f32) -> SharedString {
        if value <= 0.001 {
            return "−∞ dB".into();
        }
        let db = if value >= 0.75 {
            (value - 0.75) * 24.
        } else {
            60. * (value / 0.75).log10()
        };
        signed(db, 1, " dB")
    }

    /// A gain of ±12 dB around the centre.
    pub fn gain_db(value: f32) -> SharedString {
        signed((value - 0.5) * 24., 1, "")
    }

    /// Pan from L 50 through C to R 50.
    pub fn pan(value: f32) -> SharedString {
        let amount = ((value - 0.5) * 100.).round() as i32;
        match amount {
            0 => "C".into(),
            amount if amount < 0 => format!("L {}", -amount).into(),
            amount => format!("R {amount}").into(),
        }
    }

    /// `amount` with an explicit sign and a true minus, to `decimals` places.
    pub fn signed(amount: f32, decimals: usize, unit: &str) -> SharedString {
        let scale = 10f32.powi(decimals as i32);
        let amount = (amount * scale).round() / scale;
        let sign = if amount > 0. {
            "+"
        } else if amount < 0. {
            "−"
        } else {
            ""
        };
        format!("{sign}{:.decimals$}{unit}", amount.abs()).into()
    }
}
