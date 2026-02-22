use gpui::Hsla;

/// Type of decoration positioning relative to text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionDecorationType {
    Before,
    After,
    Range,
    WholeLine,
}

/// Controls how decorations behave at range boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionDecorationRangeBehavior {
    OpenOpen,
    OpenClosed,
    ClosedOpen,
    ClosedClosed,
}

/// Visual content to render as part of a decoration.
#[derive(Debug, Clone, PartialEq)]
pub enum ExtensionDecorationContent {
    Text(String),
    Svg {
        source: String,
        width_px: f32,
        height_px: f32,
    },
    Image {
        source: String,
        width_px: f32,
        height_px: f32,
    },
}

/// Styling properties for decorations.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExtensionDecorationStyle {
    pub background_color: Option<Hsla>,
    pub border_color: Option<String>,
    pub border_style: Option<String>,
    pub border_width: Option<String>,
    pub border_radius: Option<String>,
    pub margin: Option<String>,
    pub z_index: Option<i32>,
}

/// Theme-specific decoration styling.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionThemedDecorationStyle {
    pub base: ExtensionDecorationStyle,
    pub light: Option<ExtensionDecorationStyle>,
    pub dark: Option<ExtensionDecorationStyle>,
}

/// Complete render options for a decoration type.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionDecorationRenderOptions {
    pub decoration_type: ExtensionDecorationType,
    pub content: Option<ExtensionDecorationContent>,
    pub style: ExtensionThemedDecorationStyle,
    pub range_behavior: ExtensionDecorationRangeBehavior,
}

/// A decoration instance with byte-offset range (from WASM guest).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionDecoration {
    pub range_start: u32,
    pub range_end: u32,
}
