//! Code the agent shows or changes: command output, file excerpts and diffs.

use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, prelude::FluentBuilder, px,
};

use crate::button::CopyButton;
use crate::icon::{Icon, IconName};
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

/// The header both blocks share: an icon, a title, trailing details.
fn header(
    icon: IconName,
    title: SharedString,
    trailing: Vec<AnyElement>,
    cx: &App,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    div()
        .flex()
        .items_center()
        .gap(theme.spacing.sm)
        .h(theme.density.control_md)
        .px(theme.spacing.md)
        .border_b_1()
        .border_color(theme.colors.border)
        .child(Icon::new(icon).size(px(13.)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font(theme.typography.mono.clone())
                .text_size(theme.typography.size_sm)
                .text_color(theme.colors.text_muted)
                .child(title),
        )
        .children(trailing)
}

/// A block of code or command output, with an optional title bar.
#[derive(IntoElement)]
pub struct CodeBlock {
    id: ElementId,
    code: SharedString,
    title: Option<SharedString>,
    icon: IconName,
    line_numbers: Option<usize>,
    copy: bool,
}

impl CodeBlock {
    /// A block of `code`.
    pub fn new(id: impl Into<ElementId>, code: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            code: code.into(),
            title: None,
            icon: IconName::Code,
            line_numbers: None,
            copy: false,
        }
    }

    /// A title bar: a file path, or the command that produced the output.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Mark it as terminal output.
    pub fn terminal(mut self) -> Self {
        self.icon = IconName::Code;
        self.title = self.title.or(Some("Output".into()));
        self
    }

    /// Number the lines, starting at `first`.
    pub fn line_numbers(mut self, first: usize) -> Self {
        self.line_numbers = Some(first);
        self
    }

    /// Offer a [`CopyButton`] in the title bar, which copies the code.
    pub fn copyable(mut self) -> Self {
        self.copy = true;
        self
    }
}

impl RenderOnce for CodeBlock {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let copy = self.copy.then(|| {
            CopyButton::new((self.id.clone(), "copy"), self.code.clone()).into_any_element()
        });
        let lines: Vec<AnyElement> = self
            .code
            .lines()
            .enumerate()
            .map(|(index, line)| {
                div()
                    .flex()
                    .gap(theme.spacing.md)
                    .children(self.line_numbers.map(|first| {
                        div()
                            .w(px(28.))
                            .flex_none()
                            .flex()
                            .justify_end()
                            .text_color(colors.text_faint)
                            .child(format!("{}", first + index))
                    }))
                    .child(div().flex_1().min_w_0().child(line.to_owned()))
                    .into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .rounded(theme.shape.radius_md)
            .bg(colors.sunken)
            .border_1()
            .border_color(colors.border)
            .overflow_hidden()
            .when_some(self.title, |block, title| {
                block.child(header(self.icon, title, copy.into_iter().collect(), cx))
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .px(theme.spacing.md)
                    .py(theme.spacing.sm)
                    .font(theme.typography.mono.clone())
                    .text_size(theme.typography.size_sm)
                    .text_color(colors.text)
                    .children(lines),
            )
    }
}

/// One line of a diff.
#[derive(Clone, Debug, PartialEq)]
pub enum DiffLine {
    /// Unchanged context, with its line number in the new file.
    Context(usize, SharedString),
    /// An added line, with its number in the new file.
    Added(usize, SharedString),
    /// A removed line, with its number in the old file.
    Removed(usize, SharedString),
    /// A hunk header or an elision: `@@ -40,6 +40,9 @@`.
    Hunk(SharedString),
}

/// A file change: path, line counts, and the hunks.
#[derive(IntoElement)]
pub struct DiffBlock {
    path: SharedString,
    lines: Vec<DiffLine>,
    trailing: Vec<AnyElement>,
}

impl DiffBlock {
    /// A diff of the file at `path`.
    pub fn new(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            lines: Vec::new(),
            trailing: Vec::new(),
        }
    }

    /// Add a line.
    pub fn line(mut self, line: DiffLine) -> Self {
        self.lines.push(line);
        self
    }

    /// Add lines.
    pub fn lines(mut self, lines: impl IntoIterator<Item = DiffLine>) -> Self {
        self.lines.extend(lines);
        self
    }

    /// A control in the header, for example "Open" or "Revert".
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.trailing.push(action.into_any_element());
        self
    }
}

impl RenderOnce for DiffBlock {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let added = self
            .lines
            .iter()
            .filter(|line| matches!(line, DiffLine::Added(..)))
            .count();
        let removed = self
            .lines
            .iter()
            .filter(|line| matches!(line, DiffLine::Removed(..)))
            .count();
        let mut trailing = vec![
            Label::new(format!("+{added}"))
                .size(TextSize::Small)
                .mono()
                .color(TextColor::Success)
                .into_any_element(),
            Label::new(format!("−{removed}"))
                .size(TextSize::Small)
                .mono()
                .color(TextColor::Danger)
                .into_any_element(),
        ];
        trailing.extend(self.trailing);

        let rows: Vec<AnyElement> = self
            .lines
            .into_iter()
            .map(|line| {
                let (number, sign, text, background, sign_color) = match line {
                    DiffLine::Context(number, text) => {
                        (Some(number), " ", text, None, colors.text_faint)
                    }
                    DiffLine::Added(number, text) => (
                        Some(number),
                        "+",
                        text,
                        Some(colors.success_wash),
                        colors.success,
                    ),
                    DiffLine::Removed(number, text) => (
                        Some(number),
                        "−",
                        text,
                        Some(colors.danger_wash),
                        colors.danger,
                    ),
                    DiffLine::Hunk(text) => (None, "", text, Some(colors.info_wash), colors.info),
                };
                let hunk = number.is_none();
                div()
                    .flex()
                    .gap(theme.spacing.sm)
                    .px(theme.spacing.sm)
                    .when_some(background, |row, background| row.bg(background))
                    .child(
                        div()
                            .w(px(28.))
                            .flex_none()
                            .flex()
                            .justify_end()
                            .text_color(colors.text_faint)
                            .children(number.map(|number| format!("{number}"))),
                    )
                    .child(
                        div()
                            .w(px(10.))
                            .flex_none()
                            .text_color(sign_color)
                            .child(sign),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(if hunk { colors.info } else { colors.text })
                            .child(text),
                    )
                    .into_any_element()
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .rounded(theme.shape.radius_md)
            .bg(colors.sunken)
            .border_1()
            .border_color(colors.border)
            .overflow_hidden()
            .child(header(IconName::File, self.path, trailing, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .py(theme.spacing.xs)
                    .font(theme.typography.mono.clone())
                    .text_size(theme.typography.size_sm)
                    .children(rows),
            )
    }
}
