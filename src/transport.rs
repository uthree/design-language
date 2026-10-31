//! The transport's display: playback controls and their readouts on one sunken surface.

use gpui::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder,
};

use crate::focus::focusable;
use crate::theme::ActiveTheme;

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// One field of a transport display: a faint caption over a value, such as "TEMPO" over
/// "120.00", with an optional unit after it.
///
/// The value is set in the mono face so it does not shift as its digits change; give the field
/// a [`Readout::min_width`] that fits its longest value for the same reason. A readout given
/// [`Readout::on_click`] lights on hover and is a tab stop: open its editor there.
#[derive(IntoElement)]
pub struct Readout {
    id: ElementId,
    caption: SharedString,
    value: SharedString,
    unit: Option<SharedString>,
    min_width: Option<Pixels>,
    on_click: Option<ClickHandler>,
}

impl Readout {
    /// A field captioned `caption` showing `value`.
    pub fn new(
        id: impl Into<ElementId>,
        caption: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            caption: caption.into(),
            value: value.into(),
            unit: None,
            min_width: None,
            on_click: None,
        }
    }

    /// A unit after the value, in the caption's colour: "BPM", "Hz".
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// The narrowest the field gets, so the strip holds still as the value changes.
    pub fn min_width(mut self, width: Pixels) -> Self {
        self.min_width = Some(width);
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

impl RenderOnce for Readout {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let colors = &theme.colors;
        let hover = colors.hover;
        let mut field = focusable(
            div().id(self.id.clone()),
            &self.id,
            self.on_click.is_none(),
            theme.shape.radius_xs,
            window,
            cx,
        )
        .h_full()
        .flex()
        .flex_col()
        .justify_center()
        .px(theme.spacing.md)
        .gap(theme.spacing.xxs)
        .when_some(self.min_width, |field, width| field.min_w(width))
        .child(
            div()
                .text_size(theme.typography.size_xs)
                .line_height(theme.typography.size_xs)
                .text_color(colors.text_faint)
                .whitespace_nowrap()
                .child(self.caption),
        )
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(theme.spacing.xxs)
                .whitespace_nowrap()
                .child(
                    div()
                        .font(theme.typography.mono.clone())
                        .text_size(theme.typography.size_md)
                        .line_height(theme.typography.size_md)
                        .text_color(colors.text)
                        .child(self.value),
                )
                .children(self.unit.map(|unit| {
                    div()
                        .text_size(theme.typography.size_xs)
                        .text_color(colors.text_faint)
                        .child(unit)
                })),
        );
        if let Some(handler) = self.on_click {
            field = field
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .on_click(handler);
        }
        field
    }
}

/// A transport's controls and its [`Readout`]s on one sunken surface, the way a hardware
/// display groups them: the controls lead, and a hairline parts each readout from the next.
///
/// Sharing the surface is the point. A tempo box, a position box and a row of play buttons each
/// in its own frame read as parts bolted together; on one surface they read as one instrument.
#[derive(IntoElement)]
pub struct ReadoutStrip {
    id: Option<ElementId>,
    controls: Vec<AnyElement>,
    readouts: Vec<AnyElement>,
    height: Option<Pixels>,
}

impl ReadoutStrip {
    /// An empty strip.
    pub fn new() -> Self {
        Self {
            id: None,
            controls: Vec::new(),
            readouts: Vec::new(),
            height: None,
        }
    }

    /// The strip's test selector, so a test can find it with `VisualTestContext::debug_bounds`
    /// as it finds any other component by its id.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Controls at the leading end, usually small ghost buttons: play, stop, record, loop.
    pub fn controls(mut self, controls: impl IntoElement) -> Self {
        self.controls.push(controls.into_any_element());
        self
    }

    /// A readout, after the ones before it.
    pub fn readout(mut self, readout: impl IntoElement) -> Self {
        self.readouts.push(readout.into_any_element());
        self
    }

    /// The strip's height. Defaults to a large control's height and a little more, enough for
    /// a caption over a value.
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = Some(height);
        self
    }
}

impl Default for ReadoutStrip {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for ReadoutStrip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let height = self
            .height
            .unwrap_or(theme.density.control_lg + theme.spacing.sm);
        let separator = colors.separator;
        let has_controls = !self.controls.is_empty();
        let strip = match &self.id {
            Some(id) => crate::focus::selector(div(), id),
            None => div(),
        };
        strip
            .flex()
            .flex_none()
            .items_center()
            .h(height)
            .overflow_hidden()
            .rounded(theme.shape.radius_md)
            .bg(colors.sunken)
            .border_1()
            .border_color(colors.border)
            .when(has_controls, |strip| {
                strip.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme.spacing.xxs)
                        .px(theme.spacing.xs)
                        .children(self.controls),
                )
            })
            .children(
                self.readouts
                    .into_iter()
                    .enumerate()
                    .map(|(index, readout)| {
                        div()
                            .h_full()
                            .flex()
                            .when(index > 0 || has_controls, |cell| {
                                cell.border_l_1().border_color(separator)
                            })
                            .child(readout)
                    }),
            )
    }
}
