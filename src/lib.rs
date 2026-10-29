//! Atelier: a design language for gpui creator tools with AI agents built in.
//!
//! Every component reads its colours, sizes and fonts from the installed [`Theme`]. A theme is
//! derived from a [`Direction`] — one of the design candidates — and an [`Appearance`]:
//!
//! ```ignore
//! Application::new().with_assets(atelier::Assets::default()).run(|cx| {
//!     atelier::init(cx, Theme::new(Direction::Studio, Appearance::Dark));
//!     // open windows; read the theme with `cx.theme()`
//! });
//! ```

pub mod agent;
pub mod ambient;
mod backdrop;
mod button;
mod controls;
mod display;
pub mod emoji;
mod focus;
mod form;
mod frame;
mod gpu;
mod icon;
mod layout;
mod media;
pub mod metal;
pub mod motion;
mod overlay;
pub mod plot;
mod progress;
mod selectable;
mod text;
mod theme;
pub mod viz;
mod window;

pub use ambient::{Ambience, AmbientBackdrop};
pub use backdrop::BackdropPalette;
pub use button::{Button, ButtonVariant, ControlSize, CopyButton};
pub use controls::{Checkbox, Knob, Segment, SegmentedControl, Slider, Switch, TextField};
pub use display::{Avatar, AvatarKind, Badge, Divider, Kbd, LevelMeter, Spinner, StatusDot, Tone};
pub use focus::{
    CONTROL_CONTEXT, FocusNext, FocusPrevious, StepBackward, StepForward, StepToEnd, StepToStart,
    TRACK_CONTEXT, focus_visible, set_focus_visible, tab_navigation,
};
pub use form::{FormField, FormGroup, LabelPlacement, StatusBar};
pub use frame::{TitleBar, WindowFrame, custom_frame};
pub use icon::{Assets, Icon, IconName, IconSource};
pub use layout::{
    Card, Disclosure, Edge, EmptyState, ListItem, OptionCard, SectionHeader, Sidebar, Tabs, Toolbar,
};
pub use media::AudioPlayer;
pub use metal::MetalBackdrop;
pub use motion::{Measure, Morph, Motion};
pub use overlay::{Completion, Completions, MenuItem, Popover, Tooltip, menu_separator};
pub use progress::{
    ProgressBar, ProgressRing, Skeleton, StepList, StepState, TaskProgress, TaskState,
    skeleton_paragraph,
};
pub use selectable::{CopySelection, SELECTABLE_CONTEXT, SelectAll, SelectableText};
pub use text::{Label, TextColor, TextSize};
pub use theme::{
    ActiveTheme, Appearance, Colors, Density, Direction, Elevation, Material, PrimaryStyle, Shape,
    Spacing, Theme, Typography, contrast, ensure_contrast, lighten, mix, mono_font, ui_font,
};
pub use window::set_window_background;

/// Install `theme`, bind Tab and Shift-Tab to move keyboard focus, and keep Enter and Space
/// for the focused control. Call it at start-up, before opening windows. Calling it again only
/// installs the theme, but to switch themes later call [`Theme::install`].
pub fn init(cx: &mut gpui::App, theme: Theme) {
    focus::init(cx);
    theme.install(cx);
}
