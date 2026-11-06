//! The preferences window finds settings across pages and opens a result's page; the key
//! bindings list records the next key press.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::{
    Captured, KeyBinding, KeybindingList, PreferencesWindow, Setting, SettingsGroup, SettingsPage,
    Switch, Theme, WindowFrame,
};
use gpui::{
    Context, IntoElement, Modifiers, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, Window,
};

#[derive(Default)]
struct Log {
    pages: Vec<usize>,
    records: Vec<(usize, usize)>,
    captured: Vec<Option<String>>,
}

struct Host {
    query: &'static str,
    recording: Option<(usize, usize)>,
    log: Rc<RefCell<Log>>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pages = self.log.clone();
        let log = self.log.clone();
        let list = KeybindingList::new("keys")
            .group(
                "Transport",
                [
                    KeyBinding::new("Play", ["Space"]),
                    KeyBinding::new("Record", ["R"]),
                ],
            )
            .recording(self.recording)
            .on_record(cx.listener(|this, slot: &(usize, usize), _, cx| {
                this.log.borrow_mut().records.push(*slot);
                this.recording = Some(*slot);
                cx.notify();
            }))
            .on_capture(move |captured: &Captured, _, _| {
                log.borrow_mut()
                    .captured
                    .push(captured.as_ref().map(|keystroke| keystroke.unparse()))
            });
        WindowFrame::new().size_full().child(
            PreferencesWindow::new("prefs")
                .page(
                    SettingsPage::new("General").group(
                        SettingsGroup::new("Project")
                            .setting(
                                Setting::new("Autosave")
                                    .beside()
                                    .child(Switch::new("a", true)),
                            )
                            .setting(Setting::new("Language").keywords(["locale"])),
                    ),
                )
                .page(
                    SettingsPage::new("Audio").group(
                        SettingsGroup::new("Devices").setting(
                            Setting::new("Output")
                                .description("Where playback goes, saved per project."),
                        ),
                    ),
                )
                .page(SettingsPage::new("Keys").child(list))
                .selected(2)
                .query(self.query)
                .on_select(move |page, _, _| pages.borrow_mut().pages.push(*page)),
        )
    }
}

fn open<'a>(
    query: &'static str,
    cx: &'a mut TestAppContext,
) -> (Rc<RefCell<Log>>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        atelier::init(cx, Theme::default());
        atelier::motion::reduce_motion(cx);
    });
    let log = Rc::new(RefCell::new(Log::default()));
    let (_, window) = cx.add_window_view({
        let log = log.clone();
        move |_, _| Host {
            query,
            recording: None,
            log,
        }
    });
    window.run_until_parked();
    (log, window)
}

fn click(selector: &'static str, cx: &mut VisualTestContext) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("no {selector}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
}

#[gpui::test]
fn a_search_finds_settings_on_every_page(cx: &mut TestAppContext) {
    let (log, cx) = open("save", cx);
    // "save" is in Autosave (General) and in Output's description (Audio), not in Keys.
    assert!(cx.debug_bounds("prefs-result-0").is_some());
    assert!(cx.debug_bounds("prefs-result-1").is_some());
    assert!(cx.debug_bounds("prefs-result-2").is_none());
    click("prefs-result-1", cx);
    assert_eq!(log.borrow().pages, [1]);
}

#[gpui::test]
fn keywords_find_a_setting(cx: &mut TestAppContext) {
    let (_, cx) = open("locale", cx);
    assert!(cx.debug_bounds("prefs-result-0").is_some());
    assert!(cx.debug_bounds("prefs-result-1").is_none());
}

#[gpui::test]
fn the_page_list_opens_a_page(cx: &mut TestAppContext) {
    let (log, cx) = open("", cx);
    click("prefs-page-0", cx);
    assert_eq!(log.borrow().pages, [0]);
}

#[gpui::test]
fn recording_takes_the_next_key(cx: &mut TestAppContext) {
    let (log, cx) = open("", cx);
    click("keys-key-1-0", cx);
    assert_eq!(log.borrow().records, [(1, 0)]);
    // Modifiers alone wait; the chord is what is reported, and Escape cancels.
    cx.simulate_keystrokes("shift ctrl-k");
    cx.simulate_keystrokes("escape");
    assert_eq!(log.borrow().captured, [Some("ctrl-k".to_string()), None]);
}
