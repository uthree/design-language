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
mod media;
pub mod metal;
pub mod motion;
mod overlay;
pub mod plot;
mod progress;
mod text;
mod theme;
pub mod viz;
mod window;

pub use button::{Button, ButtonVariant, ControlSize};
pub use controls::{Checkbox, Knob, Segment, SegmentedControl, Slider, Switch, TextField};
pub use display::{Avatar, AvatarKind, Badge, Divider, Kbd, LevelMeter, Spinner, StatusDot, Tone};
pub use frame::{TitleBar, WindowFrame, custom_frame};
pub use icon::{Assets, Icon, IconName};
pub use layout::{
    Card, Disclosure, Edge, EmptyState, ListItem, OptionCard, SectionHeader, Sidebar, Tabs, Toolbar,
};
pub use media::AudioPlayer;
pub use metal::{LiquidMetal, MetalBackdrop, MetalFrame};
pub use motion::Motion;
pub use overlay::{MenuItem, Popover, Tooltip, menu_separator};
pub use progress::{
    ProgressBar, ProgressRing, Skeleton, StepList, StepState, TaskProgress, TaskState,
    skeleton_paragraph,
};
pub use text::{Label, TextColor, TextSize};
pub use theme::{
    ActiveTheme, Appearance, Colors, Density, Direction, Elevation, Material, PrimaryStyle, Shape,
    Spacing, Theme, Typography, contrast, ensure_contrast, lighten, mix, mono_font, ui_font,
};
pub use window::set_window_background;

/// Install `theme`. Call once at start-up, before opening windows.
pub fn init(cx: &mut gpui::App, theme: Theme) {
    theme.install(cx);
}
