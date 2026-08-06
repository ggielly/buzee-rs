//! Brand theme porting the original shadcn/Tailwind design tokens (slate palette)
//! plus the Buzee brand colors into iced. Light and dark variants match
//! `app.css` (`:root` / `.dark`) and `static/custom.css`.
//!
//! Uses iced 0.14 widget `Catalog` traits: each widget resolves a style from a
//! class, which is either one of our `*Kind` enums or a boxed style closure.

use iced::widget::{
    button, container, overlay::menu, pick_list, progress_bar, rule, scrollable, slider, svg, text,
    text_input, toggler,
};
use iced::{theme, Border, Color};

fn hex(hex: u32) -> Color {
    let r = ((hex >> 16) & 0xff) as f32 / 255.0;
    let g = ((hex >> 8) & 0xff) as f32 / 255.0;
    let b = (hex & 0xff) as f32 / 255.0;
    Color::from_rgb(r, g, b)
}

/// Complete set of palette + brand colors used across the UI.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: Color,
    pub foreground: Color,
    pub muted: Color,
    pub muted_foreground: Color,
    pub popover: Color,
    pub popover_foreground: Color,
    pub card: Color,
    pub border: Color,
    pub input: Color,
    pub primary: Color,
    pub primary_foreground: Color,
    pub secondary: Color,
    pub secondary_foreground: Color,
    pub accent: Color,
    pub accent_foreground: Color,
    pub destructive: Color,
    pub ring: Color,
    pub title_bar: Color,

    pub purple: Color,
    pub light_purple: Color,
    pub very_light_purple: Color,
    pub hot_pink: Color,
    pub search_bg: Color,
    pub lime_green: Color,
    pub success: Color,
}

impl Palette {
    pub fn light() -> Self {
        Self {
            background: hex(0xffffff),
            foreground: hex(0x020817),
            muted: hex(0xf1f5f9),
            muted_foreground: hex(0x64748b),
            popover: hex(0xffffff),
            popover_foreground: hex(0x020817),
            card: hex(0xffffff),
            border: hex(0xe2e8f0),
            input: hex(0xe2e8f0),
            primary: hex(0x0f172a),
            primary_foreground: hex(0xf8fafc),
            secondary: hex(0xf1f5f9),
            secondary_foreground: hex(0x0f172a),
            accent: hex(0xf1f5f9),
            accent_foreground: hex(0x0f172a),
            destructive: hex(0xef4444),
            ring: hex(0x020817),
            title_bar: hex(0xfafafa),

            purple: hex(0x5368e0),
            light_purple: hex(0x8c6ff7),
            very_light_purple: Color::from_rgba8(83, 104, 224, 0.125),
            hot_pink: hex(0xed7253),
            search_bg: hex(0xf2f2f2),
            lime_green: hex(0xa4e053),
            success: hex(0x198754),
        }
    }

    pub fn dark() -> Self {
        Self {
            background: hex(0x020817),
            foreground: hex(0xf8fafc),
            muted: hex(0x1e293b),
            muted_foreground: hex(0x94a3b8),
            popover: hex(0x0f172a),
            popover_foreground: hex(0xf8fafc),
            card: hex(0x020817),
            border: hex(0x1e293b),
            input: hex(0x1e293b),
            primary: hex(0xf8fafc),
            primary_foreground: hex(0x0f172a),
            secondary: hex(0x1e293b),
            secondary_foreground: hex(0xf8fafc),
            accent: hex(0x1e293b),
            accent_foreground: hex(0xf8fafc),
            destructive: hex(0x991b1b),
            ring: hex(0xcbd5e1),
            title_bar: hex(0x0a0f1e),

            purple: hex(0x8c6ff7),
            light_purple: hex(0xb09bff),
            very_light_purple: Color::from_rgba8(140, 111, 247, 0.16),
            hot_pink: hex(0xed7253),
            search_bg: hex(0x0d1424),
            lime_green: hex(0xa4e053),
            success: hex(0x2fb457),
        }
    }

    /// Brand color of a file type, used by the file-type badges.
    pub fn filetype_color(&self, file_type: &str) -> Color {
        match file_type.to_lowercase().as_str() {
            "pdf" => hex(0xdc3545),
            "docx" | "doc" => hex(0x0d6efd),
            "pptx" | "ppt" => hex(0xee6c45),
            "xlsx" | "xls" | "csv" => hex(0x146c43),
            "md" | "txt" => self.purple,
            "epub" => hex(0x63ee56),
            "mobi" => hex(0xfd9920),
            _ => self.muted_foreground,
        }
    }

    pub fn from_style(style: Theme) -> Self {
        let sniff = style.get_palette();
        let ext = style.get_extension();
        let mut p = if ext.is_nightly { Palette::dark() } else { Palette::light() };

        p.background = sniff.primary;
        p.foreground = sniff.text_body;
        p.muted = mix(sniff.primary, sniff.text_body, if ext.is_nightly { 0.12 } else { 0.07 });
        p.muted_foreground = mix(sniff.text_body, sniff.primary, if ext.is_nightly { 0.45 } else { 0.30 });
        p.popover = mix(sniff.primary, if ext.is_nightly { Color::WHITE } else { Color::BLACK }, if ext.is_nightly { 0.07 } else { 0.03 });
        p.popover_foreground = sniff.text_body;
        p.card = sniff.primary;
        p.border = mix(sniff.primary, sniff.text_body, if ext.is_nightly { 0.26 } else { 0.14 });
        p.input = p.border;
        p.primary = sniff.secondary;
        p.primary_foreground = sniff.text_headers;
        p.secondary = p.muted;
        p.secondary_foreground = sniff.text_body;
        p.accent = p.muted;
        p.accent_foreground = sniff.text_body;
        p.destructive = ext.red_alert_color;
        p.ring = sniff.secondary;
        p.title_bar = mix(sniff.primary, if ext.is_nightly { Color::WHITE } else { Color::BLACK }, if ext.is_nightly { 0.04 } else { 0.01 });
        p.search_bg = p.muted;
        p
    }
}

pub use crate::ui::styles::types::style_type::StyleType as Theme;

impl Theme {
    pub fn palette(&self) -> Palette {
        Palette::from_style(*self)
    }

    pub fn is_dark(&self) -> bool {
        self.get_extension().is_nightly
    }

    pub fn from_custom_file<P>(path: P) -> Option<Self>
    where
        P: AsRef<std::path::Path>,
    {
        crate::ui::styles::Palette::from_file(path)
            .map(|palette| Self::Custom(crate::ui::styles::CustomPalette::from_palette(palette)))
    }

    /// All selectable themes: the built-in styles plus the bundled custom
    /// TOML themes shipped in `resources/themes`.
    pub fn bundled_themes() -> Vec<ThemeChoice> {
        let mut choices: Vec<ThemeChoice> = Self::all_styles()
            .iter()
            .map(|&theme| ThemeChoice {
                name: theme.to_string(),
                theme,
            })
            .collect();

        let themes_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/themes");
        if let Ok(entries) = std::fs::read_dir(themes_dir) {
            let mut files: Vec<std::path::PathBuf> = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
                .collect();
            files.sort();
            for path in files {
                if let Some(theme) = Self::from_custom_file(&path) {
                    let name = path
                        .file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Custom".to_string());
                    choices.push(ThemeChoice { name, theme });
                }
            }
        }
        choices
    }
}

/// A named, selectable theme for the settings pick list.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeChoice {
    pub name: String,
    pub theme: Theme,
}

impl std::fmt::Display for ThemeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

impl theme::Base for Theme {
    fn default(preference: theme::Mode) -> Self {
        match preference {
            theme::Mode::Light | theme::Mode::None => Self::A11yLight,
            theme::Mode::Dark => Self::A11yDark,
        }
    }

    fn mode(&self) -> theme::Mode {
        if self.get_extension().is_nightly {
            theme::Mode::Dark
        } else {
            theme::Mode::Light
        }
    }

    fn base(&self) -> theme::Style {
        let p = self.palette();
        theme::Style {
            background_color: p.background,
            text_color: p.foreground,
        }
    }

    fn palette(&self) -> Option<theme::Palette> {
        let p = self.palette();
        Some(theme::Palette {
            background: p.background,
            text: p.foreground,
            primary: p.primary,
            success: p.success,
            warning: p.hot_pink,
            danger: p.destructive,
        })
    }

    fn name(&self) -> &str {
        match self {
            Self::A11yDark => "A11y Dark",
            Self::A11yLight => "A11y Light",
            Self::DraculaDark => "Dracula Dark",
            Self::DraculaLight => "Dracula Light",
            Self::GruvboxDark => "Gruvbox Dark",
            Self::GruvboxLight => "Gruvbox Light",
            Self::NordDark => "Nord Dark",
            Self::NordLight => "Nord Light",
            Self::SolarizedDark => "Solarized Dark",
            Self::SolarizedLight => "Solarized Light",
            Self::YetiDark => "Yeti Dark",
            Self::YetiLight => "Yeti Light",
            Self::Custom(_) => "Custom",
        }
    }
}

// ---------------------------------------------------------------------------
// Helper macro: define a class wrapper (Kind | StyleFn) + Catalog impl.
// ---------------------------------------------------------------------------

/// Creates the `*Class` enum (kind or closure) plus the `From` impls for a
/// widget whose `Catalog::style` does not receive a status.
macro_rules! widget_class {
    ($class:ident, $kind:ty, $catalog:ty, $style:ty, $module:ident, $stylefn:ident, $style_method:ident) => {
        pub enum $class<'a> {
            Kind($kind),
            Fn($module::$stylefn<'a, Theme>),
        }

        impl<'a> From<$kind> for $class<'a> {
            fn from(kind: $kind) -> Self {
                $class::Kind(kind)
            }
        }

        impl<'a> From<$module::$stylefn<'a, Theme>> for $class<'a> {
            fn from(f: $module::$stylefn<'a, Theme>) -> Self {
                $class::Fn(f)
            }
        }

        impl $catalog for Theme {
            type Class<'a> = $class<'a>;

            fn default<'a>() -> <Self as $catalog>::Class<'a> {
                $class::Kind(<$kind>::default())
            }

            fn style(&self, class: &<Self as $catalog>::Class<'_>) -> $style {
                match class {
                    $class::Kind(kind) => self.$style_method(*kind),
                    $class::Fn(f) => f(self),
                }
            }
        }
    };
}

/// Creates the `*Class` enum (kind or closure) plus the `From` impls for a
/// widget whose `Catalog::style` receives a `Status`.
macro_rules! widget_class_status {
    ($class:ident, $kind:ty, $catalog:ty, $style:ty, $module:ident, $stylefn:ident, $status:ty, $style_method:ident) => {
        pub enum $class<'a> {
            Kind($kind),
            Fn($module::$stylefn<'a, Theme>),
        }

        impl<'a> From<$kind> for $class<'a> {
            fn from(kind: $kind) -> Self {
                $class::Kind(kind)
            }
        }

        impl<'a> From<$module::$stylefn<'a, Theme>> for $class<'a> {
            fn from(f: $module::$stylefn<'a, Theme>) -> Self {
                $class::Fn(f)
            }
        }

        impl $catalog for Theme {
            type Class<'a> = $class<'a>;

            fn default<'a>() -> <Self as $catalog>::Class<'a> {
                $class::Kind(<$kind>::default())
            }

            fn style(&self, class: &<Self as $catalog>::Class<'_>, status: $status) -> $style {
                match class {
                    $class::Kind(kind) => self.$style_method(*kind, status),
                    $class::Fn(f) => f(self, status),
                }
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Container
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub enum ContainerKind {
    #[default]
    Transparent,
    /// Radii "0.5rem" shadcn card/popover surface with border.
    Card,
    /// Flat panel with a bottom border (header block).
    Panel,
    /// The 36px title bar, with a bottom border.
    TitleBar,
    /// Left sidebar ("menu") background.
    Sidebar,
    /// Highlighted (selected) content - e.g. a selected row.
    Selected,
    /// Hovered content background.
    Hover,
    /// Muted surface (--muted).
    Muted,
    /// A bordered popover menu surface with shadow + radius.
    Popover,
    /// A bordered rounded chip with the given fill color (file-type badges).
    Badge(Color),
    /// A filled rounded surface with the given color.
    Fill(Color),
}

widget_class!(ContainerClass, ContainerKind, container::Catalog, container::Style, container, StyleFn, container_style);

impl Theme {
    fn container_style(&self, kind: ContainerKind) -> container::Style {
        let p = self.palette();
        match kind {
            ContainerKind::Transparent => container::Style::default(),
            ContainerKind::Card => container::Style {
                background: Some(p.card.into()),
                border: Border { color: p.border, width: 1.0, radius: 8.into() },
                ..container::Style::default()
            },
            ContainerKind::Panel => container::Style {
                background: Some(p.background.into()),
                border: Border {
                    width: 1.0,
                    color: p.border,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            },
            ContainerKind::TitleBar => container::Style {
                background: Some(p.title_bar.into()),
                border: Border {
                    width: 1.0,
                    color: p.border,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            },
            ContainerKind::Sidebar => container::Style {
                background: Some(p.muted.into()),
                border: Border {
                    width: 1.0,
                    color: p.border,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            },
            ContainerKind::Selected => container::Style {
                background: Some(p.very_light_purple.into()),
                ..container::Style::default()
            },
            ContainerKind::Hover => container::Style {
                background: Some(p.accent.into()),
                ..container::Style::default()
            },
            ContainerKind::Muted => container::Style {
                background: Some(p.muted.into()),
                ..container::Style::default()
            },
            ContainerKind::Popover => container::Style {
                background: Some(p.popover.into()),
                border: Border { color: p.border, width: 1.0, radius: 8.into() },
                ..container::Style::default()
            },
            ContainerKind::Badge(color) => container::Style {
                background: Some(color.into()),
                border: iced::border::rounded(3),
                ..container::Style::default()
            },
            ContainerKind::Fill(color) => container::Style {
                background: Some(color.into()),
                border: iced::border::rounded(2),
                ..container::Style::default()
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Button
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub enum ButtonKind {
    #[default]
    Ghost,
    Primary,
    Muted,
    Purple,
    Danger,
    Link,
}

widget_class_status!(ButtonClass, ButtonKind, button::Catalog, button::Style, button, StyleFn, button::Status, button_style);

impl Theme {
    fn button_style(&self, kind: ButtonKind, status: button::Status) -> button::Style {
        match status {
            button::Status::Hovered => self.button_hovered(kind),
            _ => self.button_active(kind),
        }
    }

    fn button_active(&self, kind: ButtonKind) -> button::Style {
        let p = self.palette();
        let base = button::Style {
            border: iced::border::rounded(2),
            ..button::Style::default()
        };
        match kind {
            ButtonKind::Ghost => button::Style {
                background: None,
                text_color: p.foreground,
                ..base
            },
            ButtonKind::Primary => button::Style {
                background: Some(p.primary.into()),
                text_color: p.primary_foreground,
                ..base
            },
            ButtonKind::Muted => button::Style {
                background: Some(p.secondary.into()),
                text_color: p.foreground,
                ..base
            },
            ButtonKind::Purple => button::Style {
                background: Some(p.very_light_purple.into()),
                text_color: p.purple,
                border: Border { color: p.purple, width: 1.0, radius: 2.into() },
                ..base
            },
            ButtonKind::Danger => button::Style {
                background: Some(p.destructive.into()),
                text_color: Color::WHITE,
                ..base
            },
            ButtonKind::Link => button::Style {
                background: None,
                text_color: p.purple,
                ..base
            },
        }
    }

    fn button_hovered(&self, kind: ButtonKind) -> button::Style {
        let active = self.button_active(kind);
        let p = self.palette();
        match kind {
            ButtonKind::Link => button::Style {
                text_color: p.light_purple,
                ..active
            },
            _ => button::Style {
                background: active.background.map(|b| match b {
                    iced::Background::Color(c) => iced::Background::Color(mix(c, p.muted, 0.15)),
                    other => other,
                }),
                ..active
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub enum TextKind {
    #[default]
    Default,
    Muted,
    Error,
    Purple,
    OnPrimary,
    White,
    Color(Color),
}

widget_class!(TextClass, TextKind, text::Catalog, text::Style, text, StyleFn, text_style);

impl Theme {
    fn text_style(&self, kind: TextKind) -> text::Style {
        let p = self.palette();
        let color = match kind {
            TextKind::Default => p.foreground,
            TextKind::Muted => p.muted_foreground,
            TextKind::Error => p.destructive,
            TextKind::Purple => p.purple,
            TextKind::OnPrimary => p.primary_foreground,
            TextKind::White => Color::WHITE,
            TextKind::Color(c) => c,
        };
        text::Style { color: Some(color) }
    }
}

// ---------------------------------------------------------------------------
// Progress bar
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub enum ProgressKind {
    #[default]
    Default,
}

widget_class!(ProgressClass, ProgressKind, progress_bar::Catalog, progress_bar::Style, progress_bar, StyleFn, progress_style);

impl Theme {
    fn progress_style(&self, _kind: ProgressKind) -> progress_bar::Style {
        let p = self.palette();
        progress_bar::Style {
            background: p.muted.into(),
            bar: p.purple.into(),
            border: Border::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Text input
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub enum InputKind {
    #[default]
    Default,
    Search,
}

widget_class_status!(InputClass, InputKind, text_input::Catalog, text_input::Style, text_input, StyleFn, text_input::Status, input_style);

impl Theme {
    fn input_style(&self, kind: InputKind, status: text_input::Status) -> text_input::Style {
        match status {
            text_input::Status::Focused { .. } => self.input_focused(kind),
            text_input::Status::Disabled => self.input_disabled(kind),
            _ => self.input_active(kind),
        }
    }

    fn input_active(&self, kind: InputKind) -> text_input::Style {
        let p = self.palette();
        match kind {
            InputKind::Search => text_input::Style {
                background: p.search_bg.into(),
                border: Border { color: p.border, width: 1.0, radius: 6.into() },
                icon: p.muted_foreground,
                placeholder: p.muted_foreground,
                value: p.foreground,
                selection: p.purple,
            },
            InputKind::Default => text_input::Style {
                background: p.input.into(),
                border: Border { color: p.border, width: 1.0, radius: 6.into() },
                icon: p.muted_foreground,
                placeholder: p.muted_foreground,
                value: p.foreground,
                selection: p.purple,
            },
        }
    }

    fn input_focused(&self, kind: InputKind) -> text_input::Style {
        text_input::Style {
            border: Border { color: self.palette().ring, width: 2.0, radius: 6.into() },
            ..self.input_active(kind)
        }
    }

    fn input_disabled(&self, kind: InputKind) -> text_input::Style {
        text_input::Style {
            border: Border { color: self.palette().border, width: 1.0, radius: 6.into() },
            ..self.input_active(kind)
        }
    }
}

// ---------------------------------------------------------------------------
// Scrollable
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub enum ScrollKind {
    #[default]
    Default,
}

widget_class_status!(ScrollClass, ScrollKind, scrollable::Catalog, scrollable::Style, scrollable, StyleFn, scrollable::Status, scroll_style);

impl Theme {
    fn scroll_style(&self, kind: ScrollKind, status: scrollable::Status) -> scrollable::Style {
        let _ = kind;
        let p = self.palette();
        let rail = scrollable::Rail {
            background: None,
            border: Border::default(),
            scroller: scrollable::Scroller {
                background: match status {
                    scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. } => {
                        Color::from_rgb(0.58, 0.60, 0.64).into()
                    }
                    _ => p.border.into(),
                },
                border: Border::default(),
            },
        };
        scrollable::Style {
            container: container::Style::default(),
            vertical_rail: rail,
            horizontal_rail: rail,
            gap: None,
            auto_scroll: scrollable::AutoScroll {
                background: p.popover.into(),
                border: iced::border::rounded(8),
                shadow: iced::Shadow::default(),
                icon: p.muted_foreground,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Rule
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub enum RuleKind {
    #[default]
    Default,
}

widget_class!(RuleClass, RuleKind, rule::Catalog, rule::Style, rule, StyleFn, rule_style);

impl Theme {
    fn rule_style(&self, _kind: RuleKind) -> rule::Style {
        rule::Style {
            color: self.palette().border,
            radius: 0.0.into(),
            fill_mode: rule::FillMode::Full,
            snap: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Pick list
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub enum PickKind {
    #[default]
    Default,
}

widget_class_status!(PickClass, PickKind, pick_list::Catalog, pick_list::Style, pick_list, StyleFn, pick_list::Status, pick_style);

impl Theme {
    fn pick_style(&self, kind: PickKind, status: pick_list::Status) -> pick_list::Style {
        let active = self.pick_active(kind);
        match status {
            pick_list::Status::Active => active,
            pick_list::Status::Hovered | pick_list::Status::Opened { .. } => pick_list::Style {
                border: Border { color: self.palette().purple, width: 1.0, radius: 4.into() },
                ..active
            },
        }
    }

    fn pick_active(&self, _kind: PickKind) -> pick_list::Style {
        let p = self.palette();
        pick_list::Style {
            text_color: p.foreground,
            placeholder_color: p.muted_foreground,
            handle_color: p.muted_foreground,
            background: p.secondary.into(),
            border: Border { color: p.border, width: 1.0, radius: 4.into() },
        }
    }
}

// ---------------------------------------------------------------------------
// Menu (pick-list overlay)
// ---------------------------------------------------------------------------

widget_class!(MenuClass, PickKind, menu::Catalog, menu::Style, menu, StyleFn, menu_style);

impl Theme {
    fn menu_style(&self, _kind: PickKind) -> menu::Style {
        let p = self.palette();
        menu::Style {
            text_color: p.foreground,
            background: p.popover.into(),
            border: Border { color: p.border, width: 1.0, radius: 8.into() },
            selected_text_color: p.purple,
            selected_background: p.very_light_purple.into(),
            shadow: iced::Shadow::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// SVG
// ---------------------------------------------------------------------------

/// The logo/vector graphics keep their intrinsic colors (no color filter).
impl svg::Catalog for Theme {
    type Class<'a> = ();

    fn default<'a>() -> Self::Class<'a> {}

    fn style(&self, _class: &(), _status: svg::Status) -> svg::Style {
        svg::Style { color: None }
    }
}

// ---------------------------------------------------------------------------
// Toggler (on/off switch)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub enum TogglerKind {
    #[default]
    Default,
}

widget_class_status!(TogglerClass, TogglerKind, toggler::Catalog, toggler::Style, toggler, StyleFn, toggler::Status, toggler_style);

impl Theme {
    fn toggler_style(&self, _kind: TogglerKind, status: toggler::Status) -> toggler::Style {
        let p = self.palette();
        let is_toggled = match status {
            toggler::Status::Active { is_toggled }
            | toggler::Status::Hovered { is_toggled, .. }
            | toggler::Status::Disabled { is_toggled } => is_toggled,
        };
        toggler::Style {
            background: (if is_toggled { p.purple } else { p.muted_foreground }).into(),
            background_border_width: 0.0,
            background_border_color: Color::TRANSPARENT,
            foreground: Color::WHITE.into(),
            foreground_border_width: 1.0,
            foreground_border_color: p.popover,
            text_color: None,
            border_radius: None,
            padding_ratio: 1.0 / 5.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Slider
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub enum SliderKind {
    #[default]
    Default,
}

widget_class_status!(SliderClass, SliderKind, slider::Catalog, slider::Style, slider, StyleFn, slider::Status, slider_style);

impl Theme {
    fn slider_style(&self, _kind: SliderKind, _status: slider::Status) -> slider::Style {
        let p = self.palette();
        slider::Style {
            rail: slider::Rail {
                // (before handle, after handle) rail colours.
                backgrounds: (p.muted.into(), p.purple.into()),
                width: 4.0,
                border: Border::default(),
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 7.0 },
                background: p.purple.into(),
                border_width: 1.0,
                border_color: p.popover,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Results table (iced_table)
// ---------------------------------------------------------------------------

/// Zebra-striped table surface using the slate palette; the selected-row
/// highlight is applied per-cell (see `result_table`), since the row style is
/// global to the virtualized table.
impl iced_table::Catalog for Theme {
    type Style = ();

    fn header(&self, _style: &Self::Style) -> container::Style {
        let p = self.palette();
        container::Style {
            text_color: Some(p.muted_foreground),
            background: Some(p.muted.into()),
            border: Border { color: p.border, width: 1.0, radius: 0.0.into() },
            ..container::Style::default()
        }
    }

    fn footer(&self, style: &Self::Style) -> container::Style {
        self.header(style)
    }

    fn row(&self, _style: &Self::Style, index: usize) -> container::Style {
        let p = self.palette();
        container::Style {
            text_color: Some(p.foreground),
            background: Some(if index % 2 == 0 { p.background } else { p.muted }.into()),
            ..container::Style::default()
        }
    }

    fn divider(&self, _style: &Self::Style, hovered: bool) -> container::Style {
        let p = self.palette();
        container::Style {
            background: Some(if hovered { p.purple } else { p.border }.into()),
            ..container::Style::default()
        }
    }
}

/// Linearly mix two colors; `amount` of `b` applied over `a`.
fn mix(a: Color, b: Color, amount: f32) -> Color {
    let t = amount.clamp(0.0, 1.0);
    Color::from_rgba(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a,
    )
}
