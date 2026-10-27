//! Design tokens: every colour, radius, size and font a component draws with.
//!
//! A [`Theme`] is never written out field by field. It is derived from a [`Direction`] (the
//! design candidate: shape, density, material and palette seeds) and an [`Appearance`], so the
//! rules that make a palette coherent — raised surfaces sit nearer the eye, muted text still
//! clears 4.5:1, an accent stays readable on every surface — hold for every candidate at once.
//!
//! Components read the theme through [`ActiveTheme::theme`]; nothing outside this module should
//! name a hex literal.

use gpui::{
    App, BoxShadow, Font, FontFallbacks, FontWeight, Global, Hsla, Pixels, Rgba, SharedString,
    WindowBackgroundAppearance, hsla, point, px, rgb,
};

/// Whether the interface is drawn light on dark or dark on light.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Appearance {
    /// Dark text on light surfaces.
    Light,
    /// Light text on dark surfaces; the default for media and creative tools.
    #[default]
    Dark,
}

impl Appearance {
    /// Both appearances, in the order a picker offers them.
    pub const ALL: [Self; 2] = [Self::Light, Self::Dark];

    /// Whether this is [`Appearance::Dark`].
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    /// Name shown in pickers.
    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

/// The design candidates. Each is a complete direction, not just a palette: it fixes the shape
/// language, the density, how chrome is layered and how primary actions are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Pro-app density after Logic Pro and Final Cut Pro: graphite chrome, small radii, compact
    /// controls, bezel highlights and a system-blue accent. Built for long sessions in front of
    /// a timeline.
    Studio,
    /// After Apple's Liquid Glass: translucent, blurred chrome floating over content, capsule
    /// controls and generous radii. The adopted direction, and the default.
    #[default]
    Glass,
    /// After zeron: near-black monochrome, hairline alpha borders, solid white primary buttons
    /// and a single violet accent kept for the AI and selection. The most editorial of the three.
    Mono,
}

impl Direction {
    /// Every candidate, in the order a picker offers them.
    pub const ALL: [Self; 3] = [Self::Studio, Self::Glass, Self::Mono];

    /// Stable identifier for preference files.
    pub fn id(self) -> &'static str {
        match self {
            Self::Studio => "studio",
            Self::Glass => "glass",
            Self::Mono => "mono",
        }
    }

    /// Name shown in pickers.
    pub fn label(self) -> &'static str {
        match self {
            Self::Studio => "Studio",
            Self::Glass => "Glass",
            Self::Mono => "Mono",
        }
    }

    /// Parse an identifier written by [`Direction::id`].
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|direction| direction.id() == id)
    }
}

/// How chrome surfaces (sidebars, toolbars, panels) are filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    /// Solid fills; the window draws its own opaque background.
    Opaque,
    /// Translucent fills over a blurred window. Content areas stay opaque.
    Translucent,
}

/// How the one primary action in a group is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimaryStyle {
    /// Filled with the accent colour.
    Accent,
    /// Filled with the foreground colour (white on dark, near-black on light).
    Solid,
}

/// Semantic colours. Surfaces are listed from the back of the window to the front.
#[derive(Clone, Debug, PartialEq)]
pub struct Colors {
    /// Behind everything; also the fill of content canvases such as a timeline.
    pub background: Hsla,
    /// Sidebars, title bar and toolbars. Translucent under [`Material::Translucent`].
    pub chrome: Hsla,
    /// Standard panel surface.
    pub surface: Hsla,
    /// Cards, list groups and message bubbles drawn on a panel.
    pub elevated: Hsla,
    /// Menus, popovers, tooltips and dialogs.
    pub overlay: Hsla,
    /// Recessed wells: text fields, meters, slider tracks, lanes.
    pub sunken: Hsla,
    /// Translucent wash laid over a control under the pointer.
    pub hover: Hsla,
    /// Translucent wash laid over a control while pressed.
    pub pressed: Hsla,
    /// Background of a selected row or tab.
    pub selected: Hsla,
    /// Hairline border around controls and panels.
    pub border: Hsla,
    /// Border for emphasis and for controls that must read as editable.
    pub border_strong: Hsla,
    /// Divider between sections inside a panel.
    pub separator: Hsla,
    /// One-pixel highlight on the top edge of raised controls (the "bezel").
    pub highlight: Hsla,
    /// Keyboard focus ring.
    pub focus_ring: Hsla,

    /// Primary text.
    pub text: Hsla,
    /// Secondary text, at least 4.5:1 on every surface.
    pub text_muted: Hsla,
    /// Captions and placeholders that should recede.
    pub text_faint: Hsla,
    /// Text of a disabled control.
    pub text_disabled: Hsla,

    /// The interactive accent.
    pub accent: Hsla,
    /// Accent under the pointer.
    pub accent_hover: Hsla,
    /// Accent-coloured text, at least 4.5:1 on the standard surface.
    pub accent_text: Hsla,
    /// Low-alpha accent for large selected areas.
    pub accent_wash: Hsla,
    /// Text drawn on an accent fill.
    pub on_accent: Hsla,
    /// Foreground-coloured fill for [`PrimaryStyle::Solid`] buttons.
    pub solid: Hsla,
    /// Text on [`Colors::solid`].
    pub on_solid: Hsla,

    /// Something succeeded or is live.
    pub success: Hsla,
    /// Something deserves attention, but nothing failed.
    pub warning: Hsla,
    /// Something failed, or will destroy work.
    pub danger: Hsla,
    /// Neutral information.
    pub info: Hsla,
    /// Low-alpha washes behind the four signal colours.
    pub success_wash: Hsla,
    /// See [`Colors::success_wash`].
    pub warning_wash: Hsla,
    /// See [`Colors::success_wash`].
    pub danger_wash: Hsla,
    /// See [`Colors::success_wash`].
    pub info_wash: Hsla,

    /// The AI's identity colour: agent avatar, running indicators, AI-authored highlights.
    pub agent: Hsla,
    /// Low-alpha agent colour behind AI-authored content.
    pub agent_wash: Hsla,
    /// Three stops of the agent gradient, used for the "AI is working" edge glow.
    pub agent_gradient: [Hsla; 3],
    /// Background of the person's own chat messages.
    pub user_bubble: Hsla,
    /// Text on [`Colors::user_bubble`].
    pub on_user_bubble: Hsla,

    /// Transport and recording colours for creator tools.
    pub record: Hsla,
    /// Playhead and "playing" indicators.
    pub playhead: Hsla,
    /// Meter fill, from safe to clipping.
    pub meter: [Hsla; 3],

    /// Distinct colours for plotted series: left/right channels, voices, overlaid curves.
    pub series: [Hsla; 4],
    /// A sequential ramp for intensity: spectrograms, membrane displacement, heat maps. The
    /// first stop is the plot's own background, so silence disappears into it.
    pub heat: [Hsla; 5],
    /// Major grid lines in plots: octaves, 0 dB, bar lines.
    pub grid: Hsla,
    /// Minor grid lines in plots.
    pub grid_faint: Hsla,
    /// Colours for the data types of ports and wires in node graphs. None is the agent colour,
    /// so a wire never reads as the AI's work.
    pub ports: [Hsla; 5],

    /// The liquid-metal finish: the studio a polished surface reflects, from straight up to
    /// straight down, at the positions in [`crate::metal::STOPS`] — sky, bright band, a warm
    /// fringe, the dark horizon, a cool fringe, ground, bounce light. The two fringes are the
    /// colour a prism would split off at the horizon; everything else is near-neutral. Ornament
    /// only; see [`crate::metal`].
    pub metal: [Hsla; 9],

    /// Shadow colour; alpha is applied by [`Elevation`].
    pub shadow: Hsla,
}

/// Corner radii.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    /// Badges, checkboxes, small marks.
    pub radius_xs: Pixels,
    /// Controls: buttons, fields, segmented controls.
    pub radius_sm: Pixels,
    /// Cards, list groups, message bubbles.
    pub radius_md: Pixels,
    /// Panels, popovers, the composer.
    pub radius_lg: Pixels,
    /// Floating windows and sheets.
    pub radius_xl: Pixels,
    /// Draw buttons and segmented controls as capsules instead of [`Shape::radius_sm`].
    pub capsule_controls: bool,
}

/// Control sizes and row heights.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Density {
    /// Small controls: inline toolbars, table cells.
    pub control_sm: Pixels,
    /// The default control height.
    pub control_md: Pixels,
    /// Large controls: the composer's send button, call-to-action buttons.
    pub control_lg: Pixels,
    /// Height of a list or sidebar row.
    pub row: Pixels,
    /// Horizontal padding inside a default control.
    pub control_padding: Pixels,
    /// Padding inside a panel.
    pub panel_padding: Pixels,
    /// Height of the title bar / main toolbar.
    pub toolbar: Pixels,
}

/// Typefaces and the type scale.
#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    /// Interface font, with fallbacks for Japanese and other scripts.
    pub ui: Font,
    /// Monospaced font for code, values and timecode.
    pub mono: Font,
    /// Captions, badges, keyboard hints.
    pub size_xs: Pixels,
    /// Secondary labels, dense rows.
    pub size_sm: Pixels,
    /// Body text and control labels.
    pub size_md: Pixels,
    /// Section headings.
    pub size_lg: Pixels,
    /// Panel and sheet titles.
    pub size_xl: Pixels,
    /// Display numbers: tempo, timecode.
    pub size_display: Pixels,
    /// Weight of headings.
    pub heading_weight: FontWeight,
    /// Weight of control labels.
    pub label_weight: FontWeight,
}

/// The spacing scale. Pick a step by what two things mean to each other, never by eye:
/// parts of one control are closer than related controls, which are closer than groups.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    /// Optical correction: icon baselines, compact separators.
    pub xxs: Pixels,
    /// Parts of one control: icon and label in a menu row, title and description.
    pub xs: Pixels,
    /// Closely related controls: button icon and label, dialog actions.
    pub sm: Pixels,
    /// One content group: compact form rows, notification columns.
    pub md: Pixels,
    /// Separate groups in one section; panel padding.
    pub lg: Pixels,
    /// Separate sections.
    pub xl: Pixels,
    /// Major region boundaries and empty-state breathing room.
    pub xxl: Pixels,
}

/// Drop shadows, from barely lifted to floating.
#[derive(Clone, Debug, PartialEq)]
pub struct Elevation {
    /// Raised controls and cards.
    pub low: Vec<BoxShadow>,
    /// Popovers and menus.
    pub medium: Vec<BoxShadow>,
    /// Dialogs and floating panels.
    pub high: Vec<BoxShadow>,
}

/// The complete set of tokens in force. Install it with [`Theme::install`].
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// The design candidate this was derived from.
    pub direction: Direction,
    /// Light or dark.
    pub appearance: Appearance,
    /// Whether chrome is translucent.
    pub material: Material,
    /// How primary actions are filled.
    pub primary_style: PrimaryStyle,
    /// Semantic colours.
    pub colors: Colors,
    /// Corner radii.
    pub shape: Shape,
    /// Control and row sizes.
    pub density: Density,
    /// The spacing scale.
    pub spacing: Spacing,
    /// Fonts and the type scale.
    pub typography: Typography,
    /// Drop shadows.
    pub elevation: Elevation,
    /// Durations and whether to animate at all.
    pub motion: crate::motion::Motion,
}

impl Global for Theme {}

impl Default for Theme {
    fn default() -> Self {
        Self::new(Direction::default(), Appearance::default())
    }
}

/// Read access to the installed theme from any context.
pub trait ActiveTheme {
    /// The theme in force. Falls back to the default if none was installed.
    fn theme(&self) -> &Theme;
}

impl ActiveTheme for App {
    fn theme(&self) -> &Theme {
        // `init` installs a theme, so the fallback only matters for code that forgot to call it.
        static FALLBACK: std::sync::OnceLock<Theme> = std::sync::OnceLock::new();
        self.try_global::<Theme>()
            .unwrap_or_else(|| FALLBACK.get_or_init(Theme::default))
    }
}

/// The few decisions a palette follows from.
struct Seeds {
    background: u32,
    chrome: u32,
    surface: u32,
    elevated: u32,
    text: u32,
    muted: u32,
    accent: u32,
    agent: [u32; 3],
    success: u32,
    warning: u32,
    danger: u32,
    info: u32,
    /// Alpha of [`Colors::chrome`] under [`Material::Translucent`].
    chrome_alpha: f32,
}

impl Theme {
    /// Derive the theme for a candidate in the given appearance.
    pub fn new(direction: Direction, appearance: Appearance) -> Self {
        let seeds = seeds(direction, appearance);
        let material = match direction {
            Direction::Glass => Material::Translucent,
            Direction::Studio | Direction::Mono => Material::Opaque,
        };
        let primary_style = match direction {
            Direction::Mono => PrimaryStyle::Solid,
            Direction::Studio | Direction::Glass => PrimaryStyle::Accent,
        };
        Self {
            direction,
            appearance,
            material,
            primary_style,
            colors: colors(&seeds, direction, appearance, material),
            shape: shape(direction),
            density: density(direction),
            spacing: Spacing {
                xxs: px(2.),
                xs: px(4.),
                sm: px(8.),
                md: px(12.),
                lg: px(16.),
                xl: px(24.),
                xxl: px(32.),
            },
            typography: typography(direction),
            elevation: elevation(direction, appearance),
            motion: crate::motion::Motion::default(),
        }
    }

    /// Interface zoom: scale type, spacing, control sizes and radii together, so their
    /// relationships survive. Hairlines and shadows are left alone.
    pub fn zoomed(mut self, factor: f32) -> Self {
        let factor = factor.clamp(0.5, 3.);
        let typography = &mut self.typography;
        for size in [
            &mut typography.size_xs,
            &mut typography.size_sm,
            &mut typography.size_md,
            &mut typography.size_lg,
            &mut typography.size_xl,
            &mut typography.size_display,
        ] {
            *size *= factor;
        }
        let density = &mut self.density;
        for size in [
            &mut density.control_sm,
            &mut density.control_md,
            &mut density.control_lg,
            &mut density.row,
            &mut density.control_padding,
            &mut density.panel_padding,
            &mut density.toolbar,
        ] {
            *size *= factor;
        }
        let spacing = &mut self.spacing;
        for size in [
            &mut spacing.xxs,
            &mut spacing.xs,
            &mut spacing.sm,
            &mut spacing.md,
            &mut spacing.lg,
            &mut spacing.xl,
            &mut spacing.xxl,
        ] {
            *size *= factor;
        }
        let shape = &mut self.shape;
        for radius in [
            &mut shape.radius_xs,
            &mut shape.radius_sm,
            &mut shape.radius_md,
            &mut shape.radius_lg,
            &mut shape.radius_xl,
        ] {
            *radius *= factor;
        }
        self
    }

    /// The radius of a surface nested `inset` inside one of radius `outer`, so the band between
    /// them stays even through the corner.
    pub fn concentric(outer: Pixels, inset: Pixels) -> Pixels {
        (outer - inset).max(px(0.))
    }

    /// Install this theme as the application's and redraw every window.
    pub fn install(self, cx: &mut App) {
        cx.set_global(self);
        cx.refresh_windows();
    }

    /// The window background an application should request for this theme.
    ///
    /// Pass it to `WindowOptions::window_background`; once the window exists, go through
    /// [`crate::set_window_background`] instead, which keeps the macOS blur alive. Where the
    /// system cannot blur — most Linux desktops — translucent themes get an opaque window.
    pub fn window_background(&self) -> WindowBackgroundAppearance {
        if self.blurs_window() {
            WindowBackgroundAppearance::Blurred
        } else {
            WindowBackgroundAppearance::Opaque
        }
    }

    /// Fill for the window root. Under translucent chrome it is clear: the chrome supplies the
    /// only tint, because a tinted root under tinted chrome composites to an opaque surface and
    /// hides the blur. Where the system cannot blur, it is the opaque background, so the chrome
    /// reads as a tint over it rather than over the bare desktop.
    pub fn window_fill(&self) -> Hsla {
        if self.blurs_window() {
            gpui::transparent_black()
        } else {
            self.colors.background
        }
    }

    /// Whether this theme's window shows a blurred desktop: a translucent material on a system
    /// that blurs.
    fn blurs_window(&self) -> bool {
        self.material == Material::Translucent && crate::window::blur_supported()
    }

    /// Radius for a control of the given height, honouring [`Shape::capsule_controls`].
    pub fn control_radius(&self, height: Pixels) -> Pixels {
        if self.shape.capsule_controls {
            height / 2.
        } else {
            self.shape.radius_sm
        }
    }

    /// The readable text colour for a fill: whichever of black and white contrasts more.
    pub fn text_on(&self, fill: Hsla) -> Hsla {
        if contrast(gpui::white(), fill) >= contrast(gpui::black(), fill) {
            gpui::white()
        } else {
            hsla(0., 0., 0.08, 1.)
        }
    }

    /// Display name, for example "Studio Dark".
    pub fn name(&self) -> SharedString {
        format!("{} {}", self.direction.label(), self.appearance.label()).into()
    }
}

fn seeds(direction: Direction, appearance: Appearance) -> Seeds {
    // The agent gradient follows Apple Intelligence's warm-to-cool sweep in Studio and Glass;
    // Mono keeps to its one violet so the gradient does not undo its restraint.
    const APPLE_AGENT_DARK: [u32; 3] = [0xff9f45, 0xf05bb5, 0x7d7aff];
    const APPLE_AGENT_LIGHT: [u32; 3] = [0xe5761c, 0xd23c96, 0x5a55e8];
    match (direction, appearance) {
        (Direction::Studio, Appearance::Dark) => Seeds {
            background: 0x1b1b1d,
            chrome: 0x2a2a2d,
            surface: 0x232325,
            elevated: 0x303034,
            text: 0xececee,
            muted: 0xa3a3a8,
            accent: 0x0a84ff,
            agent: APPLE_AGENT_DARK,
            success: 0x32d74b,
            warning: 0xffd60a,
            danger: 0xff453a,
            info: 0x64d2ff,
            chrome_alpha: 1.,
        },
        (Direction::Studio, Appearance::Light) => Seeds {
            background: 0xe9e9eb,
            chrome: 0xdcdcdf,
            surface: 0xf4f4f6,
            elevated: 0xffffff,
            text: 0x1d1d1f,
            muted: 0x5b5b61,
            accent: 0x007aff,
            agent: APPLE_AGENT_LIGHT,
            success: 0x248a3d,
            warning: 0xb25000,
            danger: 0xd70015,
            info: 0x0071a4,
            chrome_alpha: 1.,
        },
        (Direction::Glass, Appearance::Dark) => Seeds {
            background: 0x16161a,
            chrome: 0x2c2c32,
            surface: 0x1e1e23,
            elevated: 0x2a2a30,
            text: 0xf5f5f7,
            muted: 0xa8a8b0,
            accent: 0x0a84ff,
            agent: APPLE_AGENT_DARK,
            success: 0x30d158,
            warning: 0xffd60a,
            danger: 0xff453a,
            info: 0x64d2ff,
            chrome_alpha: 0.42,
        },
        (Direction::Glass, Appearance::Light) => Seeds {
            background: 0xf5f5f7,
            chrome: 0xf9f9fb,
            surface: 0xffffff,
            elevated: 0xffffff,
            text: 0x1d1d1f,
            muted: 0x5e5e66,
            accent: 0x0071e3,
            agent: APPLE_AGENT_LIGHT,
            success: 0x248a3d,
            warning: 0xb25000,
            danger: 0xd70015,
            info: 0x0071a4,
            chrome_alpha: 0.5,
        },
        (Direction::Mono, Appearance::Dark) => Seeds {
            background: 0x060606,
            chrome: 0x0d0d0d,
            surface: 0x0e0e0e,
            elevated: 0x161618,
            text: 0xe8e8ea,
            muted: 0xa9a9ae,
            accent: 0x8b7cf6,
            agent: [0xa78bfa, 0x8b7cf6, 0x6d5ff0],
            success: 0x34d399,
            warning: 0xfacc15,
            danger: 0xf87171,
            info: 0x60a5fa,
            chrome_alpha: 1.,
        },
        (Direction::Mono, Appearance::Light) => Seeds {
            background: 0xffffff,
            chrome: 0xf3f3f5,
            surface: 0xffffff,
            elevated: 0xf7f7f9,
            text: 0x303035,
            muted: 0x62626a,
            accent: 0x5b43e8,
            agent: [0x7c5cf0, 0x5b43e8, 0x4630c9],
            success: 0x15803d,
            warning: 0xa16207,
            danger: 0xdc2626,
            info: 0x2563eb,
            chrome_alpha: 1.,
        },
    }
}

fn colors(
    seeds: &Seeds,
    direction: Direction,
    appearance: Appearance,
    material: Material,
) -> Colors {
    let dark = appearance.is_dark();
    let c = |value: u32| -> Hsla { rgb(value).into() };
    let background = c(seeds.background);
    let surface = c(seeds.surface);
    let elevated = c(seeds.elevated);
    let text = c(seeds.text);
    let chrome = match material {
        Material::Opaque => c(seeds.chrome),
        Material::Translucent => c(seeds.chrome).opacity(seeds.chrome_alpha),
    };
    // Washes are the foreground at low alpha, so they read on any surface they land on.
    let tone = if dark { gpui::white() } else { gpui::black() };
    let overlay = if dark {
        mix(elevated, gpui::white(), 0.04)
    } else {
        gpui::white()
    };
    let sunken = if dark {
        mix(background, gpui::black(), 0.25)
    } else {
        mix(surface, gpui::black(), 0.045)
    };

    let surfaces = [background, surface, elevated, overlay];
    let muted = surfaces.iter().fold(c(seeds.muted), |color, &fill| {
        ensure_contrast(color, fill, 4.5)
    });

    let accent = ensure_contrast(c(seeds.accent), surface, 3.0);
    let accent_text = surfaces
        .iter()
        .fold(accent, |color, &fill| ensure_contrast(color, fill, 4.5));
    let on_accent = if contrast(gpui::white(), accent) >= 3.0 {
        gpui::white()
    } else {
        hsla(0., 0., 0.08, 1.)
    };
    let solid = if dark { c(0xebebef) } else { c(0x232328) };
    let signal = |value: u32| {
        surfaces
            .iter()
            .fold(c(value), |color, &fill| ensure_contrast(color, fill, 3.0))
    };
    let success = signal(seeds.success);
    let warning = signal(seeds.warning);
    let danger = signal(seeds.danger);
    let info = signal(seeds.info);
    let rose = signal(if dark { 0xff6fae } else { 0xc2185b });
    let wash = if dark { 0.18 } else { 0.11 };

    let agent = seeds.agent.map(c);
    let agent_main = ensure_contrast(agent[1], surface, 3.0);

    let user_bubble = match direction {
        // Mono keeps colour for the AI alone; the person's words are a quiet grey card.
        Direction::Mono => mix(elevated, text, if dark { 0.06 } else { 0.04 }),
        Direction::Studio | Direction::Glass => accent,
    };
    let on_user_bubble = match direction {
        Direction::Mono => text,
        Direction::Studio | Direction::Glass => on_accent,
    };

    Colors {
        background,
        chrome,
        surface,
        elevated,
        overlay,
        sunken,
        hover: tone.opacity(if dark { 0.08 } else { 0.05 }),
        pressed: tone.opacity(if dark { 0.14 } else { 0.10 }),
        selected: match direction {
            Direction::Mono => tone.opacity(if dark { 0.10 } else { 0.07 }),
            Direction::Studio | Direction::Glass => accent.opacity(if dark { 0.28 } else { 0.16 }),
        },
        border: tone.opacity(if dark { 0.10 } else { 0.12 }),
        border_strong: tone.opacity(if dark { 0.18 } else { 0.22 }),
        separator: tone.opacity(if dark { 0.07 } else { 0.08 }),
        highlight: match direction {
            Direction::Studio => gpui::white().opacity(if dark { 0.07 } else { 0.7 }),
            Direction::Glass => gpui::white().opacity(if dark { 0.16 } else { 0.85 }),
            Direction::Mono => gpui::transparent_white(),
        },
        focus_ring: accent.opacity(0.55),
        text,
        text_muted: muted,
        text_faint: mix(muted, surface, 0.28),
        text_disabled: text.opacity(0.32),
        accent,
        accent_hover: if dark {
            lighten(accent, 0.06)
        } else {
            lighten(accent, -0.06)
        },
        accent_text,
        accent_wash: accent.opacity(wash),
        on_accent,
        solid,
        on_solid: if dark { c(0x0b0b0c) } else { gpui::white() },
        success,
        warning,
        danger,
        info,
        success_wash: success.opacity(wash),
        warning_wash: warning.opacity(wash),
        danger_wash: danger.opacity(wash),
        info_wash: info.opacity(wash),
        agent: agent_main,
        agent_wash: agent_main.opacity(if dark { 0.14 } else { 0.08 }),
        agent_gradient: agent,
        user_bubble,
        on_user_bubble,
        record: danger,
        playhead: match direction {
            // Logic draws its playhead white on dark; the others keep it in the accent.
            Direction::Studio if dark => c(0xf2f2f2),
            _ => accent,
        },
        meter: [success, warning, danger],
        series: [accent, agent_main, success, warning],
        heat: if dark {
            [
                sunken,
                mix(sunken, accent, 0.45),
                accent,
                mix(accent, warning, 0.7),
                mix(warning, gpui::white(), 0.6),
            ]
        } else {
            [
                sunken,
                mix(sunken, accent, 0.3),
                accent,
                mix(accent, danger, 0.6),
                mix(warning, danger, 0.3),
            ]
        },
        grid: tone.opacity(if dark { 0.12 } else { 0.14 }),
        grid_faint: tone.opacity(if dark { 0.05 } else { 0.06 }),
        ports: [
            match direction {
                // Mono's accent is its agent violet, so its first port type stays neutral.
                Direction::Mono => muted,
                Direction::Studio | Direction::Glass => accent,
            },
            success,
            warning,
            info,
            rose,
        ],
        metal: metal_seeds(direction).map(c),
        shadow: hsla(0., 0., 0., 1.),
    }
}

/// The metal each direction is finished in. Studio is cool steel with amber and blue fringes,
/// Glass bright platinum with peach and sky-blue fringes, and Mono black chrome whose fringes
/// are a rose and cyan sheen, like oil on a dark surface.
fn metal_seeds(direction: Direction) -> [u32; 9] {
    match direction {
        Direction::Studio => [
            0xa9b2bc, 0xdde3e9, 0x9aa2ab, 0xe39a5c, 0x15181c, 0x4f8fe8, 0x2e3339, 0x687078,
            0xa8afb6,
        ],
        Direction::Glass => [
            0xc6cfd9, 0xf6f8fb, 0xb4bcc6, 0xf3ad76, 0x1b1f25, 0x5aa6ff, 0x3a4048, 0x7f8790,
            0xd0d5db,
        ],
        Direction::Mono => [
            0x70747a, 0xc4c7cb, 0x6a6e73, 0xf29cc4, 0x060607, 0x86ddff, 0x141517, 0x2e3033,
            0x7a7d81,
        ],
    }
}

fn shape(direction: Direction) -> Shape {
    match direction {
        Direction::Studio => Shape {
            radius_xs: px(3.),
            radius_sm: px(5.),
            radius_md: px(7.),
            radius_lg: px(9.),
            radius_xl: px(12.),
            capsule_controls: false,
        },
        Direction::Glass => Shape {
            radius_xs: px(5.),
            radius_sm: px(9.),
            radius_md: px(14.),
            radius_lg: px(20.),
            radius_xl: px(26.),
            capsule_controls: true,
        },
        Direction::Mono => Shape {
            radius_xs: px(4.),
            radius_sm: px(7.),
            radius_md: px(10.),
            radius_lg: px(14.),
            radius_xl: px(18.),
            capsule_controls: false,
        },
    }
}

fn density(direction: Direction) -> Density {
    match direction {
        Direction::Studio => Density {
            control_sm: px(20.),
            control_md: px(24.),
            control_lg: px(30.),
            row: px(26.),
            control_padding: px(9.),
            panel_padding: px(10.),
            toolbar: px(40.),
        },
        Direction::Glass => Density {
            control_sm: px(24.),
            control_md: px(30.),
            control_lg: px(38.),
            row: px(32.),
            control_padding: px(14.),
            panel_padding: px(16.),
            toolbar: px(52.),
        },
        Direction::Mono => Density {
            control_sm: px(22.),
            control_md: px(28.),
            control_lg: px(34.),
            row: px(30.),
            control_padding: px(11.),
            panel_padding: px(14.),
            toolbar: px(44.),
        },
    }
}

fn typography(direction: Direction) -> Typography {
    let (xs, sm, md, lg, xl) = match direction {
        Direction::Studio => (10.5, 11.5, 12.5, 14., 17.),
        Direction::Glass => (11., 12., 13.5, 16., 21.),
        Direction::Mono => (11., 12., 13., 15., 19.),
    };
    Typography {
        ui: ui_font(None),
        mono: mono_font(),
        size_xs: px(xs),
        size_sm: px(sm),
        size_md: px(md),
        size_lg: px(lg),
        size_xl: px(xl),
        size_display: px(28.),
        heading_weight: match direction {
            Direction::Mono => FontWeight::MEDIUM,
            Direction::Studio | Direction::Glass => FontWeight::SEMIBOLD,
        },
        label_weight: FontWeight::MEDIUM,
    }
}

fn elevation(direction: Direction, appearance: Appearance) -> Elevation {
    let strength = match (direction, appearance.is_dark()) {
        (Direction::Mono, true) => 0.55,
        (_, true) => 0.42,
        (Direction::Glass, false) => 0.10,
        (_, false) => 0.14,
    };
    let shadow = |y: f32, blur: f32, alpha: f32| BoxShadow {
        color: hsla(0., 0., 0., alpha * strength),
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
    };
    Elevation {
        low: vec![shadow(1., 2., 0.6)],
        medium: vec![shadow(4., 12., 0.7), shadow(1., 3., 0.4)],
        high: vec![shadow(16., 40., 0.9), shadow(2., 8., 0.4)],
    }
}

/// The platform's interface font, with fallbacks for Japanese, Korean and Chinese.
///
/// Only families that exist on the current platform are listed: DirectWrite logs an error for
/// every missing family it is asked to map.
pub fn ui_font(family: Option<&str>) -> Font {
    let (base, fallbacks): (&str, &[&str]) = if cfg!(target_os = "macos") {
        (
            ".SystemUIFont",
            &["Hiragino Sans", "Apple SD Gothic Neo", "PingFang SC"],
        )
    } else if cfg!(target_os = "windows") {
        (
            "Segoe UI",
            &[
                "Yu Gothic UI",
                "Meiryo",
                "Malgun Gothic",
                "Microsoft YaHei UI",
            ],
        )
    } else {
        ("Inter", &["Noto Sans CJK JP", "DejaVu Sans"])
    };
    Font {
        fallbacks: Some(FontFallbacks::from_fonts(
            fallbacks
                .iter()
                .map(|family| (*family).to_owned())
                .collect(),
        )),
        ..gpui::font(family.unwrap_or(base).to_owned())
    }
}

/// The platform's monospaced font.
pub fn mono_font() -> Font {
    let (base, fallbacks): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("Menlo", &["Hiragino Sans"])
    } else if cfg!(target_os = "windows") {
        ("Cascadia Mono", &["Consolas", "Yu Gothic UI"])
    } else {
        ("DejaVu Sans Mono", &["Noto Sans CJK JP"])
    };
    Font {
        fallbacks: Some(FontFallbacks::from_fonts(
            fallbacks
                .iter()
                .map(|family| (*family).to_owned())
                .collect(),
        )),
        ..gpui::font(base.to_owned())
    }
}

/// WCAG contrast ratio between two colours, from 1 to 21. Alpha is ignored.
pub fn contrast(a: Hsla, b: Hsla) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn luminance(color: Hsla) -> f32 {
    let rgba: Rgba = color.into();
    let linear = |channel: f32| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(rgba.r) + 0.7152 * linear(rgba.g) + 0.0722 * linear(rgba.b)
}

/// Linear interpolation in RGB, from `a` (amount 0) to `b` (amount 1).
pub fn mix(a: Hsla, b: Hsla, amount: f32) -> Hsla {
    let amount = amount.clamp(0., 1.);
    // The ends are returned as given: a round trip through RGB would shift them slightly.
    if amount == 0. {
        return a;
    }
    if amount == 1. {
        return b;
    }
    let (a, b): (Rgba, Rgba) = (a.into(), b.into());
    let lerp = |x: f32, y: f32| x + (y - x) * amount;
    Rgba {
        r: lerp(a.r, b.r),
        g: lerp(a.g, b.g),
        b: lerp(a.b, b.b),
        a: lerp(a.a, b.a),
    }
    .into()
}

/// Shift lightness by `amount`, which may be negative.
pub fn lighten(color: Hsla, amount: f32) -> Hsla {
    Hsla {
        l: (color.l + amount).clamp(0., 1.),
        ..color
    }
}

/// Move `color` towards black or white until it reaches `minimum` contrast against `fill`.
pub fn ensure_contrast(color: Hsla, fill: Hsla, minimum: f32) -> Hsla {
    if contrast(color, fill) >= minimum {
        return color;
    }
    let target = if contrast(gpui::black(), fill) >= contrast(gpui::white(), fill) {
        gpui::black()
    } else {
        gpui::white()
    };
    (1..=40)
        .map(|step| mix(color, target, step as f32 / 40.))
        .find(|candidate| contrast(*candidate, fill) >= minimum)
        .unwrap_or(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_theme() -> impl Iterator<Item = Theme> {
        Direction::ALL.into_iter().flat_map(|direction| {
            Appearance::ALL
                .into_iter()
                .map(move |appearance| Theme::new(direction, appearance))
        })
    }

    #[test]
    fn text_is_readable_on_every_surface() {
        for theme in every_theme() {
            let colors = &theme.colors;
            for fill in [
                colors.background,
                colors.surface,
                colors.elevated,
                colors.overlay,
            ] {
                assert!(contrast(colors.text, fill) >= 7.0, "{}: text", theme.name());
                assert!(
                    contrast(colors.text_muted, fill) >= 4.5,
                    "{}: muted text",
                    theme.name()
                );
                assert!(
                    contrast(colors.accent_text, fill) >= 4.5,
                    "{}: accent text",
                    theme.name()
                );
            }
        }
    }

    #[test]
    fn fills_carry_readable_labels() {
        for theme in every_theme() {
            let colors = &theme.colors;
            assert!(
                contrast(colors.on_accent, colors.accent) >= 3.0,
                "{}",
                theme.name()
            );
            assert!(
                contrast(colors.on_solid, colors.solid) >= 7.0,
                "{}",
                theme.name()
            );
            assert!(
                contrast(colors.on_user_bubble, colors.user_bubble) >= 3.0,
                "{}",
                theme.name()
            );
        }
    }

    #[test]
    fn signals_stand_out_from_surfaces() {
        for theme in every_theme() {
            let colors = &theme.colors;
            for signal in [colors.success, colors.warning, colors.danger, colors.agent] {
                assert!(contrast(signal, colors.surface) >= 3.0, "{}", theme.name());
            }
        }
    }

    #[test]
    fn ports_stand_apart_from_the_agent() {
        for theme in every_theme() {
            let colors = &theme.colors;
            for port in colors.ports {
                assert!(contrast(port, colors.elevated) >= 3.0, "{}", theme.name());
                assert_ne!(port, colors.agent, "{}", theme.name());
            }
        }
    }

    #[test]
    fn metal_has_a_dark_crisp_horizon_and_neutral_body() {
        for theme in every_theme() {
            let metal = theme.colors.metal;
            let horizon = metal[4].l;
            assert!(
                metal.iter().all(|stop| stop.l >= horizon),
                "{}: the horizon is the darkest stop",
                theme.name()
            );
            assert!(
                metal[1].l - horizon >= 0.6,
                "{}: the horizon is crisp",
                theme.name()
            );
            // Colour lives in the fringes; the rest stays near-neutral so metal reads as metal.
            for (index, stop) in metal.iter().enumerate() {
                if index == 3 || index == 5 {
                    continue;
                }
                let rgb = Rgba::from(*stop);
                let chroma = rgb.r.max(rgb.g).max(rgb.b) - rgb.r.min(rgb.g).min(rgb.b);
                assert!(chroma <= 0.1, "{}: stop {index} is neutral", theme.name());
            }
        }
    }

    #[test]
    fn direction_ids_round_trip() {
        for direction in Direction::ALL {
            assert_eq!(Direction::from_id(direction.id()), Some(direction));
        }
    }
}
