//! How hard the agent thinks, and whether it answers fast: one control for both.

use std::cell::Cell;
use std::f32::consts::TAU;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Bounds, ClickEvent, ElementId, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, RenderOnce, SharedString,
    Styled, Window, canvas, div, fill, point, px, relative, size,
};

use crate::button::{Button, ControlSize};
use crate::controls::on_fill;
use crate::focus::focusable;
use crate::icon::{IconName, IconSource};
use crate::motion::{ease_out, lerp, looping};
use crate::theme::{ActiveTheme, PrimaryStyle, Theme, mix};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type SelectHandler = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

/// The reasoning effort and fast mode, as one card that opens over the composer.
///
/// Fast mode is the bolt in the corner, not a second control in the composer row: it is the
/// other half of the same trade, speed against thought. The level reads in the accent at the
/// top, the model it applies to sits under it and opens the model picker, and reset returns
/// to [`EffortPicker::default_level`]. The slider is a wide stepped track: its filled part
/// sparkles, a dot marks each step still above the thumb, and a click, a drag or the arrow
/// keys move it from step to step.
///
/// Open it from an [`EffortButton`] in the composer row, placed with gpui's `anchored` and
/// `deferred`, like any popover.
#[derive(IntoElement)]
pub struct EffortPicker {
    id: ElementId,
    levels: Vec<SharedString>,
    selected: usize,
    default_level: Option<usize>,
    fast: bool,
    model: Option<SharedString>,
    model_icon: Option<IconSource>,
    width: Pixels,
    on_select: Option<SelectHandler>,
    on_fast: Option<ClickHandler>,
    on_model: Option<ClickHandler>,
    on_reset: Option<ClickHandler>,
}

impl EffortPicker {
    /// A picker over `levels`, lowest first, with `selected` chosen.
    pub fn new(
        id: impl Into<ElementId>,
        levels: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: usize,
    ) -> Self {
        let levels: Vec<SharedString> = levels.into_iter().map(Into::into).collect();
        Self {
            id: id.into(),
            selected: selected.min(levels.len().saturating_sub(1)),
            levels,
            default_level: None,
            fast: false,
            model: None,
            model_icon: None,
            width: px(320.),
            on_select: None,
            on_fast: None,
            on_model: None,
            on_reset: None,
        }
    }

    /// The level reset returns to. Reset is disabled while it, and fast mode off, are in use.
    pub fn default_level(mut self, level: usize) -> Self {
        self.default_level = Some(level);
        self
    }

    /// Whether fast mode is on: the bolt latches in the accent.
    pub fn fast(mut self, fast: bool) -> Self {
        self.fast = fast;
        self
    }

    /// The model the effort applies to, under the level.
    pub fn model(mut self, model: impl Into<SharedString>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// The mark before the model's name, usually its provider's.
    pub fn model_icon(mut self, icon: impl Into<IconSource>) -> Self {
        self.model_icon = Some(icon.into());
        self
    }

    /// The card's width. The slider takes all of it.
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    /// Called with the level a click, a drag or an arrow key moved to.
    pub fn on_select(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Called on the bolt; flip fast mode there.
    pub fn on_fast(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_fast = Some(Box::new(handler));
        self
    }

    /// Called on the model's name; open the model menu there.
    pub fn on_model(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_model = Some(Box::new(handler));
        self
    }

    /// Called on reset; go back to the default level and turn fast mode off there.
    pub fn on_reset(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reset = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for EffortPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let colors = &theme.colors;
        let id = self.id.clone();
        let outer = theme.shape.radius_lg;
        let inset = theme.spacing.md;
        let corner = Theme::concentric(outer, inset);

        let mut bolt = Button::icon_only((id.clone(), "fast"), IconName::Bolt)
            .selected(self.fast)
            .radius(corner.min(theme.density.control_md / 2.))
            .tooltip(if self.fast {
                "Fast mode: on"
            } else {
                "Fast mode: off"
            });
        if let Some(handler) = self.on_fast {
            bolt = bolt.on_click(handler);
        }
        let at_default = self
            .default_level
            .is_none_or(|level| level == self.selected)
            && !self.fast;
        let mut reset = Button::icon_only((id.clone(), "reset"), IconName::Refresh)
            .disabled(at_default)
            .radius(corner.min(theme.density.control_md / 2.))
            .tooltip("Reset");
        if let Some(handler) = self.on_reset {
            reset = reset.on_click(handler);
        }
        let model = self.model.map(|model| {
            let mut button = Button::new((id.clone(), "model"), model)
                .ghost()
                .size(ControlSize::Small)
                .icon_after(IconName::ChevronRight);
            if let Some(icon) = self.model_icon {
                button = button.icon(icon);
            }
            if let Some(handler) = self.on_model {
                button = button.on_click(handler);
            }
            button
        });
        let level = self.levels.get(self.selected).cloned().unwrap_or_default();

        let header = div()
            .flex()
            .items_start()
            .gap(theme.spacing.sm)
            .child(bolt)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(theme.spacing.xxs)
                    .child(
                        div()
                            .text_size(theme.typography.size_xl)
                            .font_weight(theme.typography.heading_weight)
                            .text_color(colors.accent_text)
                            .child(level),
                    )
                    .children(model),
            )
            .child(reset);

        let slider = StepSlider {
            id: (id.clone(), "steps").into(),
            steps: self.levels.len(),
            selected: self.selected,
            on_select: self.on_select,
        }
        .render(window, cx);

        let surface = div()
            .id(id.clone())
            .flex()
            .flex_col()
            .gap(theme.spacing.md)
            .w(self.width)
            .p(inset)
            .rounded(outer)
            .bg(colors.overlay)
            .border_1()
            .border_color(colors.border)
            .shadow(theme.elevation.medium.clone())
            .child(header)
            .child(slider);
        crate::motion::appear(surface, id, cx, |surface, delta| {
            surface.opacity(delta).mt(px(lerp(4., 0., delta)))
        })
    }
}

/// Where the thumb is gliding from and to, kept per slider between frames.
#[derive(Clone, Copy)]
struct Glide {
    from: f32,
    to: f32,
    start: Instant,
}

/// The wide stepped track of [`EffortPicker`].
struct StepSlider {
    id: ElementId,
    steps: usize,
    selected: usize,
    on_select: Option<SelectHandler>,
}

/// One sparkle in the filled part: where it sits (0..1 of the track's width and height), how
/// big it is and when it twinkles.
struct Sparkle {
    x: f32,
    y: f32,
    radius: f32,
    phase: f32,
    rate: f32,
}

/// A fixed field of sparkles, the same every frame, so they twinkle in place.
fn sparkles() -> Vec<Sparkle> {
    // A small hash keeps them scattered without a random number generator.
    let hash = |n: u32| {
        let mut x = n.wrapping_mul(0x9e37_79b9) ^ 0x85eb_ca6b;
        x ^= x >> 15;
        x = x.wrapping_mul(0x2c1b_3c6d);
        x ^= x >> 12;
        (x & 0xffff) as f32 / 65535.
    };
    (0..28)
        .map(|i| Sparkle {
            x: (i as f32 + hash(i * 4)) / 28.,
            y: 0.18 + 0.64 * hash(i * 4 + 1),
            radius: 0.9 + 1.2 * hash(i * 4 + 2).powi(2),
            phase: hash(i * 4 + 3),
            rate: if i % 3 == 0 { 2. } else { 1. },
        })
        .collect()
}

impl StepSlider {
    fn render(self, window: &mut Window, cx: &mut App) -> AnyElement {
        let theme = cx.theme().clone();
        let colors = &theme.colors;
        let track = theme.density.control_lg.max(px(34.));
        let thumb = track + px(6.);
        let steps = self.steps.max(1);
        let last = (steps - 1).max(1) as f32;
        let target = self.selected as f32 / last;
        let duration = theme.motion.normal;

        // The thumb glides to a new step; the glide is kept per slider, not per frame.
        let glide = window.with_global_id(
            ElementId::NamedChild(Box::new(self.id.clone()), "glide".into()),
            |global_id, window| {
                window.with_element_state(global_id, |glide: Option<Glide>, _| {
                    let now = Instant::now();
                    let glide = match glide {
                        None => Glide {
                            from: target,
                            to: target,
                            start: now,
                        },
                        Some(glide) if glide.to == target => glide,
                        Some(glide) => Glide {
                            from: position(glide, now, duration),
                            to: target,
                            start: now,
                        },
                    };
                    (glide, glide)
                })
            },
        );

        let bounds: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::default();
        let fill_color = on_fill(&theme);
        // Sparkles are light on the accent; on a solid fill they take its text colour.
        let spark_color = match theme.primary_style {
            PrimaryStyle::Accent => mix(fill_color, gpui::white(), 0.9),
            PrimaryStyle::Solid => theme.colors.on_solid,
        };
        let track_color = mix(
            colors.overlay,
            colors.text,
            if theme.appearance.is_dark() {
                0.07
            } else {
                0.05
            },
        );
        let dot_color = colors.text_faint;
        let field = Rc::new(sparkles());
        let elevation = theme.elevation.medium.clone();
        let painted = bounds.clone();

        // One loop drives both the sparkles' twinkle and the thumb's glide.
        let body = looping(
            div().relative().w_full().h(thumb),
            (self.id.clone(), "sparkle"),
            Duration::from_millis(4000),
            0.,
            cx,
            move |body, phase| {
                let at = position(glide, Instant::now(), duration);
                let field = field.clone();
                let painted = painted.clone();
                body.child(
                    canvas(
                        move |bounds, _, _| painted.set(Some(bounds)),
                        move |bounds, _, window, _| {
                            let width = f32::from(bounds.size.width);
                            let height = f32::from(track);
                            let top = bounds.origin.y + (bounds.size.height - track) / 2.;
                            let half = f32::from(thumb) / 2.;
                            let centre = |at: f32| half + at * (width - half * 2.);
                            let radius = track / 2.;
                            window.paint_quad(
                                fill(
                                    Bounds::new(
                                        point(bounds.origin.x, top),
                                        size(bounds.size.width, track),
                                    ),
                                    track_color,
                                )
                                .corner_radii(radius),
                            );
                            let filled = centre(at);
                            window.paint_quad(
                                fill(
                                    Bounds::new(
                                        point(bounds.origin.x, top),
                                        size(px(filled), track),
                                    ),
                                    fill_color,
                                )
                                .corner_radii(radius),
                            );
                            // Sparkles live only in the filled part, fading out at its edge.
                            for spark in field.iter() {
                                // Kept clear of the rounded ends.
                                let x = half * 0.5 + spark.x * (width - half);
                                if x > filled - half * 0.6 {
                                    continue;
                                }
                                let twinkle =
                                    (TAU * (phase * spark.rate + spark.phase)).sin() * 0.5 + 0.5;
                                let edge = ((filled - x) / half).clamp(0., 1.);
                                let alpha = (0.25 + 0.75 * twinkle * twinkle) * edge;
                                let r = spark.radius * (0.7 + 0.3 * twinkle);
                                let centre_y = f32::from(top) + spark.y * height;
                                window.paint_quad(
                                    fill(
                                        Bounds::new(
                                            point(bounds.origin.x + px(x - r), px(centre_y - r)),
                                            size(px(r * 2.), px(r * 2.)),
                                        ),
                                        spark_color.opacity(alpha),
                                    )
                                    .corner_radii(px(r)),
                                );
                            }
                            // A dot marks each step still ahead of the thumb.
                            let dot = px(5.);
                            for step in 0..steps {
                                let x = centre(step as f32 / last);
                                if x <= filled + half {
                                    continue;
                                }
                                window.paint_quad(
                                    fill(
                                        Bounds::new(
                                            point(
                                                bounds.origin.x + px(x) - dot / 2.,
                                                top + (track - dot) / 2.,
                                            ),
                                            size(dot, dot),
                                        ),
                                        dot_color,
                                    )
                                    .corner_radii(dot / 2.),
                                );
                            }
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                )
                // The thumb's left edge sits at value × (width − thumb), so it never overhangs.
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left(relative(at))
                        .ml(-thumb * at)
                        .size(thumb)
                        .rounded(thumb / 2.)
                        .bg(gpui::white())
                        .border_1()
                        .border_color(gpui::black().opacity(0.1))
                        .shadow(elevation.clone()),
                )
            },
        );

        // The step nearest the pointer, from the bounds the canvas last painted at.
        let nearest = move |x: Pixels| -> Option<usize> {
            let bounds = bounds.get()?;
            let half = thumb / 2.;
            let span = (bounds.size.width - half * 2.).max(px(1.));
            let at = ((x - bounds.origin.x - half) / span).clamp(0., 1.);
            Some((at * last).round() as usize)
        };
        let nearest = Rc::new(nearest);
        let selected = self.selected;
        let mut slider = focusable(
            div().id(self.id.clone()),
            &self.id,
            self.on_select.is_none(),
            thumb / 2.,
            window,
            cx,
        )
        .w_full()
        .child(body);
        if let Some(handler) = self.on_select {
            let pick = {
                let handler = handler.clone();
                let nearest = nearest.clone();
                move |x: Pixels, window: &mut Window, cx: &mut App| {
                    if let Some(step) = nearest(x).filter(|step| *step != selected) {
                        handler(&step, window, cx);
                    }
                }
            };
            let drag = pick.clone();
            let keys = handler;
            slider = slider
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    move |event: &MouseDownEvent, window, cx| pick(event.position.x, window, cx),
                )
                .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        drag(event.position.x, window, cx)
                    }
                })
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let step = match event.keystroke.key.as_str() {
                        "left" | "down" => selected.checked_sub(1),
                        "right" | "up" => Some(selected + 1).filter(|step| *step < steps),
                        "home" => Some(0),
                        "end" => Some(steps - 1),
                        _ => None,
                    };
                    if let Some(step) = step.filter(|step| *step != selected) {
                        keys(&step, window, cx);
                        cx.stop_propagation();
                    }
                });
        }
        slider.into_any_element()
    }
}

/// Where a glide has the thumb at `now`, from 0 to 1.
fn position(glide: Glide, now: Instant, duration: Duration) -> f32 {
    let delta = (now - glide.start).as_secs_f32() / duration.as_secs_f32().max(0.001);
    lerp(glide.from, glide.to, ease_out(delta.clamp(0., 1.)))
}

/// What opens an [`EffortPicker`] from the composer row: the level, with the bolt in front of
/// it while fast mode is on.
#[derive(IntoElement)]
pub struct EffortButton {
    id: ElementId,
    level: SharedString,
    fast: bool,
    open: bool,
    on_click: Option<ClickHandler>,
}

impl EffortButton {
    /// A button showing `level`.
    pub fn new(id: impl Into<ElementId>, level: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            level: level.into(),
            fast: false,
            open: false,
            on_click: None,
        }
    }

    /// Whether fast mode is on.
    pub fn fast(mut self, fast: bool) -> Self {
        self.fast = fast;
        self
    }

    /// Whether the picker is open: the chevron turns down to close it.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Called on click; open or close the picker there.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for EffortButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Fast mode latches the button, bolt and all, the way the bolt in the picker latches.
        let mut button = Button::new(self.id, self.level)
            .ghost()
            .size(ControlSize::Small)
            .selected(self.fast)
            .icon_after(if self.open {
                IconName::ChevronDown
            } else {
                IconName::ChevronUp
            })
            .tooltip("Effort and fast mode");
        if self.fast {
            button = button.icon(IconName::Bolt);
        }
        if let Some(handler) = self.on_click {
            button = button.on_click(handler);
        }
        button
    }
}
