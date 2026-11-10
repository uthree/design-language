//! Colours the person chooses: a swatch to show and pick one, and a picker to make one.
//!
//! These are document colours (a track's, a video's caption, a highlight), never the interface's
//! own, which come from the theme. [`ColorSwatch`] is the small square that shows a colour and
//! opens a picker or picks a preset; [`ColorPicker`] is the panel that makes one: the colour and
//! its code, which can be typed, three sliders for hue, saturation and brightness, and a row of
//! presets to start from.
//!
//! The picker keeps the colour as hue, saturation and value ([`Hsv`]) while it is open, because
//! RGB forgets what the hand has not let go of: drag the saturation to nothing and every hue is
//! the same grey, so a colour worked out again from that grey would come back red.

use std::rc::Rc;

use gpui::{
    App, AppContext, ClickEvent, ElementId, Entity, Hsla, InteractiveElement, IntoElement,
    ParentElement, Pixels, RenderOnce, Rgba, SharedString, StatefulInteractiveElement, Styled,
    Window, div, prelude::FluentBuilder, px,
};

use crate::button::ControlSize;
use crate::controls::{Slider, TextField};
use crate::focus::{focusable, selector};
use crate::input::{CancelText, SubmitText, TextInput};
use crate::overlay::Tooltip;
use crate::strings::ActiveStrings;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

/// A colour as 0xRRGGBB, each component rounded to the nearest step.
fn rgb_of(color: Hsla) -> u32 {
    let rgba = Rgba::from(color);
    let byte = |channel: f32| (channel.clamp(0., 1.) * 255.).round() as u32;
    (byte(rgba.r) << 16) | (byte(rgba.g) << 8) | byte(rgba.b)
}

/// The opaque colour 0xRRGGBB names.
fn color_of(rgb: u32) -> Hsla {
    gpui::rgb(rgb & 0x00ff_ffff).into()
}

/// A colour written as a code: `#3D7BFF`. Alpha is left out.
pub fn hex(color: Hsla) -> String {
    format!("#{:06X}", rgb_of(color))
}

/// Reads a colour code: six hex digits or the three-digit short form, with or without the `#`,
/// in either case, ignoring the space around it.
pub fn parse_hex(text: &str) -> Option<Hsla> {
    let digits = text.trim();
    let digits = digits.strip_prefix('#').unwrap_or(digits);
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match digits.len() {
        6 => u32::from_str_radix(digits, 16).ok().map(color_of),
        3 => {
            let short = u32::from_str_radix(digits, 16).ok()?;
            let (r, g, b) = ((short >> 8) & 0xf, (short >> 4) & 0xf, short & 0xf);
            Some(color_of((r * 0x11) << 16 | (g * 0x11) << 8 | (b * 0x11)))
        }
        _ => None,
    }
}

/// One of the three things a [`ColorPicker`]'s sliders move.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum HsvChannel {
    /// Which colour, round the wheel.
    Hue,
    /// How far from grey.
    Saturation,
    /// How far from black.
    Value,
}

impl HsvChannel {
    /// Every channel, in the order the sliders are drawn.
    pub const ALL: [HsvChannel; 3] = [HsvChannel::Hue, HsvChannel::Saturation, HsvChannel::Value];

    /// A stable name for element ids and test selectors.
    pub fn id(self) -> &'static str {
        match self {
            HsvChannel::Hue => "hue",
            HsvChannel::Saturation => "saturation",
            HsvChannel::Value => "value",
        }
    }
}

/// A colour as hue, saturation and value.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Hsv {
    /// Degrees, from 0 up to but not including 360.
    pub hue: f32,
    /// From 0 (grey) to 1 (pure).
    pub saturation: f32,
    /// From 0 (black) to 1 (full brightness).
    pub value: f32,
}

impl Hsv {
    /// The hue, saturation and value of a colour, read from its RGB rounded to whole steps.
    pub fn from_color(color: Hsla) -> Self {
        Self::from_rgb(rgb_of(color))
    }

    fn from_rgb(rgb: u32) -> Self {
        let channel = |shift: u32| ((rgb >> shift) & 0xff) as f32 / 255.;
        let (r, g, b) = (channel(16), channel(8), channel(0));
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let chroma = max - min;
        let hue = if chroma <= f32::EPSILON {
            0.
        } else if max == r {
            60. * ((g - b) / chroma).rem_euclid(6.)
        } else if max == g {
            60. * ((b - r) / chroma + 2.)
        } else {
            60. * ((r - g) / chroma + 4.)
        };
        Self {
            hue: hue.rem_euclid(360.),
            saturation: if max <= f32::EPSILON {
                0.
            } else {
                chroma / max
            },
            value: max,
        }
    }

    fn to_rgb(self) -> u32 {
        let hue = self.hue.rem_euclid(360.) / 60.;
        let saturation = self.saturation.clamp(0., 1.);
        let value = self.value.clamp(0., 1.);
        let chroma = value * saturation;
        let x = chroma * (1. - (hue.rem_euclid(2.) - 1.).abs());
        let (r, g, b) = match hue as u32 {
            0 => (chroma, x, 0.),
            1 => (x, chroma, 0.),
            2 => (0., chroma, x),
            3 => (0., x, chroma),
            4 => (x, 0., chroma),
            _ => (chroma, 0., x),
        };
        let m = value - chroma;
        let byte = |channel: f32| ((channel + m).clamp(0., 1.) * 255.).round() as u32;
        (byte(r) << 16) | (byte(g) << 8) | byte(b)
    }

    /// The opaque colour this names, each component rounded to the nearest step.
    pub fn to_color(self) -> Hsla {
        color_of(self.to_rgb())
    }

    /// Where `channel` sits on its slider, from 0 to 1.
    pub fn fraction(self, channel: HsvChannel) -> f32 {
        match channel {
            HsvChannel::Hue => self.hue.rem_euclid(360.) / 360.,
            HsvChannel::Saturation => self.saturation,
            HsvChannel::Value => self.value,
        }
        .clamp(0., 1.)
    }

    /// The same colour with `channel` moved to `fraction` of its slider. A hue at the very end
    /// is kept just short of 360° rather than wrapped to 0, so the thumb stays where the hand
    /// left it: both name the same red.
    pub fn with(self, channel: HsvChannel, fraction: f32) -> Self {
        let fraction = fraction.clamp(0., 1.);
        match channel {
            HsvChannel::Hue => Self {
                hue: (fraction * 360.).min(359.99),
                ..self
            },
            HsvChannel::Saturation => Self {
                saturation: fraction,
                ..self
            },
            HsvChannel::Value => Self {
                value: fraction,
                ..self
            },
        }
    }

    /// What the channel's slider reads: degrees for the hue, a percentage for the others.
    pub fn label(self, channel: HsvChannel) -> String {
        match channel {
            HsvChannel::Hue => format!("{:.0}°", self.hue.rem_euclid(360.)),
            HsvChannel::Saturation => format!("{:.0}%", self.saturation * 100.),
            HsvChannel::Value => format!("{:.0}%", self.value * 100.),
        }
    }

    /// The colour a channel's slider is filled with: the hue at full strength for the hue, so
    /// it still says which colour it is while the others are near nothing, and the colour
    /// itself for the others.
    pub fn slider_fill(self, channel: HsvChannel) -> Hsla {
        match channel {
            HsvChannel::Hue => Hsv {
                hue: self.hue,
                saturation: 1.,
                value: 1.,
            }
            .to_color(),
            HsvChannel::Saturation | HsvChannel::Value => self.to_color(),
        }
    }
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type ColorHandler = Rc<dyn Fn(&Hsla, &mut Window, &mut App) + 'static>;
/// Where a picker's new colour goes.
type Setter = Rc<dyn Fn(Hsv, &mut Window, &mut App)>;

/// A small square showing a colour, which a press opens a picker on or picks.
///
/// It is a tab stop that Enter and Space press, ringed in the accent while
/// [`ColorSwatch::selected`] (the preset in use, or the swatch whose picker is open), and its
/// tooltip names the colour's code, so it is never told apart by colour alone.
#[derive(IntoElement)]
pub struct ColorSwatch {
    id: ElementId,
    color: Hsla,
    selected: bool,
    size: Pixels,
    label: Option<SharedString>,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl ColorSwatch {
    /// A swatch of `color`. The id is its test selector and keeps its focus.
    pub fn new(id: impl Into<ElementId>, color: Hsla) -> Self {
        Self {
            id: id.into(),
            color,
            selected: false,
            size: px(22.),
            label: None,
            disabled: false,
            on_click: None,
        }
    }

    /// Ring it as the one in use.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The square's side; 22 px by default.
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// What the colour is for, shown in the tooltip before its code: "Chord captions".
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Dimmed, out of the Tab order and unpressed.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called on a click, Enter or Space.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ColorSwatch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let radius = theme.shape.radius_sm;
        let code = hex(self.color);
        let words: SharedString = match &self.label {
            Some(label) => format!("{label} · {code}").into(),
            None => code.into(),
        };
        let swatch = div()
            .id(self.id.clone())
            .size(self.size)
            .flex_none()
            .rounded(radius)
            .bg(self.color)
            // A hairline inside the fill, so a colour near the surface's own still has an edge.
            .border_1()
            .border_color(theme.colors.border)
            .when(self.selected, |swatch| {
                swatch.child(
                    div()
                        .absolute()
                        // A gap of the surface between ring and fill, so the ring reads
                        // around a swatch of the accent's own colour.
                        .top(px(-4.))
                        .left(px(-4.))
                        .right(px(-4.))
                        .bottom(px(-4.))
                        .rounded(radius + px(4.))
                        .border_2()
                        .border_color(theme.colors.accent),
                )
            });
        let swatch = focusable(swatch, &self.id, self.disabled, radius, window, cx);
        if self.disabled {
            return swatch.opacity(0.45);
        }
        let swatch = swatch
            .cursor_pointer()
            .tooltip(move |window, cx| Tooltip::new(words.clone()).build(window, cx));
        match self.on_click {
            Some(handler) => swatch.on_click(handler),
            None => swatch,
        }
    }
}

/// What a [`ColorPicker`] keeps between frames.
struct PickerState {
    /// The colour last given, so a new one from the application is told apart.
    given: u32,
    hsv: Hsv,
    code: Entity<TextInput>,
    invalid: bool,
}

/// A panel that makes a colour: the colour and its code, three sliders and a row of presets.
///
/// Drag or arrow-key the hue, saturation and brightness (each a [`Slider`], so Shift, Home, End
/// and a double click work as everywhere else), type a code and press Return, or press a preset.
/// Without [`ColorPicker::on_change`] it keeps the colour it is moved to for itself. Put it under
/// the [`ColorSwatch`] that opened it, or in a [`crate::Popover`] anchored there.
///
/// ```ignore
/// ColorPicker::new("caption-colour", self.caption)
///     .presets([white, cream, black])
///     .on_change(cx.listener(|this, color: &Hsla, _, cx| {
///         this.caption = *color;
///         cx.notify();
///     }))
/// ```
#[derive(IntoElement)]
pub struct ColorPicker {
    id: ElementId,
    color: Hsla,
    presets: Vec<Hsla>,
    on_change: Option<ColorHandler>,
}

impl ColorPicker {
    /// A picker on `color`. The id keeps its state and prefixes its parts' selectors:
    /// `{id}-code`, `{id}-hue`, `{id}-saturation`, `{id}-value` and `{id}-preset-{n}`.
    pub fn new(id: impl Into<ElementId>, color: Hsla) -> Self {
        Self {
            id: id.into(),
            color,
            presets: Vec::new(),
            on_change: None,
        }
    }

    /// Colours to start from, as a row of swatches under the sliders.
    pub fn presets(mut self, presets: impl IntoIterator<Item = Hsla>) -> Self {
        self.presets = presets.into_iter().collect();
        self
    }

    /// Called with each new colour, opaque, as the person makes it: store it and pass it back
    /// on the next render.
    pub fn on_change(mut self, handler: impl Fn(&Hsla, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ColorPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let strings = cx.strings().clone();
        let given = rgb_of(self.color);
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| PickerState {
            given,
            hsv: Hsv::from_rgb(given),
            code: cx.new(|cx| TextInput::new(cx).with_text(hex(color_of(given)), cx)),
            invalid: false,
        });
        // A colour the application changed (or a held one) is taken up; one that only rounds to
        // what the sliders already make keeps their hue through grey and black.
        let (hsv, code, invalid) = state.update(cx, |state, _| {
            if given != state.given {
                state.given = given;
                if given != state.hsv.to_rgb() {
                    state.hsv = Hsv::from_rgb(given);
                }
            }
            (state.hsv, state.code.clone(), state.invalid)
        });
        let color = hsv.to_color();
        let code_text = hex(color);
        if !code.read(cx).is_focused(window) && code.read(cx).text() != code_text {
            code.update(cx, |input, cx| input.set_text(code_text.clone(), cx));
        }

        // Where a new colour goes: the application, or the picker's own state.
        let set: Setter = {
            let state = state.clone();
            let on_change = self.on_change.clone();
            Rc::new(move |hsv: Hsv, window: &mut Window, cx: &mut App| {
                state.update(cx, |state, cx| {
                    state.hsv = hsv;
                    state.invalid = false;
                    cx.notify();
                });
                if let Some(handler) = &on_change {
                    handler(&hsv.to_color(), window, cx);
                }
            })
        };

        let child_id =
            |part: &str| ElementId::NamedChild(Box::new(self.id.clone()), part.to_owned().into());
        let code_id = child_id("code");
        let focused = code.read(cx).is_focused(window);
        let field = selector(div().id(code_id.clone()), &code_id)
            .w(px(104.))
            .on_action({
                let set = set.clone();
                let state = state.clone();
                let code = code.clone();
                move |_: &SubmitText, window, cx| {
                    let text = code.read(cx).text().to_owned();
                    match parse_hex(&text) {
                        Some(color) => {
                            let current = state.read(cx).hsv;
                            // A code that names the colour already shown changes nothing.
                            if rgb_of(color) != current.to_rgb() {
                                set(Hsv::from_color(color), window, cx);
                            }
                            code.update(cx, |input, cx| input.set_text(hex(color), cx));
                            state.update(cx, |state, cx| {
                                state.invalid = false;
                                cx.notify();
                            });
                        }
                        None => state.update(cx, |state, cx| {
                            state.invalid = true;
                            cx.notify();
                        }),
                    }
                }
            })
            .on_action({
                let state = state.clone();
                let code = code.clone();
                move |_: &CancelText, _, cx| {
                    let text = hex(state.read(cx).hsv.to_color());
                    code.update(cx, |input, cx| input.set_text(text, cx));
                    state.update(cx, |state, cx| {
                        state.invalid = false;
                        cx.notify();
                    });
                }
            })
            .child(
                TextField::new()
                    .size(ControlSize::Small)
                    .focused(focused)
                    .font_family(theme.typography.mono.family.clone())
                    .child(code.clone()),
            );

        let head = div()
            .flex()
            .items_center()
            .gap(theme.spacing.sm)
            .child(
                div()
                    .size(px(28.))
                    .flex_none()
                    .rounded(theme.shape.radius_sm)
                    .bg(color)
                    .border_1()
                    .border_color(theme.colors.border),
            )
            .child(field)
            .when(invalid, |head| {
                head.child(
                    Label::new(strings.color_code_invalid.clone())
                        .size(TextSize::XSmall)
                        .color(TextColor::Custom(theme.colors.danger)),
                )
            });

        let sliders = HsvChannel::ALL.map(|channel| {
            let set = set.clone();
            let caption = match channel {
                HsvChannel::Hue => strings.hue.clone(),
                HsvChannel::Saturation => strings.saturation.clone(),
                HsvChannel::Value => strings.brightness.clone(),
            };
            Slider::new(hsv.fraction(channel))
                .id(child_id(channel.id()))
                .label(caption)
                .readout(hsv.label(channel))
                .color(hsv.slider_fill(channel))
                // A degree of hue, a percent of the others: the steps their readouts count in.
                .step(match channel {
                    HsvChannel::Hue => 1. / 360.,
                    HsvChannel::Saturation | HsvChannel::Value => 0.01,
                })
                .on_change(move |fraction: &f32, window, cx| {
                    set(hsv.with(channel, *fraction), window, cx)
                })
        });

        let presets = (!self.presets.is_empty()).then(|| {
            div()
                .flex()
                .flex_wrap()
                .gap(theme.spacing.xs)
                .pt(theme.spacing.xs)
                .children(self.presets.iter().enumerate().map(|(index, preset)| {
                    let preset = *preset;
                    let set = set.clone();
                    ColorSwatch::new(child_id(&format!("preset-{index}")), preset)
                        .selected(rgb_of(preset) == rgb_of(color))
                        .on_click(move |_, window, cx| set(Hsv::from_color(preset), window, cx))
                }))
        });

        selector(div().id(self.id.clone()), &self.id)
            .flex()
            .flex_col()
            .gap(theme.spacing.xs)
            .p(theme.spacing.sm)
            .rounded(theme.shape.radius_md)
            .border_1()
            .border_color(theme.colors.separator)
            .child(head)
            .children(sliders)
            .children(presets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primaries_sit_where_the_wheel_puts_them() {
        let red = Hsv::from_rgb(0xff0000);
        assert_eq!((red.hue, red.saturation, red.value), (0., 1., 1.));
        assert_eq!(Hsv::from_rgb(0x00ff00).hue, 120.);
        assert_eq!(Hsv::from_rgb(0x0000ff).hue, 240.);
        let grey = Hsv::from_rgb(0x808080);
        assert_eq!(grey.saturation, 0.);
        assert_eq!(Hsv::from_rgb(0x000000).saturation, 0.);
    }

    #[test]
    fn every_colour_comes_back_as_itself() {
        for r in (0..=255).step_by(15) {
            for g in (0..=255).step_by(15) {
                for b in (0..=255).step_by(15) {
                    let rgb = (r << 16) | (g << 8) | b;
                    assert_eq!(Hsv::from_rgb(rgb).to_rgb(), rgb, "{rgb:06x}");
                    // And through gpui's own colour, as the picker gets and gives it.
                    assert_eq!(rgb_of(color_of(rgb)), rgb, "{rgb:06x}");
                }
            }
        }
    }

    #[test]
    fn a_slider_moves_only_its_own_channel() {
        let colour = Hsv::from_rgb(0x3d7bff);
        let greyed = colour.with(HsvChannel::Saturation, 0.);
        assert_eq!(
            greyed.hue, colour.hue,
            "the hue survives a trip through grey"
        );
        let restored = greyed.with(HsvChannel::Saturation, colour.saturation);
        assert_eq!(restored.to_rgb(), 0x3d7bff);
        assert_eq!(colour.with(HsvChannel::Hue, 0.5).hue, 180.);
        let end = colour.with(HsvChannel::Hue, 1.);
        assert!(
            end.fraction(HsvChannel::Hue) > 0.999,
            "the thumb stays at the end"
        );
        assert_eq!(
            end.with(HsvChannel::Saturation, 1.)
                .with(HsvChannel::Value, 1.)
                .to_rgb(),
            0xff0000
        );
    }

    #[test]
    fn readouts_and_fills() {
        let colour = Hsv::from_rgb(0x808000);
        assert_eq!(colour.label(HsvChannel::Hue), "60°");
        assert_eq!(colour.label(HsvChannel::Saturation), "100%");
        assert_eq!(colour.label(HsvChannel::Value), "50%");
        assert_eq!(rgb_of(colour.slider_fill(HsvChannel::Hue)), 0xffff00);
        assert_eq!(rgb_of(colour.slider_fill(HsvChannel::Value)), 0x808000);
    }

    #[test]
    fn colour_codes_read_and_write() {
        assert_eq!(hex(color_of(0x3d7bff)), "#3D7BFF");
        assert_eq!(hex(color_of(0x000a0b)), "#000A0B");
        assert_eq!(parse_hex("#3D7BFF").map(rgb_of), Some(0x3d7bff));
        assert_eq!(parse_hex("  3d7bff ").map(rgb_of), Some(0x3d7bff));
        assert_eq!(parse_hex("#38f").map(rgb_of), Some(0x3388ff));
        for wrong in ["", "#", "#12345", "#1234567", "#GG0000", "+12345", "red"] {
            assert_eq!(parse_hex(wrong), None, "{wrong:?}");
        }
    }
}
