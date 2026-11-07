//! Markdown as an agent writes it: paragraphs, headings, lists, quotes, code, tables and links.

use std::ops::Range;

use gpui::{
    AnyElement, App, ElementId, FontStyle, FontWeight, HighlightStyle, IntoElement, ParentElement,
    RenderOnce, SharedString, StrikethroughStyle, Styled, StyledText, UnderlineStyle, Window, div,
    prelude::FluentBuilder, px,
};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use crate::button::CopyButton;
use crate::icon::{Icon, IconName};
use crate::selectable::{LinkHandler, SelectableText, merge};
use crate::text::{Label, TextColor, TextSize};
use crate::theme::{ActiveTheme, Theme};

/// Markdown, drawn in Atelier's type: what an agent's answer, a plan or a release note is
/// written in.
///
/// It reads CommonMark with GitHub's tables, task lists and strikethrough: paragraphs,
/// headings, bullet, numbered and task lists (nested), quotes, fenced code with its language
/// and a copy button, tables, rules, **bold**, *italic*, ~~struck~~, `code` and links. Raw HTML
/// is shown as text, never interpreted.
///
/// Each paragraph, heading, list item and code block is a [`SelectableText`] the person can
/// select and copy, with ids `{id}-block-1`, `{id}-block-2`… in reading order, and a click on a
/// link opens it (or calls [`Markdown::on_link`]). [`Markdown::plain`] draws the same without
/// selection, for text that changes every frame or has no stable id.
///
/// ```ignore
/// Markdown::new("answer-7", "Two fixes:\n\n1. Quote the path\n2. Retry on `EBUSY`")
/// ```
#[derive(IntoElement)]
pub struct Markdown {
    id: ElementId,
    source: SharedString,
    plain: bool,
    on_link: Option<LinkHandler>,
}

impl Markdown {
    /// `source` as Markdown. `id` keeps each block's selection from frame to frame.
    pub fn new(id: impl Into<ElementId>, source: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            plain: false,
            on_link: None,
        }
    }

    /// Draw without selection, links that open or copy buttons.
    pub fn plain(mut self) -> Self {
        self.plain = true;
        self
    }

    /// Follow links yourself, for example to open a file of the project in the application,
    /// instead of opening them with the system's handler.
    pub fn on_link(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_link = Some(std::rc::Rc::new(handler));
        self
    }
}

/// A run of text drawn differently from the paragraph around it.
#[derive(Clone, Debug, PartialEq)]
enum Run {
    Strong,
    Emphasis,
    Strike,
    Code,
    Link(SharedString),
}

/// One paragraph's text and its runs, as byte ranges of the text.
#[derive(Clone, Debug, Default, PartialEq)]
struct Inline {
    text: String,
    runs: Vec<(Range<usize>, Run)>,
}

#[derive(Debug, PartialEq)]
enum Block {
    Paragraph(Inline),
    Heading(u8, Inline),
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    Quote(Vec<Block>),
    Code {
        language: Option<String>,
        code: String,
    },
    Rule,
    Table {
        head: Vec<Inline>,
        rows: Vec<Vec<Inline>>,
    },
}

#[derive(Debug, PartialEq)]
struct Item {
    task: Option<bool>,
    blocks: Vec<Block>,
}

enum Frame {
    Root(Vec<Block>),
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    Item(Item),
}

enum Pending {
    Paragraph,
    Heading(u8),
}

#[derive(Default)]
struct Table {
    head: Vec<Inline>,
    rows: Vec<Vec<Inline>>,
    row: Vec<Inline>,
}

/// Builds blocks from the parser's events.
struct Builder {
    stack: Vec<Frame>,
    inline: Option<(Pending, Inline)>,
    open: Vec<(Run, usize)>,
    code: Option<(Option<String>, String)>,
    table: Option<Table>,
    cell: Option<Inline>,
}

impl Builder {
    fn push(&mut self, block: Block) {
        match self.stack.last_mut() {
            Some(Frame::Root(blocks) | Frame::Quote(blocks) | Frame::Item(Item { blocks, .. })) => {
                blocks.push(block)
            }
            // Blocks sit in items, never straight in a list; keep any stray one anyway.
            Some(Frame::List { items, .. }) => items.push(Item {
                task: None,
                blocks: vec![block],
            }),
            None => self.stack.push(Frame::Root(vec![block])),
        }
    }

    /// End the paragraph or heading under way, if any.
    fn flush(&mut self) {
        if let Some((pending, inline)) = self.inline.take() {
            self.open.clear();
            if inline.text.trim().is_empty() {
                return;
            }
            self.push(match pending {
                Pending::Paragraph => Block::Paragraph(inline),
                Pending::Heading(level) => Block::Heading(level, inline),
            });
        }
    }

    /// Where text goes now: a table cell, or the paragraph under way, which a tight list
    /// item's bare text starts.
    fn text(&mut self) -> &mut Inline {
        if let Some(cell) = self.cell.as_mut() {
            return cell;
        }
        &mut self
            .inline
            .get_or_insert_with(|| (Pending::Paragraph, Inline::default()))
            .1
    }

    fn open(&mut self, run: Run) {
        let at = self.text().text.len();
        self.open.push((run, at));
    }

    fn close(&mut self) {
        if let Some((run, start)) = self.open.pop() {
            let text = self.text();
            if start < text.text.len() {
                text.runs.push((start..text.text.len(), run));
            }
        }
    }

    fn event(&mut self, event: Event) {
        if let Some((_, code)) = self.code.as_mut() {
            match event {
                Event::Text(text) => code.push_str(&text),
                Event::End(TagEnd::CodeBlock) => {
                    let (language, mut code) = self.code.take().unwrap_or_default();
                    while code.ends_with('\n') {
                        code.pop();
                    }
                    self.push(Block::Code { language, code });
                }
                _ => {}
            }
            return;
        }
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                self.text().text.push_str(&text)
            }
            Event::Code(code) | Event::InlineMath(code) | Event::DisplayMath(code) => {
                let inline = self.text();
                let start = inline.text.len();
                inline.text.push_str(&code);
                inline.runs.push((start..inline.text.len(), Run::Code));
            }
            Event::FootnoteReference(name) => {
                let text = format!("[{name}]");
                self.text().text.push_str(&text);
            }
            Event::SoftBreak => self.text().text.push(' '),
            Event::HardBreak => self.text().text.push('\n'),
            Event::Rule => {
                self.flush();
                self.push(Block::Rule);
            }
            Event::TaskListMarker(done) => {
                if let Some(Frame::Item(item)) = self.stack.last_mut() {
                    item.task = Some(done);
                }
            }
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph | Tag::HtmlBlock => {
                self.flush();
                self.inline = Some((Pending::Paragraph, Inline::default()));
            }
            Tag::Heading { level, .. } => {
                self.flush();
                self.inline = Some((Pending::Heading(level as u8), Inline::default()));
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.stack.push(Frame::Quote(Vec::new()));
            }
            Tag::CodeBlock(kind) => {
                self.flush();
                let language = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .map(str::to_owned)
                        .filter(|language| !language.is_empty()),
                    CodeBlockKind::Indented => None,
                };
                self.code = Some((language, String::new()));
            }
            Tag::List(start) => {
                self.flush();
                self.stack.push(Frame::List {
                    start,
                    items: Vec::new(),
                });
            }
            Tag::Item => {
                self.flush();
                self.stack.push(Frame::Item(Item {
                    task: None,
                    blocks: Vec::new(),
                }));
            }
            Tag::Table(_) => {
                self.flush();
                self.table = Some(Table::default());
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some(table) = self.table.as_mut() {
                    table.row.clear();
                }
            }
            Tag::TableCell => self.cell = Some(Inline::default()),
            Tag::Emphasis => self.open(Run::Emphasis),
            Tag::Strong => self.open(Run::Strong),
            Tag::Strikethrough => self.open(Run::Strike),
            Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                self.open(Run::Link(dest_url.to_string().into()))
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::HtmlBlock => self.flush(),
            TagEnd::BlockQuote(_) => {
                self.flush();
                if let Some(Frame::Quote(blocks)) = self.stack.pop() {
                    self.push(Block::Quote(blocks));
                }
            }
            TagEnd::List(_) => {
                self.flush();
                if let Some(Frame::List { start, items }) = self.stack.pop() {
                    self.push(Block::List { start, items });
                }
            }
            TagEnd::Item => {
                self.flush();
                if let Some(Frame::Item(item)) = self.stack.pop() {
                    match self.stack.last_mut() {
                        Some(Frame::List { items, .. }) => items.push(item),
                        _ => {
                            for block in item.blocks {
                                self.push(block);
                            }
                        }
                    }
                }
            }
            TagEnd::TableCell => {
                if let (Some(cell), Some(table)) = (self.cell.take(), self.table.as_mut()) {
                    table.row.push(cell);
                }
            }
            TagEnd::TableHead => {
                if let Some(table) = self.table.as_mut() {
                    table.head = std::mem::take(&mut table.row);
                }
            }
            TagEnd::TableRow => {
                if let Some(table) = self.table.as_mut() {
                    let row = std::mem::take(&mut table.row);
                    table.rows.push(row);
                }
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.push(Block::Table {
                        head: table.head,
                        rows: table.rows,
                    });
                }
            }
            TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Link
            | TagEnd::Image => self.close(),
            _ => {}
        }
    }
}

fn parse(source: &str) -> Vec<Block> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let mut builder = Builder {
        stack: vec![Frame::Root(Vec::new())],
        inline: None,
        open: Vec::new(),
        code: None,
        table: None,
        cell: None,
    };
    for event in Parser::new_ext(source, options) {
        builder.event(event);
    }
    // A code fence still open while an answer streams in shows what has arrived so far.
    if let Some((language, code)) = builder.code.take() {
        builder.push(Block::Code {
            language,
            code: code.trim_end_matches('\n').to_owned(),
        });
    }
    builder.flush();
    while builder.stack.len() > 1 {
        match builder.stack.pop() {
            Some(Frame::Quote(blocks)) => builder.push(Block::Quote(blocks)),
            Some(Frame::List { start, items }) => builder.push(Block::List { start, items }),
            Some(Frame::Item(item)) => match builder.stack.last_mut() {
                Some(Frame::List { items, .. }) => items.push(item),
                _ => {
                    for block in item.blocks {
                        builder.push(block);
                    }
                }
            },
            _ => {}
        }
    }
    match builder.stack.pop() {
        Some(Frame::Root(blocks)) => blocks,
        _ => Vec::new(),
    }
}

/// Draws blocks, numbering the selectable ones in reading order.
struct Painter<'a> {
    id: &'a ElementId,
    plain: bool,
    on_link: Option<LinkHandler>,
    count: usize,
    theme: &'a Theme,
}

impl Painter<'_> {
    fn next_id(&mut self, part: &str) -> ElementId {
        self.count += 1;
        ElementId::NamedChild(
            Box::new(self.id.clone()),
            format!("{part}-{}", self.count).into(),
        )
    }

    fn styles(&self, inline: &Inline) -> Vec<(Range<usize>, HighlightStyle)> {
        let colors = &self.theme.colors;
        inline
            .runs
            .iter()
            .map(|(range, run)| {
                let style = match run {
                    Run::Strong => HighlightStyle {
                        font_weight: Some(FontWeight::BOLD),
                        ..Default::default()
                    },
                    Run::Emphasis => HighlightStyle {
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    },
                    Run::Strike => HighlightStyle {
                        strikethrough: Some(StrikethroughStyle {
                            thickness: px(1.),
                            color: None,
                        }),
                        ..Default::default()
                    },
                    Run::Code => HighlightStyle {
                        background_color: Some(colors.hover),
                        ..Default::default()
                    },
                    Run::Link(_) => HighlightStyle {
                        color: Some(colors.accent_text),
                        underline: Some(UnderlineStyle {
                            thickness: px(1.),
                            color: Some(colors.accent_text.opacity(0.45)),
                            wavy: false,
                        }),
                        ..Default::default()
                    },
                };
                (range.clone(), style)
            })
            .collect()
    }

    fn text(&mut self, inline: &Inline, size: TextSize, color: TextColor) -> AnyElement {
        let styles = self.styles(inline);
        if self.plain {
            return div()
                .min_w_0()
                .text_size(size.pixels(self.theme))
                .text_color(color.resolve(self.theme))
                .child(
                    StyledText::new(inline.text.clone())
                        .with_highlights(merge(styles, inline.text.len())),
                )
                .into_any_element();
        }
        let mut text = SelectableText::new(self.next_id("block"), inline.text.clone())
            .size(size)
            .color(color)
            .styles(styles)
            .on_link_shared(self.on_link.clone());
        for (range, run) in &inline.runs {
            if let Run::Link(url) = run {
                text = text.link(range.clone(), url.clone());
            }
        }
        div().min_w_0().child(text).into_any_element()
    }

    fn blocks(&mut self, blocks: &[Block], color: TextColor, depth: usize) -> Vec<AnyElement> {
        blocks
            .iter()
            .map(|block| self.block(block, color, depth))
            .collect()
    }

    fn block(&mut self, block: &Block, color: TextColor, depth: usize) -> AnyElement {
        let theme = self.theme;
        let colors = &theme.colors;
        match block {
            Block::Paragraph(inline) => self.text(inline, TextSize::Medium, color),
            Block::Heading(level, inline) => {
                let size = match level {
                    1 => TextSize::XLarge,
                    2 => TextSize::Large,
                    _ => TextSize::Medium,
                };
                div()
                    .when(*level == 1, |heading| heading.pt(theme.spacing.xs))
                    .font_weight(theme.typography.heading_weight)
                    .child(self.text(inline, size, color))
                    .into_any_element()
            }
            Block::List { start, items } => {
                let rows: Vec<AnyElement> = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let marker = match (item.task, start) {
                            (Some(done), _) => task_box(done, theme).into_any_element(),
                            (None, Some(first)) => div()
                                .min_w(px(14.))
                                .text_color(colors.text_muted)
                                .child(format!("{}.", first + index as u64))
                                .into_any_element(),
                            (None, None) => div()
                                .w(px(14.))
                                .flex()
                                .justify_center()
                                .text_color(colors.text_faint)
                                .child(if depth.is_multiple_of(2) {
                                    "•"
                                } else {
                                    "◦"
                                })
                                .into_any_element(),
                        };
                        let color = match item.task {
                            Some(true) => TextColor::Muted,
                            _ => color,
                        };
                        div()
                            .flex()
                            .gap(theme.spacing.sm)
                            .min_w_0()
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(theme.typography.size_md)
                                    .child(marker),
                            )
                            .child(
                                stack(
                                    self.blocks(&item.blocks, color, depth + 1),
                                    theme.spacing.xs,
                                )
                                .flex_1(),
                            )
                            .into_any_element()
                    })
                    .collect();
                stack(rows, theme.spacing.xs).into_any_element()
            }
            Block::Quote(blocks) => stack(
                self.blocks(blocks, TextColor::Muted, depth),
                theme.spacing.sm,
            )
            .pl(theme.spacing.md)
            .border_l_2()
            .border_color(colors.border_strong)
            .into_any_element(),
            Block::Code { language, code } => self.code(language.as_deref(), code),
            Block::Rule => div()
                .h(px(1.))
                .my(theme.spacing.xs)
                .bg(colors.separator)
                .into_any_element(),
            Block::Table { head, rows } => {
                let columns = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
                let row = |cells: &[Inline], header: bool, painter: &mut Self| {
                    div()
                        .flex()
                        .min_w_0()
                        .when(header, |row| {
                            row.bg(colors.sunken)
                                .font_weight(theme.typography.label_weight)
                        })
                        .when(!header, |row| {
                            row.border_t_1().border_color(colors.separator)
                        })
                        .children((0..columns).map(|column| {
                            let cell = cells.get(column).cloned().unwrap_or_default();
                            div()
                                .flex_1()
                                .flex_basis(px(0.))
                                .min_w_0()
                                .px(theme.spacing.sm)
                                .py(theme.spacing.xs)
                                .child(painter.text(&cell, TextSize::Small, color))
                        }))
                        .into_any_element()
                };
                let mut table = div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .rounded(theme.shape.radius_md)
                    .border_1()
                    .border_color(colors.border)
                    .overflow_hidden();
                if !head.is_empty() {
                    table = table.child(row(head, true, self));
                }
                for cells in rows {
                    table = table.child(row(cells, false, self));
                }
                table.into_any_element()
            }
        }
    }

    fn code(&mut self, language: Option<&str>, code: &str) -> AnyElement {
        let theme = self.theme;
        let colors = &theme.colors;
        let copy = (!self.plain).then(|| {
            CopyButton::new(
                ElementId::NamedChild(
                    Box::new(self.id.clone()),
                    format!("copy-{}", self.count + 1).into(),
                ),
                SharedString::from(code.to_owned()),
            )
        });
        let body = if self.plain {
            div().child(code.to_owned()).into_any_element()
        } else {
            SelectableText::new(self.next_id("block"), code.to_owned())
                .mono()
                .size(TextSize::Small)
                .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .min_w_0()
            .rounded(theme.shape.radius_md)
            .bg(colors.sunken)
            .border_1()
            .border_color(colors.border)
            .overflow_hidden()
            .when(language.is_some() || copy.is_some(), |block| {
                block.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme.spacing.sm)
                        .pl(theme.spacing.md)
                        .pr(theme.spacing.xs)
                        .py(theme.spacing.xxs)
                        .border_b_1()
                        .border_color(colors.separator)
                        .child(div().flex_1().min_w_0().children(language.map(|language| {
                            Label::new(language.to_owned())
                                .size(TextSize::XSmall)
                                .color(TextColor::Faint)
                        })))
                        .children(copy),
                )
            })
            .child(
                div()
                    .px(theme.spacing.md)
                    .py(theme.spacing.sm)
                    .font(theme.typography.mono.clone())
                    .text_size(theme.typography.size_sm)
                    .text_color(colors.text)
                    .child(body),
            )
            .into_any_element()
    }
}

/// The box before a task list item: ticked when done.
fn task_box(done: bool, theme: &Theme) -> impl IntoElement + use<> {
    let colors = &theme.colors;
    div()
        .mt(px(3.))
        .size(px(14.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme.shape.radius_xs)
        .border_1()
        .when(done, |mark| {
            mark.bg(colors.accent).border_color(colors.accent).child(
                Icon::new(IconName::Check)
                    .size(px(10.))
                    .color(colors.on_accent),
            )
        })
        .when(!done, |mark| mark.border_color(colors.border_strong))
}

/// Blocks one under another, `gap` apart, as a block container rather than a flex column.
///
/// As a flex column a stack of wrapping text took its height from a measurement made before its
/// width was known, when text wraps at every break opportunity. Japanese breaks between any two
/// characters, so a three-line answer measured one character per line and kept that height,
/// thousands of pixels, at its real width, and everything after it in a transcript fell past the
/// end of the scroll range. Block layout gives the width first and sums the blocks as drawn.
fn stack(children: Vec<AnyElement>, gap: gpui::Pixels) -> gpui::Div {
    div()
        .w_full()
        .min_w_0()
        .children(children.into_iter().enumerate().map(|(index, child)| {
            if index == 0 {
                child
            } else {
                div().mt(gap).child(child).into_any_element()
            }
        }))
}

impl RenderOnce for Markdown {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let blocks = parse(&self.source);
        let mut painter = Painter {
            id: &self.id,
            plain: self.plain,
            on_link: self.on_link.clone(),
            count: 0,
            theme,
        };
        let children = painter.blocks(&blocks, TextColor::Default, 0);
        stack(children, theme.spacing.sm)
            .text_size(theme.typography.size_md)
            .text_color(theme.colors.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(block: &Block) -> &str {
        match block {
            Block::Paragraph(inline) | Block::Heading(_, inline) => &inline.text,
            _ => panic!("{block:?} has no text of its own"),
        }
    }

    #[test]
    fn paragraphs_headings_and_runs() {
        let blocks =
            parse("# Fix\n\nQuote **the path** and *retry*,\nsee [docs](https://x.dev) or `ls`.");
        assert_eq!(blocks.len(), 2);
        assert!(matches!(blocks[0], Block::Heading(1, _)));
        let Block::Paragraph(inline) = &blocks[1] else {
            panic!()
        };
        assert_eq!(inline.text, "Quote the path and retry, see docs or ls.");
        let runs: Vec<_> = inline
            .runs
            .iter()
            .map(|(range, run)| (&inline.text[range.clone()], run.clone()))
            .collect();
        assert_eq!(
            runs,
            [
                ("the path", Run::Strong),
                ("retry", Run::Emphasis),
                ("docs", Run::Link("https://x.dev".into())),
                ("ls", Run::Code),
            ]
        );
    }

    #[test]
    fn lists_nest_and_tick() {
        let blocks = parse("1. one\n2. two\n   - inner\n\n- [x] done\n- [ ] open");
        let Block::List { start, items } = &blocks[0] else {
            panic!()
        };
        assert_eq!(*start, Some(1));
        assert_eq!(text(&items[0].blocks[0]), "one");
        assert!(matches!(
            items[1].blocks[1],
            Block::List { start: None, .. }
        ));
        let Block::List { items, .. } = &blocks[1] else {
            panic!()
        };
        assert_eq!(items[0].task, Some(true));
        assert_eq!(items[1].task, Some(false));
    }

    #[test]
    fn code_quotes_tables_and_an_open_fence() {
        let blocks = parse(
            "> careful\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n---\n\n```rust\nfn main() {}\n```\n\n```sh\ncargo te",
        );
        assert!(matches!(&blocks[0], Block::Quote(inner) if text(&inner[0]) == "careful"));
        let Block::Table { head, rows } = &blocks[1] else {
            panic!("{:?}", blocks[1])
        };
        assert_eq!(head[1].text, "b");
        assert_eq!(rows[0][0].text, "1");
        assert_eq!(blocks[2], Block::Rule);
        assert_eq!(
            blocks[3],
            Block::Code {
                language: Some("rust".into()),
                code: "fn main() {}".into()
            }
        );
        assert!(matches!(&blocks[4], Block::Code { code, .. } if code == "cargo te"));
    }

    #[test]
    fn html_stays_text() {
        let blocks = parse("<b>hi</b> there");
        assert_eq!(text(&blocks[0]), "<b>hi</b> there");
    }
}
