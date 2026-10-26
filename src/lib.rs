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
mod button;
mod controls;
mod display;
mod frame;
mod icon;
mod layout;
mod overlay;
mod text;
mod theme;

pub use button::{Button, ButtonVariant, ControlSize};
pub use controls::{Checkbox, Knob, Segment, SegmentedControl, Slider, Switch, TextField};
pub use display::{
    Avatar, AvatarKind, Badge, Divider, Kbd, LevelMeter, ProgressBar, Spinner, StatusDot, Tone,
};
pub use frame::{TitleBar, custom_frame};
pub use icon::{Assets, Icon, IconName};
pub use layout::{
    Card, Disclosure, Edge, EmptyState, ListItem, SectionHeader, Sidebar, Tabs, Toolbar,
};
pub use overlay::{MenuItem, Popover, Tooltip, menu_separator};
pub use text::{Label, TextColor, TextSize};
pub use theme::{
    ActiveTheme, Appearance, Colors, Density, Direction, Elevation, Material, PrimaryStyle, Shape,
    Spacing, Theme, Typography, contrast, ensure_contrast, lighten, mix, mono_font, ui_font,
};

/// Install `theme`. Call once at start-up, before opening windows.
pub fn init(cx: &mut gpui::App, theme: Theme) {
    theme.install(cx);
}
