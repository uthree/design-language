//! Atelier: a design language for gpui creator tools with AI agents built in.
//!
//! Every component reads its colours, sizes and fonts from the installed [`Theme`]. A theme is
//! derived from a [`Direction`] — one of the design candidates — and an [`Appearance`]:
//!
//! ```ignore
//! use atelier::prelude::*;
//!
//! Application::new().with_assets(Assets::default()).run(|cx| {
//!     atelier::init(cx, Theme::new(Direction::Glass, Appearance::Dark));
//!     // open windows; read the theme with `cx.theme()`
//! });
//! ```
//!
//! [`prelude`] brings in the theme, the extension traits and the components most windows use;
//! everything else is at the crate root or in its module ([`agent`], [`viz`], [`plot`],
//! [`ambient`], [`metal`], [`emoji`], [`motion`]). Atelier re-exports the [`gpui`] it is built
//! against, so `atelier::gpui` always matches it.
//!
//! # Features
//!
//! - `gpu` (default): shade the liquid metal, the ambient backdrops and the 3-D plots on the
//!   crate's own wgpu device. Without it they draw on the CPU, as on a machine with no GPU, and
//!   wgpu is not built.
//! - `runtime-shaders`: turn on gpui's `runtime_shaders`, so an application builds on a Mac with
//!   only the Command Line Tools installed.

/// The `atelier-agents` crate Atelier was built with, which runs the coding-agent CLIs that
/// [`agent::AgentConnection`] shows. Needs the `agents` feature.
#[cfg(feature = "agents")]
pub use atelier_agents as agents;
pub use gpui;

pub mod agent;
pub mod ambient;
mod backdrop;
mod button;
mod controls;
mod dialog;
mod display;
pub mod emoji;
mod focus;
mod follow;
mod form;
mod frame;
mod gpu;
pub mod graph;
mod icon;
mod inline_edit;
mod input;
mod keyboard;
mod layout;
mod markdown;
mod media;
pub mod metal;
pub mod motion;
mod overlay;
pub mod plot;
mod preferences;
mod progress;
mod selectable;
mod stat;
mod strings;
mod table;
mod text;
mod theme;
mod transport;
pub mod viz;
pub mod web;
mod window;

pub use ambient::{Ambience, AmbientBackdrop};
pub use backdrop::BackdropPalette;
pub use button::{Button, ButtonVariant, ControlSize, CopyButton};
pub use controls::{Checkbox, Knob, Segment, SegmentedControl, Slider, Switch, TextField};
pub use dialog::{DIALOG_CONTEXT, Dialog};
pub use display::{
    Avatar, AvatarKind, Badge, Divider, Kbd, LevelMeter, METER_FLOOR_DB, Spinner, StatusDot, Tone,
};
pub use focus::{
    CONTROL_CONTEXT, FocusNext, FocusPrevious, LIST_CONTEXT, SelectFirst, SelectLast, SelectNext,
    SelectPrevious, StepBackward, StepForward, StepToEnd, StepToStart, TRACK_CONTEXT,
    focus_visible, set_focus_visible, tab_navigation,
};
pub use follow::{BackToPlayhead, Follow, PlayheadSide};
pub use form::{FormField, FormGroup, LabelPlacement, StatusBar};
pub use frame::{TitleBar, WindowFrame, custom_frame};
pub use icon::{Assets, Icon, IconName, IconSource, ProviderLogo};
pub use inline_edit::{InlineEdit, SpotBook};
pub use input::{
    CancelText, SubmitText, TEXT_INPUT_CONTEXT, TextBuffer, TextInput, TextInputEvent,
};
pub use keyboard::{PianoKeyboard, note_name};
pub use layout::{
    Card, Disclosure, Edge, EmptyState, ListItem, OptionCard, SectionHeader, Sidebar, Tabs, Toolbar,
};
pub use markdown::Markdown;
pub use media::AudioPlayer;
pub use metal::MetalBackdrop;
pub use motion::{Measure, Morph, Motion};
pub use overlay::{Completion, Completions, MenuItem, Popover, Tooltip, menu_separator};
pub use preferences::{
    Captured, KeyBinding, KeybindingList, PreferencesWindow, Setting, SettingsGroup, SettingsPage,
    keystroke_label,
};
pub use progress::{
    ProgressBar, ProgressRing, Skeleton, StepList, StepState, TaskProgress, TaskState,
    skeleton_paragraph,
};
pub use selectable::{CopySelection, LinkHandler, SELECTABLE_CONTEXT, SelectAll, SelectableText};
pub use stat::Stat;
pub use strings::{ActiveStrings, Strings};
pub use table::{Column, ColumnWidth, RowMark, Table, TableCell, TableRow};
pub use text::{Label, TextColor, TextSize};
pub use theme::{
    ActiveTheme, Appearance, Colors, Density, Direction, Elevation, Material, PrimaryStyle, Shape,
    Spacing, Theme, Typography, contrast, ensure_contrast, lighten, mix, mono_font, ui_font,
};
pub use transport::{Readout, ReadoutStrip};
pub use window::set_window_background;

/// Install `theme`, bind Tab and Shift-Tab to move keyboard focus, and keep Enter and Space
/// for the focused control. Call it at start-up, before opening windows. Calling it again only
/// installs the theme, but to switch themes later call [`Theme::install`].
pub fn init(cx: &mut gpui::App, theme: Theme) {
    focus::init(cx);
    theme.install(cx);
}

/// What most windows built with Atelier use: the theme and its tokens, the extension traits that
/// read the theme and the words (`cx.theme()`, `cx.strings()`), and the everyday components.
/// Specialised parts stay in their modules: [`agent`], [`viz`], [`plot`], [`ambient`],
/// [`metal`], [`emoji`] and [`motion`].
///
/// ```ignore
/// use atelier::prelude::*;
/// ```
pub mod prelude {
    pub use crate::{
        ActiveStrings, ActiveTheme, Appearance, Assets, Avatar, Badge, Button, ButtonVariant, Card,
        Checkbox, ControlSize, Dialog, Direction, Divider, EmptyState, FormField, FormGroup, Icon,
        IconName, Label, ListItem, MenuItem, Popover, SectionHeader, Sidebar, Slider, Spinner,
        StatusBar, Strings, Switch, Tabs, TextColor, TextField, TextInput, TextInputEvent,
        TextSize, Theme, TitleBar, Tone, Toolbar, Tooltip, WindowFrame,
    };
}
