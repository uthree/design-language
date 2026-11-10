//! Following a playhead through a view that scrolls, and letting go of it when the person
//! scrolls away.
//!
//! A timeline, a piano roll or an event list that plays pages along so the playhead stays in
//! view. A person who scrolls away to look at something else while it plays should not be
//! dragged back on the next page turn, so [`Follow`] notices the scroll and stops; a
//! [`BackToPlayhead`] button in the corner on the side the playhead went then takes them back.

use gpui::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, Styled, Window, div,
    prelude::FluentBuilder,
};

use crate::button::{Button, ControlSize};
use crate::icon::IconName;
use crate::strings::ActiveStrings;
use crate::theme::ActiveTheme;

/// How far into the view, as a share of what it shows, a page turn puts the playhead, so the
/// note it is about to reach is already in view.
const LEAD: f64 = 0.05;

/// Which side of the view the playhead is on while it is out of sight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayheadSide {
    /// Earlier than the view shows: left on a timeline, above in a list.
    Before,
    /// Later than the view shows.
    After,
}

/// Whether a view follows its playhead, and the page turns that keep it in view.
///
/// Call [`Follow::update`] each frame with where the view is, how much it shows, where the
/// playhead is (all in one unit: beats, seconds, rows) and whether it plays. It answers with an
/// offset to scroll to when the playhead is about to leave the view. A change of offset that
/// keeps the span and leaves the playhead out of view is the person scrolling away, and
/// following stops until the playhead comes back into view, playback starts again, or
/// [`Follow::engage`] is called. A changed span is a zoom and does not let go.
#[derive(Clone, Debug, Default)]
pub struct Follow {
    released: bool,
    last: Option<(f64, f64)>,
    was_playing: bool,
}

impl Follow {
    /// A view that follows.
    pub fn new() -> Self {
        Self::default()
    }

    /// Follow again: what the back-to-playhead button does, along with scrolling to
    /// [`Follow::page`].
    pub fn engage(&mut self) {
        self.released = false;
    }

    /// Stop following, as when the person picks something to look at.
    pub fn release(&mut self) {
        self.released = true;
    }

    /// Whether the person has scrolled away from the playhead.
    pub fn is_released(&self) -> bool {
        self.released
    }

    /// Where a page turn puts the view so that `at` is just inside its start.
    pub fn page(span: f64, at: f64) -> f64 {
        (at - span * LEAD).max(0.0)
    }

    /// Take in this frame's view; returns the offset to scroll to, if a page turn is due.
    pub fn update(&mut self, offset: f64, span: f64, at: f64, playing: bool) -> Option<f64> {
        if playing && !self.was_playing {
            self.released = false;
        }
        self.was_playing = playing;
        if !playing {
            self.last = None;
            return None;
        }
        let last = self.last.replace((offset, span));
        let visible = at >= offset && at < offset + span;
        if visible {
            self.released = false;
            return None;
        }
        if let Some((last_offset, last_span)) = last
            && last_offset != offset
            && last_span == span
        {
            self.released = true;
        }
        if self.released {
            return None;
        }
        // The view may clamp the page turn at its end, so the offset it lands on is taken as
        // it comes next frame rather than counted as a scroll.
        self.last = None;
        Some(Self::page(span, at))
    }

    /// The side the playhead is on, while the person has scrolled away from it.
    pub fn side(&self, offset: f64, span: f64, at: f64) -> Option<PlayheadSide> {
        if !self.released {
            None
        } else if at < offset {
            Some(PlayheadSide::Before)
        } else if at >= offset + span {
            Some(PlayheadSide::After)
        } else {
            None
        }
    }
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The button that takes a released view back to its playhead.
///
/// It floats in the bottom corner on the side the playhead went, with the shadow a popover
/// casts, and points that way. Put it in the view's `relative()` container, shown while
/// [`Follow::side`] answers.
#[derive(IntoElement)]
pub struct BackToPlayhead {
    id: ElementId,
    side: PlayheadSide,
    label: Option<gpui::SharedString>,
    on_click: Option<ClickHandler>,
}

impl BackToPlayhead {
    /// The button for a playhead on `side` of the view.
    pub fn new(id: impl Into<ElementId>, side: PlayheadSide) -> Self {
        Self {
            id: id.into(),
            side,
            label: None,
            on_click: None,
        }
    }

    /// Other words than [`crate::Strings::back_to_playhead`].
    pub fn label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Called on a click, Enter or Space: scroll to [`Follow::page`] and [`Follow::engage`].
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for BackToPlayhead {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let label = self
            .label
            .unwrap_or_else(|| cx.strings().back_to_playhead.clone());
        let mut button = Button::new(self.id, label).size(ControlSize::Small);
        button = match self.side {
            PlayheadSide::Before => button.icon(IconName::ChevronLeft),
            PlayheadSide::After => button.icon_after(IconName::ChevronRight),
        };
        if let Some(handler) = self.on_click {
            button = button.on_click(handler);
        }
        let inset = theme.spacing.md;
        div()
            .absolute()
            .bottom(inset)
            .map(|place| match self.side {
                PlayheadSide::Before => place.left(inset),
                PlayheadSide::After => place.right(inset),
            })
            .rounded(theme.shape.radius_md)
            .shadow(theme.elevation.medium.clone())
            .child(button)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_along_while_playing() {
        let mut follow = Follow::new();
        assert_eq!(follow.update(0.0, 10.0, 5.0, true), None);
        let to = follow.update(0.0, 10.0, 10.0, true).unwrap();
        assert!(to < 10.0 && to > 9.0);
        // The view took the page turn; the playhead is in view again.
        assert_eq!(follow.update(to, 10.0, 10.2, true), None);
        assert!(!follow.is_released());
    }

    #[test]
    fn a_scroll_away_lets_go_until_the_playhead_is_back() {
        let mut follow = Follow::new();
        follow.update(0.0, 10.0, 5.0, true);
        // The person scrolls on to 40..50 while the playhead is at 5.
        assert_eq!(follow.update(40.0, 10.0, 5.1, true), None);
        assert!(follow.is_released());
        assert_eq!(follow.side(40.0, 10.0, 5.1), Some(PlayheadSide::Before));
        // It stays where they put it.
        assert_eq!(follow.update(40.0, 10.0, 9.0, true), None);
        // The playhead walks into view, and following picks up again.
        assert_eq!(follow.update(40.0, 10.0, 41.0, true), None);
        assert!(!follow.is_released());
        assert!(follow.update(40.0, 10.0, 50.0, true).is_some());
    }

    #[test]
    fn a_zoom_does_not_let_go() {
        let mut follow = Follow::new();
        follow.update(0.0, 10.0, 8.0, true);
        // Zooming in to 2..6 hides the playhead but keeps following it.
        assert!(follow.update(2.0, 4.0, 8.1, true).is_some());
        assert!(!follow.is_released());
    }

    #[test]
    fn playing_again_engages() {
        let mut follow = Follow::new();
        follow.update(0.0, 10.0, 5.0, true);
        follow.update(30.0, 10.0, 5.0, true);
        assert!(follow.is_released());
        // Scrolling while stopped is never a release, and play starts following again.
        assert_eq!(follow.update(30.0, 10.0, 5.0, false), None);
        assert_eq!(follow.update(60.0, 10.0, 5.0, false), None);
        assert!(follow.update(60.0, 10.0, 5.0, true).is_some());
        assert!(!follow.is_released());
        assert_eq!(follow.side(60.0, 10.0, 5.0), None);
    }
}
