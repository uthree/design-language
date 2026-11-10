//! A table: rows of short values under column headings, read and edited as text — an event
//! list beside a piano roll, a list of markers, a parameter sheet.
//!
//! Only the rows on screen are built, so a clip with thousands of events scrolls as freely as
//! one with ten. A row is selected by a click or the arrow keys; a double-click on a cell (or
//! Enter on the selected row) asks the application to edit it, which it does with an
//! [`crate::InlineEdit`] over the cell: give the table a [`SpotBook`] and every cell records
//! where it is under [`Table::cell_key`].

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, Hsla, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, RenderOnce, ScrollStrategy, SharedString, StatefulInteractiveElement,
    Styled, UniformListScrollHandle, Window, div, prelude::FluentBuilder, px, uniform_list,
};

use crate::focus::{
    CONTROL_CONTEXT, LIST_CONTEXT, SelectFirst, SelectLast, SelectNext, SelectPrevious, focusable,
};
use crate::inline_edit::SpotBook;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::{ActiveTheme, Theme};

/// How wide a column is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColumnWidth {
    /// Exactly this wide: numbers, positions, short codes.
    Fixed(Pixels),
    /// A share of what the fixed columns leave: names and descriptions.
    Flex(f32),
}

/// One column of a [`Table`]: its heading, its width and how its values sit.
#[derive(Clone, Debug)]
pub struct Column {
    title: SharedString,
    width: ColumnWidth,
    end: bool,
    mono: bool,
}

impl Column {
    /// A column headed `title`, sharing the free width with the other flexible columns.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            width: ColumnWidth::Flex(1.),
            end: false,
            mono: false,
        }
    }

    /// A fixed width.
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = ColumnWidth::Fixed(width);
        self
    }

    /// A larger or smaller share of the free width than 1.
    pub fn grow(mut self, share: f32) -> Self {
        self.width = ColumnWidth::Flex(share.max(0.01));
        self
    }

    /// Values (and the heading) against the column's trailing edge, as numbers sit.
    pub fn end(mut self) -> Self {
        self.end = true;
        self
    }

    /// Values in the mono face, so digits line up from row to row.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }
}

/// What marks a row down its leading edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RowMark {
    /// The theme's categorical colour `n` ([`Theme::categorical`]): a family of rows, such as
    /// notes, controllers and pitch bends.
    Category(usize),
    /// The accent.
    Accent,
    /// The AI's colour, for rows an agent wrote.
    Agent,
}

impl RowMark {
    fn color(self, theme: &Theme) -> Hsla {
        match self {
            Self::Category(index) => theme.categorical(index),
            Self::Accent => theme.colors.accent,
            Self::Agent => theme.colors.agent,
        }
    }
}

/// One row of a [`Table`].
#[derive(Clone, Debug, Default)]
pub struct TableRow {
    cells: Vec<SharedString>,
    mark: Option<RowMark>,
    tinted: bool,
    active: bool,
    faint: bool,
    selected: bool,
}

impl TableRow {
    /// A row of `cells`, one per column.
    pub fn new<S: Into<SharedString>>(cells: impl IntoIterator<Item = S>) -> Self {
        Self {
            cells: cells.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }

    /// A stripe down the leading edge in `mark`'s colour. It stays when the row is selected.
    pub fn mark(mut self, mark: RowMark) -> Self {
        self.mark = Some(mark);
        self
    }

    /// Set the row's text in its mark's colour too, kept readable against the surface.
    pub fn tinted(mut self) -> Self {
        self.tinted = true;
        self
    }

    /// The row is happening now: the event being heard while the transport rolls.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// The row recedes: an event past the end of its clip, a muted marker.
    pub fn faint(mut self, faint: bool) -> Self {
        self.faint = faint;
        self
    }

    /// The row is selected. Several may be.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

/// A cell of a [`Table`], by row and column index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TableCell {
    /// The row, from 0.
    pub row: usize,
    /// The column, from 0.
    pub column: usize,
}

type SelectHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;
type ActivateHandler = Rc<dyn Fn(&TableCell, &mut Window, &mut App)>;

/// Rows of short values under column headings, only the visible rows built.
///
/// ```ignore
/// Table::new("events", columns, rows)
///     .track_scroll(&self.scroll)
///     .spots(&self.spots)
///     .on_select(cx.listener(|this, row: &usize, _, cx| this.select(*row, cx)))
///     .on_activate(cx.listener(|this, cell: &TableCell, _, cx| this.edit(*cell, cx)))
/// // Then, while editing, over the cell:
/// self.spots.get(&Table::cell_key("events", cell), window)
///     .map(|over| InlineEdit::new("edit-cell", &self.field, over) /* … */)
/// ```
///
/// A click selects a row ([`Table::on_select`]); a double-click on a cell, or Enter on the
/// selected row, activates it ([`Table::on_activate`], the first column for Enter). While the
/// table holds focus, Up and Down move the selection and Home and End go to the ends, taken
/// before any key binding, and the table keeps the row it moves to in view. To walk down a
/// column the way a list editor does, commit the field and activate the same column of the
/// next row: [`UniformListScrollHandle::scroll_to_item`] brings it on screen first.
#[derive(IntoElement)]
pub struct Table {
    id: ElementId,
    columns: Vec<Column>,
    rows: Rc<Vec<TableRow>>,
    row_height: Option<Pixels>,
    scroll: Option<UniformListScrollHandle>,
    spots: Option<SpotBook>,
    empty: Option<SharedString>,
    on_select: Option<SelectHandler>,
    on_activate: Option<ActivateHandler>,
}

impl Table {
    /// A table of `rows` under `columns`.
    pub fn new(
        id: impl Into<ElementId>,
        columns: impl IntoIterator<Item = Column>,
        rows: impl IntoIterator<Item = TableRow>,
    ) -> Self {
        Self {
            id: id.into(),
            columns: columns.into_iter().collect(),
            rows: Rc::new(rows.into_iter().collect()),
            row_height: None,
            scroll: None,
            spots: None,
            empty: None,
            on_select: None,
            on_activate: None,
        }
    }

    /// The key a cell records its rectangle under in the table's [`SpotBook`].
    pub fn cell_key(id: impl std::fmt::Display, cell: TableCell) -> SharedString {
        format!("{id}:{}:{}", cell.row, cell.column).into()
    }

    /// Row height. [`crate::Density::control_sm`] by default.
    pub fn row_height(mut self, height: Pixels) -> Self {
        self.row_height = Some(height);
        self
    }

    /// Share the table's scroll position, to bring a row into view
    /// ([`UniformListScrollHandle::scroll_to_item`]) or read where the person scrolled.
    pub fn track_scroll(mut self, handle: &UniformListScrollHandle) -> Self {
        self.scroll = Some(handle.clone());
        self
    }

    /// Record each visible cell's rectangle in `book`, under [`Table::cell_key`].
    pub fn spots(mut self, book: &SpotBook) -> Self {
        self.spots = Some(book.clone());
        self
    }

    /// What the table says when it has no rows.
    pub fn empty(mut self, text: impl Into<SharedString>) -> Self {
        self.empty = Some(text.into());
        self
    }

    /// Called with the row a click or an arrow key selects.
    pub fn on_select(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Called with the cell a double-click (or Enter on the selected row) asks to edit.
    pub fn on_activate(
        mut self,
        handler: impl Fn(&TableCell, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }
}

/// The width of the stripe down a marked row's leading edge.
const STRIPE: Pixels = px(3.);

fn cell(column: &Column, theme: &Theme) -> gpui::Div {
    let base = div()
        .relative()
        .h_full()
        .px(theme.spacing.xs * 1.5)
        .flex()
        .items_center()
        .overflow_hidden()
        .whitespace_nowrap()
        .when(column.end, |cell| cell.justify_end());
    match column.width {
        ColumnWidth::Fixed(width) => base.w(width).flex_none(),
        ColumnWidth::Flex(share) => {
            base.flex_grow()
                .flex_basis(px(0.))
                .min_w_0()
                .map(|mut cell| {
                    cell.style().flex_grow = Some(share);
                    cell
                })
        }
    }
}

impl RenderOnce for Table {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let row_height = self.row_height.unwrap_or(theme.density.control_sm);
        let marked = self.rows.iter().any(|row| row.mark.is_some());
        // The table keeps a scroll position of its own when the application shares none, so the
        // arrow keys can still bring the row they reach into view.
        let scroll = self.scroll.clone().unwrap_or_else(|| {
            window
                .use_keyed_state(
                    ElementId::NamedChild(Box::new(self.id.clone()), "scroll".into()),
                    cx,
                    |_, _| UniformListScrollHandle::new(),
                )
                .read(cx)
                .clone()
        });
        let columns = Rc::new(self.columns);

        let heading = div()
            .flex()
            .flex_none()
            .w_full()
            .h(row_height + px(4.))
            .border_b_1()
            .border_color(theme.colors.separator)
            .when(marked, |heading| heading.child(div().w(STRIPE).flex_none()))
            .children(columns.iter().map(|column| {
                cell(column, &theme).child(
                    Label::new(column.title.clone())
                        .size(TextSize::XSmall)
                        .color(TextColor::Muted),
                )
            }));

        let count = self.rows.len();
        let selected = self.rows.iter().position(|row| row.selected);
        let body = if count == 0 {
            div()
                .flex_1()
                .p(theme.spacing.sm)
                .children(self.empty.clone().map(|empty| {
                    Label::new(empty)
                        .size(TextSize::Small)
                        .color(TextColor::Faint)
                }))
                .into_any_element()
        } else {
            let rows = self.rows.clone();
            let id = self.id.clone();
            let spots = self.spots.clone();
            let on_select = self.on_select.clone();
            let on_activate = self.on_activate.clone();
            let columns = columns.clone();
            uniform_list(
                ElementId::NamedChild(Box::new(self.id.clone()), "rows".into()),
                count,
                move |range, _, cx| {
                    let theme = cx.theme().clone();
                    range
                        .map(|index| {
                            let row = &rows[index];
                            let mark = row.mark.map(|mark| mark.color(&theme));
                            let text = if row.faint {
                                theme.colors.text_faint
                            } else if let (true, Some(mark)) = (row.tinted, mark) {
                                crate::theme::ensure_contrast(mark, theme.colors.surface, 4.5)
                            } else {
                                theme.colors.text
                            };
                            let on_select = on_select.clone();
                            div()
                                .id(ElementId::NamedChild(
                                    Box::new(id.clone()),
                                    format!("row-{index}").into(),
                                ))
                                .debug_selector({
                                    let id = id.clone();
                                    move || format!("{id}-row-{index}")
                                })
                                .flex()
                                .w_full()
                                .h(row_height)
                                .text_size(theme.typography.size_sm)
                                .text_color(text)
                                .when(row.active && !row.selected, |el| {
                                    el.bg(theme.colors.accent_wash)
                                })
                                .when(row.selected, |el| el.bg(theme.colors.selected))
                                .when(!row.selected, |el| {
                                    let hover = theme.colors.hover;
                                    el.hover(move |style| style.bg(hover))
                                })
                                .when(marked, |el| {
                                    el.child(
                                        div()
                                            .w(STRIPE)
                                            .flex_none()
                                            .h_full()
                                            .when_some(mark, |stripe, mark| stripe.bg(mark)),
                                    )
                                })
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    if let Some(handler) = &on_select {
                                        handler(&index, window, cx);
                                    }
                                })
                                .children(columns.iter().enumerate().map(
                                    |(column_index, column)| {
                                        let value = row
                                            .cells
                                            .get(column_index)
                                            .cloned()
                                            .unwrap_or_default();
                                        let at = TableCell {
                                            row: index,
                                            column: column_index,
                                        };
                                        let on_activate = on_activate.clone();
                                        cell(column, &theme)
                                            .when(column.mono, |cell| {
                                                cell.font(crate::theme::mono_font())
                                            })
                                            .child(value)
                                            .when_some(spots.as_ref(), |cell, spots| {
                                                cell.child(spots.spot(Table::cell_key(&id, at)))
                                            })
                                            .id(ElementId::NamedChild(
                                                Box::new(id.clone()),
                                                format!("cell-{index}-{column_index}").into(),
                                            ))
                                            .debug_selector({
                                                let id = id.clone();
                                                move || format!("{id}-cell-{index}-{column_index}")
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                move |event, window, cx| {
                                                    if event.click_count >= 2
                                                        && let Some(handler) = &on_activate
                                                    {
                                                        handler(&at, window, cx);
                                                    }
                                                },
                                            )
                                    },
                                ))
                                .into_any_element()
                        })
                        .collect::<Vec<AnyElement>>()
                },
            )
            .track_scroll(scroll.clone())
            .flex_1()
            .min_h_0()
            .into_any_element()
        };

        let step = {
            let on_select = self.on_select.clone();
            let scroll = scroll.clone();
            move |to: usize, window: &mut Window, cx: &mut App| {
                scroll.scroll_to_item(to, ScrollStrategy::Top);
                if let Some(handler) = &on_select {
                    handler(&to, window, cx);
                }
            }
        };
        let step = Rc::new(step);
        let last = count.saturating_sub(1);
        let table = div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(heading)
            .child(body);
        let table = focusable(table, &self.id, false, theme.shape.radius_sm, window, cx)
            .key_context(format!("{CONTROL_CONTEXT} {LIST_CONTEXT}").as_str());
        table
            .when(count > 0, |table| {
                let (previous, next, first, end) =
                    (step.clone(), step.clone(), step.clone(), step.clone());
                table
                    .on_action(move |_: &SelectPrevious, window, cx| {
                        previous(
                            selected.map_or(last, |row| row.saturating_sub(1)),
                            window,
                            cx,
                        )
                    })
                    .on_action(move |_: &SelectNext, window, cx| {
                        next(selected.map_or(0, |row| (row + 1).min(last)), window, cx)
                    })
                    .on_action(move |_: &SelectFirst, window, cx| first(0, window, cx))
                    .on_action(move |_: &SelectLast, window, cx| end(last, window, cx))
            })
            .on_click({
                let on_activate = self.on_activate.clone();
                move |event: &ClickEvent, window, cx| {
                    if event.is_keyboard()
                        && let (Some(row), Some(handler)) = (selected, &on_activate)
                    {
                        handler(&TableCell { row, column: 0 }, window, cx);
                    }
                }
            })
    }
}
