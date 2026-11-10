//! An application's own log: records the person can read, select across and copy.

use gpui::{
    App, ElementId, IntoElement, ParentElement, RenderOnce, ScrollHandle, SharedString, Styled,
    Window, div, prelude::FluentBuilder, px,
};

use crate::selectable::{LINE_SEPARATOR, SelectableText, SelectionScope};
use crate::text::{TextColor, TextSize};
use crate::theme::ActiveTheme;

/// How bad a [`LogLine`] is: `log`'s five levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    /// Something failed.
    Error,
    /// Something is wrong but carried on.
    Warn,
    /// What happened.
    Info,
    /// Detail for whoever is debugging.
    Debug,
    /// Every step.
    Trace,
}

impl LogLevel {
    /// The word for the level, as `RUST_LOG` and every terminal spell it. Not translated: the
    /// same records also go to a terminal under these names.
    pub fn word(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        }
    }

    /// Only the two levels that mean something are coloured, so the one red line nobody is
    /// meant to miss is not just another colour.
    fn color(self) -> TextColor {
        match self {
            Self::Error => TextColor::Danger,
            Self::Warn => TextColor::Warning,
            _ => TextColor::Faint,
        }
    }
}

/// One record of a [`LogView`].
#[derive(Clone, Debug)]
pub struct LogLine {
    key: SharedString,
    time: Option<SharedString>,
    level: Option<LogLevel>,
    source: Option<SharedString>,
    message: SharedString,
}

impl LogLine {
    /// A record. `key` names the record, not its row — a sequence number, say — so a selection
    /// stays on it as new lines arrive.
    pub fn new(key: impl Into<SharedString>, message: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            time: None,
            level: None,
            source: None,
            message: message.into(),
        }
    }

    /// When it was written, as the application likes to show it: `12s`, `14:02:07`.
    pub fn time(mut self, time: impl Into<SharedString>) -> Self {
        self.time = Some(time.into());
        self
    }

    /// How bad it is.
    pub fn level(mut self, level: LogLevel) -> Self {
        self.level = Some(level);
        self
    }

    /// Who wrote it: a module path, a tool's name.
    pub fn source(mut self, source: impl Into<SharedString>) -> Self {
        self.source = Some(source.into());
        self
    }
}

/// Log records one to a line, in the monospaced face: the time, the level, who wrote it and
/// what it said.
///
/// The whole log is one [`SelectionScope`]: a drag runs down across records, and a copy gives
/// them as a terminal would, one per line, `12s WARN auris::session: message`, with the source
/// in full where the column cuts it. ⌘A or Ctrl+A selects everything. It draws only its lines;
/// put it in a scrolled area and pass that area's handle to [`LogView::scroll`] so a drag past
/// the edge scrolls on.
#[derive(IntoElement)]
pub struct LogView {
    id: ElementId,
    lines: Vec<LogLine>,
    scroll: Option<ScrollHandle>,
    source_width: gpui::Pixels,
}

impl LogView {
    /// An empty log.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            lines: Vec::new(),
            scroll: None,
            source_width: px(120.),
        }
    }

    /// Add a record.
    pub fn line(mut self, line: LogLine) -> Self {
        self.lines.push(line);
        self
    }

    /// Add records, oldest first.
    pub fn lines(mut self, lines: impl IntoIterator<Item = LogLine>) -> Self {
        self.lines.extend(lines);
        self
    }

    /// The scrolled area the log sits in, which a drag past its edge scrolls.
    pub fn scroll(mut self, handle: &ScrollHandle) -> Self {
        self.scroll = Some(handle.clone());
        self
    }

    /// How wide the source column is; 120 px by default. Longer sources are cut on screen and
    /// copied whole.
    pub fn source_width(mut self, width: gpui::Pixels) -> Self {
        self.source_width = width;
        self
    }
}

impl RenderOnce for LogView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let spacing = &theme.spacing;
        let id = self.id.clone();
        let part = move |line: &LogLine, name: &str, text: SharedString| {
            SelectableText::new(
                ElementId::NamedChild(Box::new(id.clone()), format!("{}-{name}", line.key).into()),
                text,
            )
            .mono()
            .size(TextSize::XSmall)
        };
        let rows = self.lines.iter().map(|line| {
            // Each record starts a line of the copy; its parts follow one another on it.
            let mut separator = LINE_SEPARATOR;
            let mut next = |after: &'static str| std::mem::replace(&mut separator, after);
            let time = line.time.clone().map(|time| {
                part(line, "time", time)
                    .color(TextColor::Faint)
                    .separator(next(" "))
            });
            let level = line.level.map(|level| {
                part(line, "level", level.word().into())
                    .color(level.color())
                    .separator(next(" "))
            });
            let source = line.source.clone().map(|source| {
                part(line, "source", source)
                    .color(TextColor::Faint)
                    .separator(next(": "))
            });
            let message = part(line, "message", line.message.clone()).separator(next(""));
            div()
                .flex()
                .items_start()
                .gap(spacing.sm)
                .min_w_0()
                .px(spacing.xs)
                .py(px(1.))
                .children(time.map(|time| div().flex_none().min_w(px(32.)).child(time)))
                .children(level.map(|level| div().flex_none().w(px(44.)).child(level)))
                .children(source.map(|source| {
                    div()
                        .flex_none()
                        .w(self.source_width)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(source)
                }))
                .child(div().flex_1().min_w_0().child(message))
        });
        SelectionScope::new(self.id.clone())
            .when_some(self.scroll.as_ref(), |scope, handle| scope.scroll(handle))
            .flex()
            .flex_col()
            .min_w_0()
            .children(rows)
    }
}
