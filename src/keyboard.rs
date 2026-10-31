//! A piano keyboard for auditioning notes: a sound's preview, a scale, a chord.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, MouseButton, MouseMoveEvent,
    ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
    div, prelude::FluentBuilder, px, relative,
};

use crate::controls::on_fill;
use crate::focus::{
    CONTROL_CONTEXT, StepBackward, StepForward, StepToEnd, StepToStart, TRACK_CONTEXT, focusable,
};
use crate::theme::{ActiveTheme, PrimaryStyle, mix};

type PitchHandler = Rc<dyn Fn(&u8, &mut Window, &mut App) + 'static>;

/// Whether a MIDI pitch is a black key.
fn is_black(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

/// The name of a pitch with its octave, middle C (60) being "C4".
pub fn note_name(pitch: u8) -> SharedString {
    const NAMES: [&str; 12] = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ];
    format!(
        "{}{}",
        NAMES[(pitch % 12) as usize],
        i32::from(pitch) / 12 - 1
    )
    .into()
}

/// A run of piano keys from `low` to `high`, black keys over the seams of the white ones.
///
/// A press sounds the key at once, on the way down rather than on release, and dragging across
/// the keys plays each one the pointer reaches, as on an instrument. The keys in
/// [`PianoKeyboard::pressed`] hold the accent. The keyboard is one tab stop: the arrows step the
/// pressed key a semitone, Home and End go to the ends, and Enter or Space plays it again.
///
/// Each key's test selector is the keyboard's id and its pitch: `keys-60`.
#[derive(IntoElement)]
pub struct PianoKeyboard {
    id: ElementId,
    low: u8,
    high: u8,
    pressed: Vec<u8>,
    labels: bool,
    disabled: bool,
    height: Option<Pixels>,
    on_press: Option<PitchHandler>,
}

impl PianoKeyboard {
    /// Keys from `low` to `high`, as MIDI pitches. An end on a black key widens to the white key
    /// beside it, so the keyboard always starts and ends on a white key.
    pub fn new(id: impl Into<ElementId>, low: u8, high: u8) -> Self {
        let (low, high) = (low.min(high), low.max(high));
        let low = if is_black(low) { low - 1 } else { low };
        let high = if is_black(high) {
            high.saturating_add(1)
        } else {
            high
        };
        Self {
            id: id.into(),
            low,
            high,
            pressed: Vec::new(),
            labels: true,
            disabled: false,
            height: None,
            on_press: None,
        }
    }

    /// One octave from middle C to the C above it.
    pub fn octave(id: impl Into<ElementId>) -> Self {
        Self::new(id, 60, 72)
    }

    /// The keys that are sounding, in the accent.
    pub fn pressed(mut self, pressed: impl IntoIterator<Item = u8>) -> Self {
        self.pressed = pressed.into_iter().collect();
        self
    }

    /// Name each C with its octave ("C4"), the way a keyboard's own markings do. On by default.
    pub fn labels(mut self, labels: bool) -> Self {
        self.labels = labels;
        self
    }

    /// Greyed and silent, out of the Tab order: for example while nothing is selected to play.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The height of the white keys; the black ones take three fifths of it. Defaults to four
    /// control heights.
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = Some(height);
        self
    }

    /// Called with the pitch a press, a drag or a key reaches; sound it there.
    pub fn on_press(mut self, handler: impl Fn(&u8, &mut Window, &mut App) + 'static) -> Self {
        self.on_press = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PianoKeyboard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let colors = &theme.colors;
        let dark = theme.appearance.is_dark();
        let height = self.height.unwrap_or(theme.density.control_md * 4.);
        let radius = theme.shape.radius_xs;
        let handler = self.on_press.filter(|_| !self.disabled);

        let white_fill = if dark {
            mix(colors.elevated, colors.text, 0.12)
        } else {
            colors.elevated
        };
        let black_fill = if dark {
            mix(colors.sunken, gpui::black(), 0.5)
        } else {
            mix(colors.text, colors.elevated, 0.08)
        };
        let lit = on_fill(&theme);
        let on_lit = match theme.primary_style {
            PrimaryStyle::Accent => colors.on_accent,
            PrimaryStyle::Solid => colors.on_solid,
        };
        let label_color = colors.text_faint;

        let pitches: Vec<u8> = (self.low..=self.high).collect();
        let whites = pitches
            .iter()
            .filter(|pitch| !is_black(**pitch))
            .count()
            .max(1);
        let pressed = self.pressed.clone();
        let current = pressed.first().copied();
        let key_id =
            |pitch: u8| ElementId::NamedChild(Box::new(self.id.clone()), pitch.to_string().into());
        let selector = {
            let id = self.id.clone();
            move |pitch: u8| -> SharedString { format!("{id}-{pitch}").into() }
        };

        // A key sounds on the press, and again for each new key a held pointer slides onto.
        let key = |pitch: u8, element: gpui::Stateful<gpui::Div>| {
            let lit = pressed.contains(&pitch);
            let name = selector(pitch);
            let element = element.debug_selector(move || name.to_string());
            match handler.clone() {
                None => element,
                Some(handler) => {
                    let slide = handler.clone();
                    element
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            handler(&pitch, window, cx)
                        })
                        .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                            if event.pressed_button == Some(MouseButton::Left) && !lit {
                                slide(&pitch, window, cx)
                            }
                        })
                }
            }
        };

        let white_keys = pitches
            .iter()
            .copied()
            .filter(|p| !is_black(*p))
            .map(|pitch| {
                let on = pressed.contains(&pitch);
                let hover = mix(white_fill, colors.text, 0.06);
                key(pitch, div().id(key_id(pitch)))
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .items_center()
                    .pb(theme.spacing.xs)
                    .rounded_b(radius)
                    .border_1()
                    .border_color(colors.border)
                    .bg(if on { lit } else { white_fill })
                    .when(!on && handler.is_some(), |key| {
                        key.hover(move |style| style.bg(hover))
                    })
                    .when(self.labels && pitch % 12 == 0, |key| {
                        key.child(
                            div()
                                .text_size(theme.typography.size_xs)
                                .text_color(if on { on_lit } else { label_color })
                                .child(note_name(pitch)),
                        )
                    })
            });

        let mut seam = 0;
        let mut black_keys = Vec::new();
        for pitch in pitches.iter().copied() {
            if !is_black(pitch) {
                seam += 1;
                continue;
            }
            let on = pressed.contains(&pitch);
            let hover = mix(black_fill, colors.text, 0.18);
            black_keys.push(
                key(pitch, div().id(key_id(pitch)))
                    .absolute()
                    .top_0()
                    .left(relative((seam as f32 - 0.3) / whites as f32))
                    .w(relative(0.6 / whites as f32))
                    .h(relative(0.6))
                    .rounded_b(radius)
                    .bg(if on { lit } else { black_fill })
                    .border_1()
                    .border_color(gpui::black().opacity(0.25))
                    .when(!on && handler.is_some(), |key| {
                        key.hover(move |style| style.bg(hover))
                    })
                    // Over the white keys, so a press on a black key never reaches the one below.
                    .occlude(),
            );
        }

        let (low, high) = (self.low, self.high);
        let mut keyboard = focusable(
            div().id(self.id.clone()),
            &self.id,
            handler.is_none(),
            radius,
            window,
            cx,
        )
        .relative()
        .flex()
        .w_full()
        .h(height)
        .when(self.disabled, |keyboard| keyboard.opacity(0.5))
        .children(white_keys)
        .children(black_keys);
        if let Some(handler) = handler {
            let step = move |to: Option<u8>| {
                let handler = handler.clone();
                move |window: &mut Window, cx: &mut App| {
                    if let Some(to) = to {
                        handler(&to.clamp(low, high), window, cx)
                    }
                }
            };
            let (back, forward, start, end, again) = (
                step(Some(current.map_or(low, |at| at.saturating_sub(1)))),
                step(Some(current.map_or(low, |at| at.saturating_add(1)))),
                step(Some(low)),
                step(Some(high)),
                step(Some(current.unwrap_or(low))),
            );
            keyboard = keyboard
                .key_context(format!("{CONTROL_CONTEXT} {TRACK_CONTEXT}").as_str())
                .on_action(move |_: &StepBackward, window, cx| back(window, cx))
                .on_action(move |_: &StepForward, window, cx| forward(window, cx))
                .on_action(move |_: &StepToStart, window, cx| start(window, cx))
                .on_action(move |_: &StepToEnd, window, cx| end(window, cx))
                // Enter and Space; a pointer has already played its key on the way down.
                .on_click(move |event: &ClickEvent, window, cx| {
                    if event.is_keyboard() {
                        again(window, cx)
                    }
                });
        }
        keyboard.min_w(px(whites as f32 * 12.))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ends_widen_to_white_keys() {
        let keys = PianoKeyboard::new("k", 61, 70);
        assert_eq!((keys.low, keys.high), (60, 71));
        let keys = PianoKeyboard::octave("k");
        assert_eq!((keys.low, keys.high), (60, 72));
    }

    #[test]
    fn names_carry_the_octave() {
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(61), "C♯4");
        assert_eq!(note_name(21), "A0");
        assert_eq!(note_name(0), "C-1");
    }
}
