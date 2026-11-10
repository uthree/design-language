//! A figure read at a glance: a caption, a large number and its unit.

use gpui::{
    App, IntoElement, ParentElement, Pixels, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder,
};

use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

/// What a [`Stat`] shows while there is nothing to read yet.
const NOTHING: &str = "—";

/// One figure that a panel exists to report: loudness, peak, a count, a time.
///
/// A faint caption sits over the number, which is set in the mono face so its digits hold still
/// as they change, with the unit after it in the caption's step. While there is nothing to read
/// yet, [`Stat::empty`] shows a dash rather than a zero that looks measured. [`Stat::inline`]
/// puts the caption before the number on one line, for a title bar or a status line.
///
/// Several stats side by side read as one row of instruments; give each a
/// [`Stat::min_width`] that fits its longest value so the row does not shift.
#[derive(IntoElement)]
pub struct Stat {
    caption: SharedString,
    value: Option<SharedString>,
    unit: Option<SharedString>,
    size: TextSize,
    color: TextColor,
    inline: bool,
    end: bool,
    min_width: Option<Pixels>,
}

impl Stat {
    /// `value` captioned `caption`.
    pub fn new(caption: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self::maybe(caption, Some(value.into()))
    }

    /// The figure captioned `caption` before there is anything to read: a dash.
    pub fn empty(caption: impl Into<SharedString>) -> Self {
        Self::maybe(caption, None::<SharedString>)
    }

    /// `value` if there is one, otherwise the dash of [`Stat::empty`].
    pub fn maybe(caption: impl Into<SharedString>, value: Option<impl Into<SharedString>>) -> Self {
        Self {
            caption: caption.into(),
            value: value.map(Into::into),
            unit: None,
            size: TextSize::XLarge,
            color: TextColor::Default,
            inline: false,
            end: false,
            min_width: None,
        }
    }

    /// The unit after the number: "LUFS", "dB", "ms".
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// The number's step on the type scale: `XLarge` by default, `Display` for the one figure
    /// a view is about, `Medium` or `Small` in a dense row or [`Stat::inline`].
    pub fn size(mut self, size: TextSize) -> Self {
        self.size = size;
        self
    }

    /// The number's colour, when the figure is a verdict as well: `Danger` over the ceiling.
    /// Say it in words too; colour alone is not read by everyone.
    pub fn color(mut self, color: TextColor) -> Self {
        self.color = color;
        self
    }

    /// The caption before the number on one line.
    pub fn inline(mut self) -> Self {
        self.inline = true;
        self
    }

    /// Aligned to the end, for a figure at the right of a row.
    pub fn end(mut self) -> Self {
        self.end = true;
        self
    }

    /// The narrowest it gets, so a row of stats holds still as the values change.
    pub fn min_width(mut self, width: Pixels) -> Self {
        self.min_width = Some(width);
        self
    }
}

impl RenderOnce for Stat {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let unit_size = if self.inline {
            TextSize::XSmall
        } else {
            TextSize::Small
        };
        let measured = self.value.is_some();
        let figure = div()
            .flex()
            .items_baseline()
            .gap(theme.spacing.xs)
            .when(self.end, |figure| figure.justify_end())
            .child(
                Label::new(self.value.unwrap_or_else(|| NOTHING.into()))
                    .size(self.size)
                    .mono()
                    .weight(theme.typography.label_weight)
                    .color(if measured {
                        self.color
                    } else {
                        TextColor::Faint
                    }),
            )
            .children(
                self.unit
                    .map(|unit| Label::new(unit).size(unit_size).color(TextColor::Muted)),
            );
        let caption = Label::new(self.caption)
            .size(TextSize::XSmall)
            .color(TextColor::Faint);
        let stat = div()
            .flex()
            .whitespace_nowrap()
            .when_some(self.min_width, |stat, width| stat.min_w(width));
        if self.inline {
            stat.items_baseline()
                .gap(theme.spacing.sm)
                .when(self.end, |stat| stat.justify_end())
                .child(caption)
                .child(figure)
        } else {
            stat.flex_col()
                .gap(theme.spacing.xxs)
                .when(self.end, |stat| stat.items_end())
                .child(caption)
                .child(figure)
        }
    }
}
