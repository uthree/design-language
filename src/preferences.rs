//! The preferences window's body: pages in a sidebar with a search over every setting, one
//! page at a time in a readable column, and the key bindings list.
//!
//! [`PreferencesWindow`] draws what the `preferences` example used to lay out by hand: a
//! [`crate::Sidebar`] of pages, a search that finds a setting on any page by its label, its
//! description or its keywords, and the page itself as [`crate::FormGroup`]s of
//! [`crate::FormField`]s. The application keeps the values; the window keeps nothing but which
//! page is shown when you do not.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, KeyDownEvent, Keystroke,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};

use crate::button::{Button, ControlSize};
use crate::controls::{TextField, held};
use crate::display::Tone;
use crate::focus::{own_handle, selector};
use crate::form::{FormField, FormGroup, LabelPlacement};
use crate::icon::{Icon, IconName, IconSource};
use crate::layout::{Edge, EmptyState, ListItem, SectionHeader, Sidebar};
use crate::strings::ActiveStrings;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

type IndexHandler = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

/// One setting: its label, what it does, words a search should also find it by, and its
/// control. It is drawn as a [`FormField`].
pub struct Setting {
    label: SharedString,
    description: Option<SharedString>,
    note: Option<SharedString>,
    error: Option<SharedString>,
    keywords: Vec<SharedString>,
    placement: LabelPlacement,
    controls: Vec<AnyElement>,
}

impl Setting {
    /// A setting labelled `label`, its label above the control.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            description: None,
            note: None,
            error: None,
            keywords: Vec::new(),
            placement: LabelPlacement::Above,
            controls: Vec::new(),
        }
    }

    /// One sentence on what it does; a search finds the setting by it too.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A caution that applies only to the current choice. See [`FormField::note`].
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// What is wrong with the value. See [`FormField::error`].
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Other words a search should find it by: synonyms, the other language's name for it.
    pub fn keywords<S: Into<SharedString>>(
        mut self,
        keywords: impl IntoIterator<Item = S>,
    ) -> Self {
        self.keywords.extend(keywords.into_iter().map(Into::into));
        self
    }

    /// The label at the leading edge and the control at the trailing edge, for a switch or a
    /// checkbox.
    pub fn beside(mut self) -> Self {
        self.placement = LabelPlacement::Beside;
        self
    }

    fn text(&self) -> impl Iterator<Item = &SharedString> {
        std::iter::once(&self.label)
            .chain(self.description.iter())
            .chain(self.keywords.iter())
    }

    fn into_field(self) -> FormField {
        let mut field = FormField::new(self.label).placement(self.placement);
        if let Some(description) = self.description {
            field = field.description(description);
        }
        if let Some(note) = self.note {
            field = field.note(note);
        }
        if let Some(error) = self.error {
            field = field.error(error);
        }
        field.children(self.controls)
    }
}

impl ParentElement for Setting {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.controls.extend(elements)
    }
}

/// A titled group of settings on a page.
pub struct SettingsGroup {
    title: Option<SharedString>,
    description: Option<SharedString>,
    settings: Vec<Setting>,
}

impl SettingsGroup {
    /// A group titled `title`.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: Some(title.into()),
            description: None,
            settings: Vec::new(),
        }
    }

    /// A group without a title, for a page that holds one group.
    pub fn untitled() -> Self {
        Self {
            title: None,
            description: None,
            settings: Vec::new(),
        }
    }

    /// A line under the title on what the group covers.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a setting.
    pub fn setting(mut self, setting: Setting) -> Self {
        self.settings.push(setting);
        self
    }

    fn into_group(self, settings: Vec<Setting>) -> FormGroup {
        let mut group = match self.title {
            Some(title) => FormGroup::new(title),
            None => FormGroup::untitled(),
        };
        if let Some(description) = self.description {
            group = group.description(description);
        }
        group.children(settings.into_iter().map(Setting::into_field))
    }
}

/// One page of the window: its name and icon in the sidebar, a line on what it covers, and its
/// groups. Anything that is not a list of settings (a key bindings list, a device test) goes in
/// with [`SettingsPage::child`]; a search does not look inside it.
pub struct SettingsPage {
    title: SharedString,
    icon: Option<IconSource>,
    description: Option<SharedString>,
    keywords: Vec<SharedString>,
    groups: Vec<SettingsGroup>,
    content: Vec<AnyElement>,
}

impl SettingsPage {
    /// A page named `title`.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            icon: None,
            description: None,
            keywords: Vec::new(),
            groups: Vec::new(),
            content: Vec::new(),
        }
    }

    /// The page's icon in the sidebar.
    pub fn icon(mut self, icon: impl Into<IconSource>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// One line on what the page covers, under its title and in the sidebar.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Words a search should find the page by, for content [`SettingsPage::child`] holds.
    pub fn keywords<S: Into<SharedString>>(
        mut self,
        keywords: impl IntoIterator<Item = S>,
    ) -> Self {
        self.keywords.extend(keywords.into_iter().map(Into::into));
        self
    }

    /// Add a group of settings.
    pub fn group(mut self, group: SettingsGroup) -> Self {
        self.groups.push(group);
        self
    }
}

impl ParentElement for SettingsPage {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.content.extend(elements)
    }
}

/// Whether every word of `query` is in one of `texts`, ignoring case.
pub(crate) fn matches<'a>(
    query: &str,
    texts: impl IntoIterator<Item = &'a SharedString> + Clone,
) -> bool {
    query.split_whitespace().all(|word| {
        let word = word.to_lowercase();
        texts
            .clone()
            .into_iter()
            .any(|text| text.to_lowercase().contains(&word))
    })
}

/// A preferences window's body: the pages in a sidebar beside the page that is open, and the
/// status line under both.
///
/// Put it under a [`crate::TitleBar`] in a [`crate::WindowFrame`]. Pass the search text with
/// [`PreferencesWindow::search`]: while it is not empty, the sidebar selects nothing and the
/// page shows every matching setting from every page, under the name of its page and group,
/// and a press on that name opens the page. Every word must match a setting's label,
/// description or keywords; a group's title or a page's title or keywords bring in all of it.
/// Clear the search when a page is picked.
///
/// ```ignore
/// PreferencesWindow::new("prefs")
///     .page(SettingsPage::new("General").icon(IconName::Settings)
///         .group(SettingsGroup::new("Project").setting(
///             Setting::new("Autosave").beside().keywords(["save"]).child(Switch::new(..)))))
///     .selected(self.page)
///     .on_select(cx.listener(|this, page: &usize, _, cx| { this.page = *page; this.query.clear(); }))
///     .search(self.query.clone(), self.search_field())
///     .action(Button::new("save", "Save").primary())
///     .status(StatusBar::new().message("Saved"))
/// ```
#[derive(IntoElement)]
pub struct PreferencesWindow {
    id: ElementId,
    pages: Vec<SettingsPage>,
    selected: usize,
    on_select: Option<IndexHandler>,
    query: SharedString,
    search_field: Option<AnyElement>,
    sidebar_footer: Vec<AnyElement>,
    actions: Vec<AnyElement>,
    status: Option<AnyElement>,
    sidebar_width: gpui::Pixels,
}

impl PreferencesWindow {
    /// An empty window body.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            pages: Vec::new(),
            selected: 0,
            on_select: None,
            query: SharedString::default(),
            search_field: None,
            sidebar_footer: Vec::new(),
            actions: Vec::new(),
            status: None,
            sidebar_width: px(240.),
        }
    }

    /// Add a page.
    pub fn page(mut self, page: SettingsPage) -> Self {
        self.pages.push(page);
        self
    }

    /// Which page is open. Without [`PreferencesWindow::on_select`] the window keeps the
    /// person's choice itself, starting here.
    pub fn selected(mut self, page: usize) -> Self {
        self.selected = page;
        self
    }

    /// Called with a page's index when the person opens it, from the sidebar or a search result.
    pub fn on_select(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// The search text and the field the person types it into. The field is the application's
    /// own text input, since typing needs one; without it the sidebar shows a field that only
    /// displays `query`.
    pub fn search(mut self, query: impl Into<SharedString>, field: impl IntoElement) -> Self {
        self.query = query.into();
        self.search_field = Some(field.into_any_element());
        self
    }

    /// The search text, shown in a field that cannot be typed into: for a window driven by
    /// something else, such as a test or a "show me where" link.
    pub fn query(mut self, query: impl Into<SharedString>) -> Self {
        self.query = query.into();
        self
    }

    /// Something at the bottom of the sidebar, such as a setup guide link.
    pub fn sidebar_footer(mut self, footer: impl IntoElement) -> Self {
        self.sidebar_footer.push(footer.into_any_element());
        self
    }

    /// A button at the end of the page. Add the quiet ones first and the primary last, so it
    /// sits at the trailing end. Hidden while searching.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// The line along the bottom: a [`crate::StatusBar`] saying what the last action did.
    pub fn status(mut self, status: impl IntoElement) -> Self {
        self.status = Some(status.into_any_element());
        self
    }

    /// The sidebar's width; 240 px by default.
    pub fn sidebar_width(mut self, width: gpui::Pixels) -> Self {
        self.sidebar_width = width;
        self
    }
}

impl RenderOnce for PreferencesWindow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let strings = cx.strings().clone();
        let (selected, set_selected) = held(&self.id, self.selected, window, cx);
        let on_select = self.on_select.clone();
        let select: IndexHandler = Rc::new(move |page: &usize, window, cx| {
            set_selected(page, window, cx);
            if let Some(handler) = &on_select {
                handler(page, window, cx);
            }
        });
        let query = self.query.trim().to_string();
        let searching = !query.is_empty();
        let selected = selected.min(self.pages.len().saturating_sub(1));
        let child_id = |name: String| ElementId::NamedChild(Box::new(self.id.clone()), name.into());

        let search = self.search_field.unwrap_or_else(|| {
            TextField::new()
                .id(child_id("search".into()))
                .icon(IconName::Search)
                .placeholder(strings.search_settings.clone())
                .value(self.query.clone())
                .size(ControlSize::Small)
                .into_any_element()
        });
        let nav = self.pages.iter().enumerate().map(|(index, page)| {
            let select = select.clone();
            let mut item = ListItem::new(child_id(format!("page-{index}")), page.title.clone())
                .selected(!searching && index == selected)
                .on_click(move |_, window, cx| select(&index, window, cx));
            if let Some(icon) = page.icon.clone() {
                item = item.icon(icon);
            }
            item
        });
        let sidebar = Sidebar::new(Edge::Left)
            .id(child_id("pages".into()))
            .overflow_y_scroll()
            .w(self.sidebar_width)
            .p(theme.spacing.sm)
            .gap(theme.spacing.xxs)
            .child(div().pb(theme.spacing.xs).child(search))
            .children(nav)
            .child(div().flex_1().min_h(theme.spacing.lg))
            .children(self.sidebar_footer);

        let mut body: Vec<AnyElement> = Vec::new();
        if searching {
            let mut found = 0;
            for (index, page) in self.pages.into_iter().enumerate() {
                // The page's name and keywords bring in all of it; its description is too broad
                // ("language, folders, start-up") to stand for every setting on it.
                let page_matches = matches(
                    &query,
                    std::iter::once(&page.title).chain(page.keywords.iter()),
                );
                let mut groups: Vec<AnyElement> = Vec::new();
                for group in page.groups {
                    let group_matches = page_matches
                        || group
                            .title
                            .as_ref()
                            .is_some_and(|title| matches(&query, [title]));
                    let mut settings = Vec::new();
                    let SettingsGroup {
                        title,
                        description,
                        settings: all,
                    } = group;
                    for setting in all {
                        if group_matches || matches(&query, setting.text().collect::<Vec<_>>()) {
                            settings.push(setting);
                        }
                    }
                    if settings.is_empty() {
                        continue;
                    }
                    found += settings.len();
                    let shell = SettingsGroup {
                        title,
                        description,
                        settings: Vec::new(),
                    };
                    groups.push(shell.into_group(settings).into_any_element());
                }
                if groups.is_empty() && !(page_matches && !page.content.is_empty()) {
                    continue;
                }
                let select = select.clone();
                let link_id = child_id(format!("result-{index}"));
                let link = selector(div().id(link_id.clone()), &link_id)
                    .flex()
                    .items_center()
                    .gap(theme.spacing.xs)
                    .cursor_pointer()
                    .text_color(theme.colors.text_muted)
                    .hover(|style| style.text_color(theme.colors.text))
                    .on_click(move |_, window, cx| select(&index, window, cx))
                    .children(page.icon.clone().map(|icon| Icon::new(icon).size(px(13.))))
                    .child(
                        Label::new(page.title.clone())
                            .size(TextSize::Small)
                            .weight(theme.typography.label_weight)
                            .color(TextColor::Muted),
                    )
                    .child(Icon::new(IconName::ChevronRight).size(px(11.)));
                if groups.is_empty() {
                    // The page itself matched, by content the search cannot look into.
                    found += 1;
                }
                body.push(
                    div()
                        .flex()
                        .flex_col()
                        .gap(theme.spacing.md)
                        .child(link)
                        .children(groups)
                        .into_any_element(),
                );
            }
            if found == 0 {
                body.push(
                    EmptyState::new(IconName::Search, strings.nothing_matches(&query))
                        .into_any_element(),
                );
            } else {
                body.insert(
                    0,
                    Label::new(strings.settings_found(found))
                        .size(TextSize::Small)
                        .color(TextColor::Muted)
                        .into_any_element(),
                );
            }
        } else if let Some(page) = self.pages.into_iter().nth(selected) {
            let mut header = SectionHeader::new(page.title.clone());
            if let Some(description) = page.description.clone() {
                header = header.description(description);
            }
            body.push(header.into_any_element());
            for group in page.groups {
                let SettingsGroup {
                    title,
                    description,
                    settings,
                } = group;
                body.push(
                    SettingsGroup {
                        title,
                        description,
                        settings: Vec::new(),
                    }
                    .into_group(settings)
                    .into_any_element(),
                );
            }
            body.extend(page.content);
            if !self.actions.is_empty() {
                let mut actions = self.actions;
                let primary = actions.pop();
                body.push(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(theme.spacing.sm)
                        .pt(theme.spacing.lg)
                        .children(actions)
                        .child(div().flex_1())
                        .children(primary)
                        .into_any_element(),
                );
            }
        }

        let page = div()
            .id(child_id("page".into()))
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .overflow_y_scroll()
            .bg(theme.colors.surface)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.xl)
                    .w_full()
                    .max_w(theme.density.form_width)
                    .px(theme.spacing.xxl)
                    .py(theme.spacing.xl)
                    .children(body),
            );
        selector(div().id(self.id.clone()), &self.id)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(div().flex().flex_1().min_h_0().child(sidebar).child(page))
            .children(self.status)
    }
}

/// One command in a [`KeybindingList`]: what it does, the keys that run it, and how its keys
/// stand.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyBinding {
    /// What the command does: "Play or stop".
    pub label: SharedString,
    /// Where the keys work, for a command that does not work everywhere: "Piano roll".
    pub scope: Option<SharedString>,
    /// Each key that runs it, as the platform prints it: "⌘S", "Ctrl+S". See
    /// [`keystroke_label`].
    pub keys: Vec<SharedString>,
    /// The person changed its keys, so it can be put back.
    pub changed: bool,
    /// Other commands one of its keys also runs.
    pub conflict: Option<SharedString>,
}

impl KeyBinding {
    /// A command labelled `label`, bound to `keys`.
    pub fn new<S: Into<SharedString>>(
        label: impl Into<SharedString>,
        keys: impl IntoIterator<Item = S>,
    ) -> Self {
        Self {
            label: label.into(),
            scope: None,
            keys: keys.into_iter().map(Into::into).collect(),
            changed: false,
            conflict: None,
        }
    }

    /// Where its keys work.
    pub fn scope(mut self, scope: impl Into<SharedString>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// Whether the person changed its keys.
    pub fn changed(mut self, changed: bool) -> Self {
        self.changed = changed;
        self
    }

    /// The other commands one of its keys also runs, named.
    pub fn conflict(mut self, others: impl Into<SharedString>) -> Self {
        self.conflict = Some(others.into());
        self
    }
}

/// A key press the [`KeybindingList`] heard while recording, or `None` when Escape cancelled
/// it.
pub type Captured = Option<Keystroke>;

type SlotHandler = Rc<dyn Fn(&(usize, usize), &mut Window, &mut App) + 'static>;
type CaptureHandler = Rc<dyn Fn(&Captured, &mut Window, &mut App) + 'static>;

/// Key bindings, grouped, with a way to record new keys.
///
/// Each command is a row: its label, where it works, then a chip per key. Pressing a chip
/// (or "None" for an unbound command, or + to add one) calls [`KeybindingList::on_record`]
/// with the row and the key's slot; set [`KeybindingList::recording`] to that, and the list
/// takes focus and reports the next key press through [`KeybindingList::on_capture`] (`None`
/// for Escape). Rows count from 0 across every group.
#[derive(IntoElement)]
pub struct KeybindingList {
    id: ElementId,
    groups: Vec<(Option<SharedString>, Vec<KeyBinding>)>,
    recording: Option<(usize, usize)>,
    on_record: Option<SlotHandler>,
    on_capture: Option<CaptureHandler>,
    on_remove: Option<SlotHandler>,
    on_reset: Option<IndexHandler>,
}

impl KeybindingList {
    /// An empty list.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            groups: Vec::new(),
            recording: None,
            on_record: None,
            on_capture: None,
            on_remove: None,
            on_reset: None,
        }
    }

    /// A titled group of commands.
    pub fn group(
        mut self,
        title: impl Into<SharedString>,
        bindings: impl IntoIterator<Item = KeyBinding>,
    ) -> Self {
        self.groups
            .push((Some(title.into()), bindings.into_iter().collect()));
        self
    }

    /// Commands without a group title.
    pub fn bindings(mut self, bindings: impl IntoIterator<Item = KeyBinding>) -> Self {
        self.groups.push((None, bindings.into_iter().collect()));
        self
    }

    /// The row and key slot waiting for a key press, if any. A slot past the row's last key
    /// adds a key.
    pub fn recording(mut self, recording: Option<(usize, usize)>) -> Self {
        self.recording = recording;
        self
    }

    /// Called with `(row, slot)` when the person asks to record a key there.
    pub fn on_record(
        mut self,
        handler: impl Fn(&(usize, usize), &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_record = Some(Rc::new(handler));
        self
    }

    /// Called with the key pressed while recording, or `None` for Escape.
    pub fn on_capture(
        mut self,
        handler: impl Fn(&Captured, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_capture = Some(Rc::new(handler));
        self
    }

    /// Called with `(row, slot)` when the person removes one of a command's keys. Offered
    /// only where a command has more than one.
    pub fn on_remove(
        mut self,
        handler: impl Fn(&(usize, usize), &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }

    /// Called with a row when the person puts a changed command's keys back.
    pub fn on_reset(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_reset = Some(Rc::new(handler));
        self
    }
}

/// A keystroke as the platform prints it in menus: "⌘⇧S" on macOS, "Ctrl+Shift+S" elsewhere.
pub fn keystroke_label(keystroke: &Keystroke) -> SharedString {
    let key = match keystroke.key.as_str() {
        "space" => "Space".to_string(),
        "enter" => "Enter".to_string(),
        "escape" => "Esc".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" => "Delete".to_string(),
        "tab" => "Tab".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" => "Page Up".to_string(),
        "pagedown" => "Page Down".to_string(),
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        }
    };
    let modifiers = &keystroke.modifiers;
    if cfg!(target_os = "macos") {
        let mut label = String::new();
        for (on, mark) in [
            (modifiers.control, "⌃"),
            (modifiers.alt, "⌥"),
            (modifiers.shift, "⇧"),
            (modifiers.platform, "⌘"),
        ] {
            if on {
                label.push_str(mark);
            }
        }
        label.push_str(&key);
        label.into()
    } else {
        let mut parts: Vec<&str> = Vec::new();
        for (on, name) in [
            (modifiers.control, "Ctrl"),
            (modifiers.alt, "Alt"),
            (modifiers.shift, "Shift"),
            (modifiers.platform, "Super"),
        ] {
            if on {
                parts.push(name);
            }
        }
        parts.push(&key);
        parts.join("+").into()
    }
}

/// Keys that only modify another key, which recording waits past.
fn modifier_only(key: &str) -> bool {
    matches!(
        key,
        "shift" | "control" | "alt" | "platform" | "function" | "capslock" | "fn"
    )
}

impl RenderOnce for KeybindingList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let strings = cx.strings().clone();
        let handle = own_handle(&self.id, window, cx);
        if self.recording.is_some() && !handle.is_focused(window) {
            // Recording reads the next key press, so the list must hold focus.
            let handle = handle.clone();
            window.defer(cx, move |window, _| window.focus(&handle));
        }
        let child_id = |name: String| ElementId::NamedChild(Box::new(self.id.clone()), name.into());
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut row = 0;
        for (title, bindings) in self.groups {
            if let Some(title) = title {
                rows.push(
                    div()
                        .pt(if row == 0 { px(0.) } else { theme.spacing.md })
                        .pb(theme.spacing.xxs)
                        .child(Label::caption(title))
                        .into_any_element(),
                );
            }
            for binding in bindings {
                let index = row;
                row += 1;
                let recording_slot = self
                    .recording
                    .filter(|(at, _)| *at == index)
                    .map(|(_, slot)| slot);
                let record = |slot: usize| {
                    let handler = self.on_record.clone();
                    move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                        if let Some(handler) = &handler {
                            handler(&(index, slot), window, cx);
                        }
                    }
                };
                let chip = |slot: usize, text: SharedString| {
                    let armed = recording_slot == Some(slot);
                    Button::new(
                        child_id(format!("key-{index}-{slot}")),
                        if armed {
                            strings.press_a_key.clone()
                        } else {
                            text
                        },
                    )
                    .size(ControlSize::Small)
                    .selected(armed)
                    .on_click(record(slot))
                };
                let several = binding.keys.len() > 1;
                let count = binding.keys.len();
                let chips: Vec<AnyElement> = binding
                    .keys
                    .iter()
                    .enumerate()
                    .map(|(slot, key)| {
                        let remove = self.on_remove.clone();
                        div()
                            .flex()
                            .items_center()
                            .child(chip(slot, key.clone()))
                            .when(several, |chip_row| {
                                chip_row.child(
                                    Button::icon_only(
                                        child_id(format!("remove-{index}-{slot}")),
                                        IconName::Close,
                                    )
                                    .ghost()
                                    .size(ControlSize::Small)
                                    .tooltip(strings.remove(key))
                                    .on_click(
                                        move |_, window, cx| {
                                            if let Some(remove) = &remove {
                                                remove(&(index, slot), window, cx);
                                            }
                                        },
                                    ),
                                )
                            })
                            .into_any_element()
                    })
                    .collect();
                let adding = recording_slot.is_some_and(|slot| slot >= count);
                let unbound = (count == 0 && !adding).then(|| chip(0, strings.no_shortcut.clone()));
                let reset = self.on_reset.clone();
                rows.push(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(theme.spacing.sm)
                        .min_h(theme.density.row)
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(120.))
                                .truncate()
                                .text_size(theme.typography.size_sm)
                                .child(binding.label.clone()),
                        )
                        .children(binding.scope.map(|scope| {
                            Label::new(scope)
                                .size(TextSize::XSmall)
                                .color(TextColor::Faint)
                        }))
                        .children(binding.conflict.map(|others| {
                            div()
                                .flex()
                                .items_center()
                                .gap(theme.spacing.xxs)
                                .text_size(theme.typography.size_xs)
                                .text_color(Tone::Warning.colors(&theme).0)
                                .child(Icon::new(IconName::Alert).size(px(12.)))
                                .child(strings.also_runs(&others))
                        }))
                        .children(chips)
                        .children(unbound)
                        .children(adding.then(|| chip(count, SharedString::default())))
                        .child(
                            Button::icon_only(child_id(format!("add-{index}")), IconName::Plus)
                                .ghost()
                                .size(ControlSize::Small)
                                .tooltip(strings.add_shortcut.clone())
                                .on_click(record(count)),
                        )
                        .child(
                            div()
                                .w(theme.density.control_sm)
                                .children(binding.changed.then(|| {
                                    Button::icon_only(
                                        child_id(format!("reset-{index}")),
                                        IconName::Undo,
                                    )
                                    .ghost()
                                    .size(ControlSize::Small)
                                    .tooltip(strings.reset_shortcut.clone())
                                    .on_click(
                                        move |_, window, cx| {
                                            if let Some(reset) = &reset {
                                                reset(&index, window, cx);
                                            }
                                        },
                                    )
                                })),
                        )
                        .into_any_element(),
                );
            }
        }
        let capture = self.on_capture.clone();
        let recording = self.recording.is_some();
        selector(div().id(self.id.clone()), &self.id)
            .track_focus(&handle)
            .flex()
            .flex_col()
            .gap(theme.spacing.xxs)
            .when(recording, |list| {
                list.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let key = event.keystroke.key.as_str();
                    if modifier_only(key) {
                        return;
                    }
                    cx.stop_propagation();
                    let captured = (key != "escape").then(|| event.keystroke.clone());
                    if let Some(capture) = &capture {
                        capture(&captured, window, cx);
                    }
                })
            })
            .children(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_must_match_somewhere() {
        let texts: Vec<SharedString> = vec!["Autosave".into(), "Saves every minute".into()];
        assert!(matches("auto", &texts));
        assert!(matches("SAVE minute", &texts));
        assert!(!matches("save hour", &texts));
        let japanese: Vec<SharedString> = vec!["自動保存".into()];
        assert!(matches("保存", &japanese));
    }

    #[test]
    fn keystrokes_print_as_menus_do() {
        let save = Keystroke::parse("ctrl-shift-s").unwrap();
        let label = keystroke_label(&save);
        if cfg!(target_os = "macos") {
            assert_eq!(label.as_ref(), "⌃⇧S");
        } else {
            assert_eq!(label.as_ref(), "Ctrl+Shift+S");
        }
        assert_eq!(
            keystroke_label(&Keystroke::parse("space").unwrap()).as_ref(),
            "Space"
        );
    }
}
