//! Start-up shared by the examples: the theme from the command line, a custom-framed window,
//! and the direction/appearance switch each title bar carries.
//!
//! Every example accepts `--direction glass|studio|mono` and `--appearance dark|light`.

#![allow(dead_code)]

use atelier::{Appearance, Assets, ControlSize, Direction, Segment, SegmentedControl, Theme};
use gpui::{
    App, AppContext, Application, Bounds, Context, IntoElement, ParentElement, Render, Styled,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, px, size,
};

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
