//! Decoration API for rendering overlays and highlights in the editor.
//!
//! This module provides types for creating decorations that can be rendered
//! alongside editor text. Decorations are visual enhancements that don't modify
//! the underlying buffer content but provide additional visual information.
//!
//! Cursorless uses decorations for:
//! - **Hat decorations**: SVG overlays positioned above characters for voice targeting
//! - **Range highlights**: Background color highlights for visual feedback during operations
//! - **Scope visualizer**: Border decorations showing language scope boundaries
//!
//! The API is designed to be:
//! - **Serializable**: Types can cross the WASM boundary for extension use
//! - **Theme-aware**: Support for light/dark theme variants
//! - **Performant**: Efficient representation for thousands of decorations
//! - **Buffer-aware**: Decorations track through text edits via anchors

use gpui::{Hsla, SharedString};
use multi_buffer::Anchor;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Parse SVG data URI and extract the SVG content as bytes.
///
/// Supports two formats:
/// 1. `data:image/svg+xml;utf8,<svg>...</svg>` - UTF-8 encoded
/// 2. `data:image/svg+xml;base64,PHN2Zy4uLjwvc3ZnPg==` - Base64 encoded
///
/// Returns the decoded SVG bytes if the URI is valid, None otherwise.
fn parse_svg_data_uri(uri: &str) -> Option<Vec<u8>> {
    let uri = uri.trim();

    if !uri.starts_with("data:") {
        return None;
    }

    let uri = uri.strip_prefix("data:")?;

    let (mime_and_encoding, data) = uri.split_once(',')?;

    if !mime_and_encoding.starts_with("image/svg+xml") {
        return None;
    }

    if mime_and_encoding.contains("base64") {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .decode(data.as_bytes())
            .ok()
    } else {
        Some(data.as_bytes().to_vec())
    }
}

/// Unique identifier for a decoration type.
///
/// Decoration types define the visual style of decorations. Multiple decoration
/// instances can share the same type. The ID is used to look up the decoration's
/// rendering properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DecorationTypeId(pub usize);

/// Unique identifier for a decoration instance.
///
/// Each decoration instance has a unique ID that persists across updates.
/// This allows decorations to be efficiently updated or removed without
/// recreating the entire decoration set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DecorationId(pub usize);

/// Defines where and how a decoration is positioned relative to text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecorationType {
    /// Content rendered before a character position.
    ///
    /// Used for hat decorations in Cursorless - SVG shapes positioned above
    /// characters using negative margins to create visual overlays.
    Before,

    /// Content rendered after a character position.
    ///
    /// Could be used for inline annotations or trailing decorations.
    After,

    /// Highlight applied to a range of text.
    ///
    /// Used for temporary flash effects (delete preview, copy feedback) and
    /// persistent highlights (stored targets in Cursorless).
    Range,

    /// Highlight applied to entire lines.
    ///
    /// Similar to Range but always extends to full line width, used for
    /// line-level highlighting in Cursorless.
    WholeLine,
}

/// Visual content to render as part of a decoration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DecorationContent {
    /// Plain text content.
    Text(SharedString),

    /// SVG image from a data URI or file path.
    ///
    /// For Cursorless hats, this contains SVG data URIs like:
    /// `data:image/svg+xml;utf8,<svg>...</svg>`
    ///
    /// The SVG should be self-contained with embedded colors and dimensions.
    Svg {
        /// SVG source - either a data URI or file path
        source: SharedString,

        /// Width in pixels (used for sizing and positioning)
        width_px: f32,

        /// Height in pixels (used for sizing and positioning)
        height_px: f32,
    },

    /// Image from a file path or data URI.
    ///
    /// Generic image support for PNG, JPG, etc. if needed beyond SVG.
    Image {
        /// Image source - either a data URI or file path
        source: SharedString,

        /// Width in pixels
        width_px: f32,

        /// Height in pixels
        height_px: f32,
    },
}

/// Styling properties for decorations.
///
/// These properties control the visual appearance of decorations.
/// Different decoration types use different subsets of these properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecorationStyle {
    /// Background color for range and whole-line decorations.
    ///
    /// For Cursorless highlights, this provides the flash colors:
    /// - Red for pending deletes
    /// - Blue for referenced ranges
    /// - Green for newly added text
    /// - Yellow/orange for modifications
    pub background_color: Option<Hsla>,

    /// Border color (can specify per-side for scope visualizer).
    ///
    /// Format: "top right bottom left" or single color for all sides.
    /// Example: "#ff0000 #00ff00 #0000ff #ffff00"
    pub border_color: Option<String>,

    /// Border style (can specify per-side for scope visualizer).
    ///
    /// Format: "top right bottom left" or single style for all sides.
    /// Values: "solid", "dashed", "dotted", "none"
    /// Example: "solid dashed dashed solid"
    pub border_style: Option<String>,

    /// Border width in pixels.
    ///
    /// Example: "1px" or "2px"
    pub border_width: Option<String>,

    /// Border radius for rounded corners.
    ///
    /// Format: "top-left top-right bottom-right bottom-left"
    /// Example: "2px 0px 0px 0px" (rounded top-left only)
    pub border_radius: Option<String>,

    /// CSS margin for positioning before/after content.
    ///
    /// For Cursorless hats, this uses negative margins to position SVGs above text:
    /// Example: "-20px -10px 0 0" (negative top and right to position above)
    pub margin: Option<String>,

    /// Z-index for layering decorations.
    ///
    /// Higher values render on top. Useful for overlapping decorations.
    pub z_index: Option<i32>,
}

impl Default for DecorationStyle {
    fn default() -> Self {
        Self {
            background_color: None,
            border_color: None,
            border_style: None,
            border_width: None,
            border_radius: None,
            margin: None,
            z_index: None,
        }
    }
}

/// Theme-specific decoration styling.
///
/// Allows different styles for light and dark themes. If theme-specific
/// styles are not provided, the base style is used for both themes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemedDecorationStyle {
    /// Base style used when theme-specific styles are not available.
    pub base: DecorationStyle,

    /// Override style for light theme.
    pub light: Option<DecorationStyle>,

    /// Override style for dark theme.
    pub dark: Option<DecorationStyle>,
}

impl ThemedDecorationStyle {
    /// Create a themed style with only a base style.
    pub fn new(base: DecorationStyle) -> Self {
        Self {
            base,
            light: None,
            dark: None,
        }
    }

    /// Create a themed style with light and dark variants.
    pub fn with_variants(
        base: DecorationStyle,
        light: DecorationStyle,
        dark: DecorationStyle,
    ) -> Self {
        Self {
            base,
            light: Some(light),
            dark: Some(dark),
        }
    }

    /// Get the appropriate style for a theme (true = light, false = dark).
    pub fn style_for_theme(&self, is_light: bool) -> &DecorationStyle {
        if is_light {
            self.light.as_ref().unwrap_or(&self.base)
        } else {
            self.dark.as_ref().unwrap_or(&self.base)
        }
    }
}

/// Complete render options for a decoration type.
///
/// This combines the decoration type, content, and styling into a complete
/// specification that can be used to render decorations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecorationRenderOptions {
    /// Type of decoration (before, after, range, whole-line).
    pub decoration_type: DecorationType,

    /// Optional content to render (for before/after decorations).
    pub content: Option<DecorationContent>,

    /// Themed styling for the decoration.
    pub style: ThemedDecorationStyle,

    /// How the decoration behaves at range boundaries.
    pub range_behavior: DecorationRangeBehavior,
}

/// Controls how decorations behave at the start/end of ranges.
///
/// This affects what happens when the cursor is at a range boundary or when
/// text is inserted at the edges of a decorated range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecorationRangeBehavior {
    /// Range includes text inserted at both start and end positions.
    OpenOpen,

    /// Range excludes text inserted at start, includes at end.
    OpenClosed,

    /// Range includes text inserted at start, excludes at end.
    ClosedOpen,

    /// Range excludes text inserted at both start and end positions.
    ///
    /// This is the default for Cursorless decorations - it prevents the
    /// decoration from expanding when text is inserted at boundaries.
    ClosedClosed,
}

impl Default for DecorationRangeBehavior {
    fn default() -> Self {
        Self::ClosedClosed
    }
}

impl DecorationRangeBehavior {
    /// Convert range behavior to anchor biases for start and end positions.
    ///
    /// Returns (start_bias, end_bias) tuple:
    /// - Left bias means the anchor doesn't move when text is inserted at that position
    /// - Right bias means the anchor moves forward when text is inserted at that position
    ///
    /// # Examples
    ///
    /// ```
    /// use editor::decorations::DecorationRangeBehavior;
    /// use text::Bias;
    ///
    /// let (start, end) = DecorationRangeBehavior::ClosedClosed.to_bias();
    /// assert_eq!(start, Bias::Left);
    /// assert_eq!(end, Bias::Left);
    /// ```
    pub fn to_bias(self) -> (text::Bias, text::Bias) {
        use text::Bias;
        match self {
            // Closed means the boundary doesn't expand (Left bias)
            // Open means the boundary expands (Right bias)
            DecorationRangeBehavior::ClosedClosed => (Bias::Left, Bias::Left),
            DecorationRangeBehavior::OpenOpen => (Bias::Right, Bias::Right),
            DecorationRangeBehavior::ClosedOpen => (Bias::Left, Bias::Right),
            DecorationRangeBehavior::OpenClosed => (Bias::Right, Bias::Left),
        }
    }
}

/// A decoration instance applied to the editor.
///
/// Each decoration tracks a position or range in the buffer using anchors,
/// which automatically update as the buffer is edited.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decoration {
    /// Unique identifier for this decoration instance.
    pub id: DecorationId,

    /// The decoration type ID that defines how this decoration renders.
    pub type_id: DecorationTypeId,

    /// Starting position or single position for point decorations.
    pub start: Anchor,

    /// Optional end position for range decorations.
    ///
    /// If None, this is a point decoration (before/after type).
    /// If Some, this is a range decoration (range/whole-line type).
    pub end: Option<Anchor>,
}

impl Decoration {
    /// Create a new point decoration (before/after).
    pub fn point(id: DecorationId, type_id: DecorationTypeId, position: Anchor) -> Self {
        Self {
            id,
            type_id,
            start: position,
            end: None,
        }
    }

    /// Create a new range decoration (range/whole-line).
    pub fn range(
        id: DecorationId,
        type_id: DecorationTypeId,
        start: Anchor,
        end: Anchor,
    ) -> Self {
        Self {
            id,
            type_id,
            start,
            end: Some(end),
        }
    }

    /// Check if this is a point decoration.
    pub fn is_point(&self) -> bool {
        self.end.is_none()
    }

    /// Check if this is a range decoration.
    pub fn is_range(&self) -> bool {
        self.end.is_some()
    }
}

/// Builder for creating decoration render options with a fluent API.
///
/// This provides a convenient way to construct decoration specifications
/// without manually filling all fields.
pub struct DecorationRenderOptionsBuilder {
    decoration_type: DecorationType,
    content: Option<DecorationContent>,
    base_style: DecorationStyle,
    light_style: Option<DecorationStyle>,
    dark_style: Option<DecorationStyle>,
    range_behavior: DecorationRangeBehavior,
}

impl DecorationRenderOptionsBuilder {
    /// Create a builder for a before decoration.
    pub fn before() -> Self {
        Self::new(DecorationType::Before)
    }

    /// Create a builder for an after decoration.
    pub fn after() -> Self {
        Self::new(DecorationType::After)
    }

    /// Create a builder for a range decoration.
    pub fn range() -> Self {
        Self::new(DecorationType::Range)
    }

    /// Create a builder for a whole-line decoration.
    pub fn whole_line() -> Self {
        Self::new(DecorationType::WholeLine)
    }

    fn new(decoration_type: DecorationType) -> Self {
        Self {
            decoration_type,
            content: None,
            base_style: DecorationStyle::default(),
            light_style: None,
            dark_style: None,
            range_behavior: DecorationRangeBehavior::default(),
        }
    }

    /// Set text content for the decoration.
    pub fn with_text(mut self, text: impl Into<SharedString>) -> Self {
        self.content = Some(DecorationContent::Text(text.into()));
        self
    }

    /// Set SVG content for the decoration.
    ///
    /// # Panics
    ///
    /// Panics if width_px or height_px are not positive (> 0.0).
    pub fn with_svg(mut self, source: impl Into<SharedString>, width_px: f32, height_px: f32) -> Self {
        assert!(
            width_px > 0.0 && height_px > 0.0,
            "SVG dimensions must be positive (width: {}, height: {})",
            width_px,
            height_px
        );
        self.content = Some(DecorationContent::Svg {
            source: source.into(),
            width_px,
            height_px,
        });
        self
    }

    /// Set background color for the decoration.
    pub fn with_background_color(mut self, color: Hsla) -> Self {
        self.base_style.background_color = Some(color);
        self
    }

    /// Set margin for positioning.
    pub fn with_margin(mut self, margin: impl Into<String>) -> Self {
        self.base_style.margin = Some(margin.into());
        self
    }

    /// Set border properties.
    pub fn with_border(
        mut self,
        color: impl Into<String>,
        style: impl Into<String>,
        width: impl Into<String>,
    ) -> Self {
        self.base_style.border_color = Some(color.into());
        self.base_style.border_style = Some(style.into());
        self.base_style.border_width = Some(width.into());
        self
    }

    /// Set border radius.
    pub fn with_border_radius(mut self, radius: impl Into<String>) -> Self {
        self.base_style.border_radius = Some(radius.into());
        self
    }

    /// Set z-index for layering.
    pub fn with_z_index(mut self, z_index: i32) -> Self {
        self.base_style.z_index = Some(z_index);
        self
    }

    /// Set light theme style override.
    pub fn with_light_style(mut self, style: DecorationStyle) -> Self {
        self.light_style = Some(style);
        self
    }

    /// Set dark theme style override.
    pub fn with_dark_style(mut self, style: DecorationStyle) -> Self {
        self.dark_style = Some(style);
        self
    }

    /// Set range behavior.
    pub fn with_range_behavior(mut self, behavior: DecorationRangeBehavior) -> Self {
        self.range_behavior = behavior;
        self
    }

    /// Build the final decoration render options.
    pub fn build(self) -> DecorationRenderOptions {
        let style = if self.light_style.is_some() || self.dark_style.is_some() {
            ThemedDecorationStyle {
                base: self.base_style,
                light: self.light_style,
                dark: self.dark_style,
            }
        } else {
            ThemedDecorationStyle::new(self.base_style)
        };

        DecorationRenderOptions {
            decoration_type: self.decoration_type,
            content: self.content,
            style,
            range_behavior: self.range_behavior,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoration_type_id() {
        let id1 = DecorationTypeId(1);
        let id2 = DecorationTypeId(1);
        let id3 = DecorationTypeId(2);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_decoration_id() {
        let id1 = DecorationId(1);
        let id2 = DecorationId(1);
        let id3 = DecorationId(2);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_decoration_type_variants() {
        let before = DecorationType::Before;
        let after = DecorationType::After;
        let range = DecorationType::Range;
        let whole_line = DecorationType::WholeLine;

        assert_ne!(before, after);
        assert_ne!(range, whole_line);
    }

    #[test]
    fn test_decoration_content_text() {
        let content = DecorationContent::Text("test".into());
        match content {
            DecorationContent::Text(text) => assert_eq!(text.as_ref(), "test"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_decoration_content_svg() {
        let svg_source = "data:image/svg+xml;utf8,<svg></svg>";
        let content = DecorationContent::Svg {
            source: svg_source.into(),
            width_px: 10.0,
            height_px: 20.0,
        };

        match content {
            DecorationContent::Svg {
                source,
                width_px,
                height_px,
            } => {
                assert_eq!(source.as_ref(), svg_source);
                assert_eq!(width_px, 10.0);
                assert_eq!(height_px, 20.0);
            }
            _ => panic!("Expected Svg variant"),
        }
    }

    #[test]
    fn test_decoration_style_default() {
        let style = DecorationStyle::default();
        assert!(style.background_color.is_none());
        assert!(style.border_color.is_none());
        assert!(style.margin.is_none());
    }

    #[test]
    fn test_themed_style_creation() {
        let base = DecorationStyle::default();
        let themed = ThemedDecorationStyle::new(base.clone());

        assert_eq!(themed.base, base);
        assert!(themed.light.is_none());
        assert!(themed.dark.is_none());
    }

    #[test]
    fn test_themed_style_with_variants() {
        let base = DecorationStyle::default();
        let mut light = DecorationStyle::default();
        light.background_color = Some(Hsla::white());
        let mut dark = DecorationStyle::default();
        dark.background_color = Some(Hsla::black());

        let themed = ThemedDecorationStyle::with_variants(base, light.clone(), dark.clone());

        assert_eq!(themed.style_for_theme(true), &light);
        assert_eq!(themed.style_for_theme(false), &dark);
    }

    #[test]
    fn test_themed_style_fallback() {
        let mut base = DecorationStyle::default();
        base.background_color = Some(Hsla::white());
        let themed = ThemedDecorationStyle::new(base.clone());

        assert_eq!(themed.style_for_theme(true), &base);
        assert_eq!(themed.style_for_theme(false), &base);
    }

    #[test]
    fn test_range_behavior_default() {
        let behavior = DecorationRangeBehavior::default();
        assert_eq!(behavior, DecorationRangeBehavior::ClosedClosed);
    }

    #[test]
    fn test_builder_before_decoration() {
        let options = DecorationRenderOptionsBuilder::before()
            .with_text("test")
            .with_margin("-10px 0 0 0")
            .build();

        assert_eq!(options.decoration_type, DecorationType::Before);
        assert!(matches!(options.content, Some(DecorationContent::Text(_))));
        assert_eq!(
            options.style.base.margin,
            Some("-10px 0 0 0".to_string())
        );
    }

    #[test]
    fn test_builder_range_decoration() {
        let color = Hsla::red();
        let options = DecorationRenderOptionsBuilder::range()
            .with_background_color(color)
            .build();

        assert_eq!(options.decoration_type, DecorationType::Range);
        assert_eq!(options.style.base.background_color, Some(color));
    }

    #[test]
    fn test_builder_svg_decoration() {
        let svg = "data:image/svg+xml;utf8,<svg></svg>";
        let options = DecorationRenderOptionsBuilder::before()
            .with_svg(svg, 12.0, 9.0)
            .with_margin("-9px -12px 0 0")
            .build();

        match options.content {
            Some(DecorationContent::Svg {
                source,
                width_px,
                height_px,
            }) => {
                assert_eq!(source.as_ref(), svg);
                assert_eq!(width_px, 12.0);
                assert_eq!(height_px, 9.0);
            }
            _ => panic!("Expected Svg content"),
        }
    }

    #[test]
    fn test_builder_with_border() {
        let options = DecorationRenderOptionsBuilder::range()
            .with_border("#ff0000", "solid", "1px")
            .with_border_radius("2px")
            .build();

        assert_eq!(options.style.base.border_color, Some("#ff0000".to_string()));
        assert_eq!(options.style.base.border_style, Some("solid".to_string()));
        assert_eq!(options.style.base.border_width, Some("1px".to_string()));
        assert_eq!(options.style.base.border_radius, Some("2px".to_string()));
    }

    #[test]
    fn test_builder_with_theme_variants() {
        let mut light = DecorationStyle::default();
        light.background_color = Some(Hsla::white());
        let mut dark = DecorationStyle::default();
        dark.background_color = Some(Hsla::black());

        let options = DecorationRenderOptionsBuilder::range()
            .with_light_style(light.clone())
            .with_dark_style(dark.clone())
            .build();

        assert_eq!(options.style.light, Some(light));
        assert_eq!(options.style.dark, Some(dark));
    }

    #[test]
    fn test_builder_z_index() {
        let options = DecorationRenderOptionsBuilder::before()
            .with_z_index(100)
            .build();

        assert_eq!(options.style.base.z_index, Some(100));
    }

    // Serialization tests - critical for extension communication

    #[test]
    fn test_serialize_decoration_type_id() {
        let id = DecorationTypeId(42);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "42");

        let deserialized: DecorationTypeId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_serialize_decoration_id() {
        let id = DecorationId(123);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "123");

        let deserialized: DecorationId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_serialize_decoration_type() {
        let types = vec![
            DecorationType::Before,
            DecorationType::After,
            DecorationType::Range,
            DecorationType::WholeLine,
        ];

        for dt in types {
            let json = serde_json::to_string(&dt).unwrap();
            let deserialized: DecorationType = serde_json::from_str(&json).unwrap();
            assert_eq!(dt, deserialized);
        }
    }

    #[test]
    fn test_serialize_decoration_content_text() {
        let content = DecorationContent::Text("hello world".into());
        let json = serde_json::to_string(&content).unwrap();
        let deserialized: DecorationContent = serde_json::from_str(&json).unwrap();

        match deserialized {
            DecorationContent::Text(text) => assert_eq!(text.as_ref(), "hello world"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_serialize_decoration_content_svg() {
        let content = DecorationContent::Svg {
            source: "data:image/svg+xml;utf8,<svg></svg>".into(),
            width_px: 12.0,
            height_px: 9.0,
        };

        let json = serde_json::to_string(&content).unwrap();
        let deserialized: DecorationContent = serde_json::from_str(&json).unwrap();

        match deserialized {
            DecorationContent::Svg {
                source,
                width_px,
                height_px,
            } => {
                assert_eq!(source.as_ref(), "data:image/svg+xml;utf8,<svg></svg>");
                assert_eq!(width_px, 12.0);
                assert_eq!(height_px, 9.0);
            }
            _ => panic!("Expected Svg variant"),
        }
    }

    #[test]
    fn test_serialize_decoration_style() {
        let mut style = DecorationStyle::default();
        style.background_color = Some(Hsla::red());
        style.margin = Some("-10px 0 0 0".to_string());
        style.z_index = Some(5);

        let json = serde_json::to_string(&style).unwrap();
        let deserialized: DecorationStyle = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.background_color, Some(Hsla::red()));
        assert_eq!(deserialized.margin, Some("-10px 0 0 0".to_string()));
        assert_eq!(deserialized.z_index, Some(5));
    }

    #[test]
    fn test_serialize_themed_decoration_style() {
        let mut base = DecorationStyle::default();
        base.background_color = Some(Hsla::white());

        let mut light = DecorationStyle::default();
        light.background_color = Some(Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.9,
            a: 1.0,
        });

        let mut dark = DecorationStyle::default();
        dark.background_color = Some(Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.1,
            a: 1.0,
        });

        let themed = ThemedDecorationStyle::with_variants(base, light.clone(), dark.clone());

        let json = serde_json::to_string(&themed).unwrap();
        let deserialized: ThemedDecorationStyle = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.light, Some(light));
        assert_eq!(deserialized.dark, Some(dark));
    }

    #[test]
    fn test_serialize_range_behavior() {
        let behaviors = vec![
            DecorationRangeBehavior::OpenOpen,
            DecorationRangeBehavior::OpenClosed,
            DecorationRangeBehavior::ClosedOpen,
            DecorationRangeBehavior::ClosedClosed,
        ];

        for behavior in behaviors {
            let json = serde_json::to_string(&behavior).unwrap();
            let deserialized: DecorationRangeBehavior = serde_json::from_str(&json).unwrap();
            assert_eq!(behavior, deserialized);
        }
    }

    #[test]
    fn test_serialize_decoration_render_options() {
        let options = DecorationRenderOptionsBuilder::before()
            .with_svg("data:image/svg+xml;utf8,<svg></svg>", 12.0, 9.0)
            .with_margin("-9px -12px 0 0")
            .with_z_index(10)
            .build();

        let json = serde_json::to_string(&options).unwrap();
        let deserialized: DecorationRenderOptions = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.decoration_type, DecorationType::Before);
        assert!(matches!(
            deserialized.content,
            Some(DecorationContent::Svg { .. })
        ));
        assert_eq!(deserialized.style.base.margin, Some("-9px -12px 0 0".to_string()));
        assert_eq!(deserialized.style.base.z_index, Some(10));
    }

    #[test]
    fn test_serialize_decoration_point() {
        let anchor = Anchor::min();
        let decoration = Decoration::point(DecorationId(42), DecorationTypeId(1), anchor);

        let json = serde_json::to_string(&decoration).unwrap();
        let deserialized: Decoration = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, DecorationId(42));
        assert_eq!(deserialized.type_id, DecorationTypeId(1));
        assert!(deserialized.is_point());
        assert!(!deserialized.is_range());
    }

    #[test]
    fn test_serialize_decoration_range() {
        let start = Anchor::min();
        let end = Anchor::max();
        let decoration = Decoration::range(DecorationId(100), DecorationTypeId(5), start, end);

        let json = serde_json::to_string(&decoration).unwrap();
        let deserialized: Decoration = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, DecorationId(100));
        assert_eq!(deserialized.type_id, DecorationTypeId(5));
        assert!(deserialized.is_range());
        assert!(!deserialized.is_point());
    }

    // Property tests - verify invariants and edge cases

    #[test]
    fn test_decoration_point_is_consistent() {
        // Point decorations should never have an end anchor
        let anchor = Anchor::min();
        let decoration = Decoration::point(DecorationId(1), DecorationTypeId(1), anchor);

        assert!(decoration.is_point());
        assert!(!decoration.is_range());
        assert!(decoration.end.is_none());
    }

    #[test]
    fn test_decoration_range_is_consistent() {
        // Range decorations should always have both start and end
        let start = Anchor::min();
        let end = Anchor::max();
        let decoration =
            Decoration::range(DecorationId(1), DecorationTypeId(1), start, end);

        assert!(decoration.is_range());
        assert!(!decoration.is_point());
        assert!(decoration.end.is_some());
    }

    #[test]
    fn test_themed_style_always_returns_valid_reference() {
        // Themed styles should never panic when getting style for theme
        let base = DecorationStyle::default();
        let themed = ThemedDecorationStyle::new(base.clone());

        let light_style = themed.style_for_theme(true);
        let dark_style = themed.style_for_theme(false);

        // Should return base when no theme-specific style exists
        assert_eq!(light_style, &base);
        assert_eq!(dark_style, &base);
    }

    #[test]
    fn test_themed_style_prefers_theme_specific() {
        let mut base = DecorationStyle::default();
        base.background_color = Some(Hsla::white());

        let mut light = DecorationStyle::default();
        light.background_color = Some(Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.9,
            a: 1.0,
        });

        let mut dark = DecorationStyle::default();
        dark.background_color = Some(Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.1,
            a: 1.0,
        });

        let themed = ThemedDecorationStyle::with_variants(base, light.clone(), dark.clone());

        // Should return theme-specific styles when available
        assert_eq!(themed.style_for_theme(true), &light);
        assert_eq!(themed.style_for_theme(false), &dark);
    }

    #[test]
    fn test_builder_accumulates_properties() {
        // Builder should accumulate all properties correctly
        let options = DecorationRenderOptionsBuilder::range()
            .with_background_color(Hsla::red())
            .with_border("#ff0000", "solid", "1px")
            .with_border_radius("2px")
            .with_z_index(5)
            .build();

        assert_eq!(options.style.base.background_color, Some(Hsla::red()));
        assert_eq!(options.style.base.border_color, Some("#ff0000".to_string()));
        assert_eq!(options.style.base.border_style, Some("solid".to_string()));
        assert_eq!(options.style.base.border_width, Some("1px".to_string()));
        assert_eq!(options.style.base.border_radius, Some("2px".to_string()));
        assert_eq!(options.style.base.z_index, Some(5));
    }

    #[test]
    fn test_builder_default_range_behavior() {
        let options = DecorationRenderOptionsBuilder::before().build();
        assert_eq!(options.range_behavior, DecorationRangeBehavior::ClosedClosed);
    }

    #[test]
    fn test_builder_custom_range_behavior() {
        let options = DecorationRenderOptionsBuilder::before()
            .with_range_behavior(DecorationRangeBehavior::OpenOpen)
            .build();
        assert_eq!(options.range_behavior, DecorationRangeBehavior::OpenOpen);
    }

    // Edge case tests

    #[test]
    fn test_empty_svg_source() {
        let content = DecorationContent::Svg {
            source: "".into(),
            width_px: 0.0,
            height_px: 0.0,
        };

        match content {
            DecorationContent::Svg {
                source,
                width_px,
                height_px,
            } => {
                assert_eq!(source.as_ref(), "");
                assert_eq!(width_px, 0.0);
                assert_eq!(height_px, 0.0);
            }
            _ => panic!("Expected Svg variant"),
        }
    }

    #[test]
    fn test_negative_dimensions() {
        // Should handle negative dimensions without panicking
        let content = DecorationContent::Svg {
            source: "test".into(),
            width_px: -10.0,
            height_px: -20.0,
        };

        match content {
            DecorationContent::Svg { width_px, height_px, .. } => {
                assert_eq!(width_px, -10.0);
                assert_eq!(height_px, -20.0);
            }
            _ => panic!("Expected Svg variant"),
        }
    }

    #[test]
    fn test_very_large_dimensions() {
        let content = DecorationContent::Svg {
            source: "test".into(),
            width_px: f32::MAX,
            height_px: f32::MAX,
        };

        match content {
            DecorationContent::Svg { width_px, height_px, .. } => {
                assert_eq!(width_px, f32::MAX);
                assert_eq!(height_px, f32::MAX);
            }
            _ => panic!("Expected Svg variant"),
        }
    }

    #[test]
    fn test_border_style_single_value() {
        let style = DecorationStyle {
            border_style: Some("solid".to_string()),
            ..Default::default()
        };

        assert_eq!(style.border_style, Some("solid".to_string()));
    }

    #[test]
    fn test_border_style_per_side() {
        let style = DecorationStyle {
            border_style: Some("solid dashed dashed solid".to_string()),
            ..Default::default()
        };

        assert_eq!(
            style.border_style,
            Some("solid dashed dashed solid".to_string())
        );
    }

    #[test]
    fn test_margin_formats() {
        let styles = vec![
            "-10px 0 0 0",
            "-10px",
            "-10px -5px",
            "0",
        ];

        for margin_str in styles {
            let style = DecorationStyle {
                margin: Some(margin_str.to_string()),
                ..Default::default()
            };
            assert_eq!(style.margin, Some(margin_str.to_string()));
        }
    }

    #[test]
    fn test_cursorless_hat_example() {
        // Example: Blue hat with default shape positioned above character
        let hat_svg = "data:image/svg+xml;utf8,<svg width=\"1em\" height=\"1em\" viewBox=\"0 0 12 9\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\"><path d=\"M6 9C9.31371 9 12 6.98528 12 4.5C12 2.01472 9.31371 0 6 0C2.68629 0 0 2.01472 0 4.5C0 6.98528 2.68629 9 6 9Z\" fill=\"#0000ff\"/></svg>";

        let options = DecorationRenderOptionsBuilder::before()
            .with_svg(hat_svg, 12.0, 9.0)
            .with_margin("-9px -12px 0 0")
            .with_range_behavior(DecorationRangeBehavior::ClosedClosed)
            .build();

        assert_eq!(options.decoration_type, DecorationType::Before);
        assert!(matches!(
            options.content,
            Some(DecorationContent::Svg { .. })
        ));
        assert_eq!(options.style.base.margin, Some("-9px -12px 0 0".to_string()));
        assert_eq!(options.range_behavior, DecorationRangeBehavior::ClosedClosed);
    }

    #[test]
    fn test_cursorless_flash_highlight_example() {
        // Example: Red background for pending delete
        let pending_delete_color = Hsla {
            h: 0.0,       // Red hue
            s: 1.0,       // Full saturation
            l: 0.5,       // Medium lightness
            a: 0.54,      // ~54% opacity (0x8a / 255)
        };

        let options = DecorationRenderOptionsBuilder::range()
            .with_background_color(pending_delete_color)
            .with_range_behavior(DecorationRangeBehavior::ClosedClosed)
            .build();

        assert_eq!(options.decoration_type, DecorationType::Range);
        assert_eq!(options.style.base.background_color, Some(pending_delete_color));
    }

    #[test]
    fn test_cursorless_scope_visualizer_example() {
        // Example: Top line of a multi-line scope
        let options = DecorationRenderOptionsBuilder::range()
            .with_border(
                "#010002c0 #010001c0 #010001c0 #010002c0",
                "solid dashed dashed solid",
                "1px",
            )
            .with_border_radius("2px 0px 0px 0px")
            .build();

        assert_eq!(options.decoration_type, DecorationType::Range);
        assert_eq!(
            options.style.base.border_color,
            Some("#010002c0 #010001c0 #010001c0 #010002c0".to_string())
        );
        assert_eq!(
            options.style.base.border_style,
            Some("solid dashed dashed solid".to_string())
        );
        assert_eq!(options.style.base.border_width, Some("1px".to_string()));
        assert_eq!(
            options.style.base.border_radius,
            Some("2px 0px 0px 0px".to_string())
        );
    }

    #[test]
    fn test_multiple_decorations_same_type() {
        // Multiple decoration instances can share the same type
        let type_id = DecorationTypeId(1);
        let anchor1 = Anchor::min();
        let anchor2 = Anchor::max();

        let dec1 = Decoration::point(DecorationId(1), type_id, anchor1);
        let dec2 = Decoration::point(DecorationId(2), type_id, anchor2);

        assert_eq!(dec1.type_id, dec2.type_id);
        assert_ne!(dec1.id, dec2.id);
    }

    #[test]
    fn test_decoration_ids_are_unique() {
        let id1 = DecorationId(1);
        let id2 = DecorationId(2);
        let id3 = DecorationId(1);

        assert_ne!(id1, id2);
        assert_eq!(id1, id3);
    }

    #[test]
    #[should_panic(expected = "SVG dimensions must be positive")]
    fn test_builder_rejects_zero_width() {
        DecorationRenderOptionsBuilder::before()
            .with_svg("test", 0.0, 10.0)
            .build();
    }

    #[test]
    #[should_panic(expected = "SVG dimensions must be positive")]
    fn test_builder_rejects_zero_height() {
        DecorationRenderOptionsBuilder::before()
            .with_svg("test", 10.0, 0.0)
            .build();
    }

    #[test]
    #[should_panic(expected = "SVG dimensions must be positive")]
    fn test_builder_rejects_negative_width() {
        DecorationRenderOptionsBuilder::before()
            .with_svg("test", -10.0, 10.0)
            .build();
    }

    #[test]
    #[should_panic(expected = "SVG dimensions must be positive")]
    fn test_builder_rejects_negative_height() {
        DecorationRenderOptionsBuilder::before()
            .with_svg("test", 10.0, -10.0)
            .build();
    }
}

/// In-memory storage system for managing decorations across editor instances.
///
/// The `DecorationRegistry` manages the lifecycle of decoration types and instances.
/// It tracks which decorations belong to which editors and handles cleanup when
/// editors close or decoration types are disposed.
///
/// # Design
///
/// - **Decoration Types**: Stored by `DecorationTypeId`, defining how decorations render
/// - **Decoration Instances**: Stored per-editor, each referencing a decoration type
/// - **Editor Separation**: Each editor's decorations are isolated by `EntityId`
/// - **Thread Safety**: Uses `RwLock` for concurrent access from multiple threads
///
/// # Usage
///
/// ```rust,ignore
/// use gpui::EntityId;
/// use editor::decorations::{DecorationRegistry, DecorationRenderOptionsBuilder};
/// use multi_buffer::Anchor;
///
/// let registry = DecorationRegistry::new();
///
/// // Create a decoration type for hats
/// let hat_options = DecorationRenderOptionsBuilder::before()
///     .with_svg("data:image/svg+xml;utf8,<svg></svg>", 12.0, 9.0)
///     .build();
/// let type_id = registry.create_decoration_type(hat_options);
///
/// // Add decorations to an editor
/// let editor_id = EntityId::from(1);
/// let anchor = Anchor::min();
/// let decoration = Decoration::point(DecorationId(1), type_id, anchor);
/// registry.set_decorations(editor_id, type_id, vec![decoration]);
///
/// // Retrieve decorations
/// let all_decorations = registry.get_decorations(editor_id);
/// let type_decorations = registry.get_decorations_for_type(editor_id, type_id);
///
/// // Cleanup
/// registry.clear_editor(editor_id);
/// registry.dispose_decoration_type(type_id);
/// ```
use std::sync::{Arc, RwLock};
use gpui::EntityId;

/// Registry for managing decoration types and instances across editors.
///
/// This registry provides centralized storage for all decorations in the application.
/// It uses interior mutability via `RwLock` to allow concurrent access.
pub struct DecorationRegistry {
    inner: Arc<RwLock<DecorationRegistryInner>>,
}

struct DecorationRegistryInner {
    /// Storage for decoration type render options, indexed by type ID.
    decoration_types: HashMap<DecorationTypeId, DecorationTypeData>,

    /// Storage for decoration instances, indexed by editor ID.
    /// Each editor maintains its own collection of decorations.
    editor_decorations: HashMap<EntityId, EditorDecorations>,

    /// Counter for generating unique decoration type IDs.
    next_type_id: usize,
}

/// Metadata about a decoration type.
struct DecorationTypeData {
    /// The render options defining how this decoration type appears.
    options: DecorationRenderOptions,

    /// Reference count tracking how many decoration instances use this type.
    /// When this reaches zero, the type can be safely disposed.
    reference_count: usize,

    /// Cached parsed SVG bytes for data URI decorations.
    /// This is computed once when the decoration type is created to avoid
    /// parsing the same SVG data URI thousands of times per frame.
    cached_svg_bytes: Option<Vec<u8>>,
}

/// All decorations for a single editor.
struct EditorDecorations {
    /// All decoration instances, indexed by decoration ID.
    decorations: HashMap<DecorationId, Decoration>,

    /// Index mapping type IDs to the set of decoration IDs using that type.
    /// This allows efficient lookup of all decorations of a specific type.
    decorations_by_type: HashMap<DecorationTypeId, HashSet<DecorationId>>,
}

impl DecorationRegistry {
    /// Create a new empty decoration registry.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(DecorationRegistryInner {
                decoration_types: HashMap::new(),
                editor_decorations: HashMap::new(),
                next_type_id: 1,
            })),
        }
    }

    /// Create a new decoration type with the given render options.
    ///
    /// Returns a unique `DecorationTypeId` that can be used to create decoration
    /// instances of this type.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let options = DecorationRenderOptionsBuilder::before()
    ///     .with_text("→")
    ///     .build();
    /// let type_id = registry.create_decoration_type(options);
    /// ```
    pub fn create_decoration_type(&self, options: DecorationRenderOptions) -> DecorationTypeId {
        let mut inner = self.inner.write().expect("registry lock poisoned");
        let type_id = DecorationTypeId(inner.next_type_id);
        inner.next_type_id += 1;

        // Parse and cache SVG data URIs once at creation time to avoid
        // parsing thousands of times per frame during rendering
        let cached_svg_bytes = if let Some(DecorationContent::Svg { ref source, .. }) = options.content {
            parse_svg_data_uri(source.as_ref())
        } else {
            None
        };

        inner.decoration_types.insert(
            type_id,
            DecorationTypeData {
                options,
                reference_count: 0,
                cached_svg_bytes,
            },
        );

        type_id
    }

    /// Get the render options for a decoration type.
    ///
    /// Returns `None` if the type ID doesn't exist.
    pub fn get_decoration_type(&self, type_id: DecorationTypeId) -> Option<DecorationRenderOptions> {
        let inner = self.inner.read().expect("registry lock poisoned");
        inner.decoration_types.get(&type_id).map(|data| data.options.clone())
    }

    /// Get the render options and cached SVG bytes for a decoration type.
    ///
    /// Returns `None` if the type ID doesn't exist.
    /// The cached SVG bytes will be Some if the decoration type has SVG content
    /// with a data URI that was successfully parsed at creation time.
    pub fn get_decoration_type_with_cache(&self, type_id: DecorationTypeId) -> Option<(DecorationRenderOptions, Option<Vec<u8>>)> {
        let inner = self.inner.read().expect("registry lock poisoned");
        inner.decoration_types.get(&type_id).map(|data| (data.options.clone(), data.cached_svg_bytes.clone()))
    }

    /// Set the decorations for a specific type in an editor.
    ///
    /// This replaces any existing decorations of the given type in the editor.
    /// The decorations are indexed by their IDs for efficient updates and removal.
    ///
    /// # Arguments
    ///
    /// * `editor_id` - The editor to add decorations to
    /// * `type_id` - The decoration type (must have been created via `create_decoration_type`)
    /// * `decorations` - Vector of decoration instances to add
    ///
    /// # Panics
    ///
    /// Panics if `type_id` doesn't exist in the registry.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let decorations = vec![
    ///     Decoration::point(DecorationId(1), type_id, anchor1),
    ///     Decoration::point(DecorationId(2), type_id, anchor2),
    /// ];
    /// registry.set_decorations(editor_id, type_id, decorations);
    /// ```
    pub fn set_decorations(
        &self,
        editor_id: EntityId,
        type_id: DecorationTypeId,
        decorations: Vec<Decoration>,
    ) {
        let mut inner = self.inner.write().expect("registry lock poisoned");

        // Verify the type exists
        let type_data = inner.decoration_types.get_mut(&type_id)
            .expect("decoration type must exist before setting decorations");

        // Get or create the editor's decoration storage
        let editor_decorations = inner.editor_decorations
            .entry(editor_id)
            .or_insert_with(|| EditorDecorations {
                decorations: HashMap::new(),
                decorations_by_type: HashMap::new(),
            });

        // Remove old decorations of this type
        if let Some(old_decoration_ids) = editor_decorations.decorations_by_type.get(&type_id) {
            for old_id in old_decoration_ids {
                editor_decorations.decorations.remove(old_id);
                type_data.reference_count = type_data.reference_count.saturating_sub(1);
            }
        }

        // Add new decorations
        let mut decoration_ids = HashSet::new();
        for decoration in decorations {
            decoration_ids.insert(decoration.id);
            editor_decorations.decorations.insert(decoration.id, decoration);
            type_data.reference_count += 1;
        }

        // Update the type index
        if decoration_ids.is_empty() {
            editor_decorations.decorations_by_type.remove(&type_id);
        } else {
            editor_decorations.decorations_by_type.insert(type_id, decoration_ids);
        }
    }

    /// Get all decorations for an editor.
    ///
    /// Returns an empty vector if the editor has no decorations.
    ///
    /// # Note
    ///
    /// This method returns `Vec<&Decoration>` instead of cloning decorations
    /// to avoid unnecessary allocations. For most use cases, this is more efficient
    /// than `get_decorations_owned()`.
    pub fn get_decorations(&self, editor_id: EntityId) -> Vec<&Decoration> {
        let inner = self.inner.read().expect("registry lock poisoned");
        inner.editor_decorations
            .get(&editor_id)
            .map(|ed| ed.decorations.values().collect())
            .unwrap_or_default()
    }

    /// Get all decorations for an editor (owned).
    ///
    /// Returns owned copies of decorations. Use `get_decorations()` if you only
    /// need to read decoration data.
    pub fn get_decorations_owned(&self, editor_id: EntityId) -> Vec<Decoration> {
        let inner = self.inner.read().expect("registry lock poisoned");
        inner.editor_decorations
            .get(&editor_id)
            .map(|ed| ed.decorations.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all decorations of a specific type for an editor.
    ///
    /// Returns an empty vector if the editor has no decorations of this type.
    ///
    /// # Note
    ///
    /// This method returns `Vec<&Decoration>` instead of cloning decorations
    /// to avoid unnecessary allocations. For most use cases, this is more efficient
    /// than `get_decorations_for_type_owned()`.
    pub fn get_decorations_for_type(
        &self,
        editor_id: EntityId,
        type_id: DecorationTypeId,
    ) -> Vec<&Decoration> {
        let inner = self.inner.read().expect("registry lock poisoned");
        inner.editor_decorations
            .get(&editor_id)
            .and_then(|ed| {
                ed.decorations_by_type.get(&type_id).map(|decoration_ids| {
                    decoration_ids
                        .iter()
                        .filter_map(|id| ed.decorations.get(id))
                        .collect()
                })
            })
            .unwrap_or_default()
    }

    /// Get all decorations of a specific type for an editor (owned).
    ///
    /// Returns owned copies of decorations. Use `get_decorations_for_type()` if you only
    /// need to read decoration data.
    pub fn get_decorations_for_type_owned(
        &self,
        editor_id: EntityId,
        type_id: DecorationTypeId,
    ) -> Vec<Decoration> {
        let inner = self.inner.read().expect("registry lock poisoned");
        inner.editor_decorations
            .get(&editor_id)
            .and_then(|ed| {
                ed.decorations_by_type.get(&type_id).map(|decoration_ids| {
                    decoration_ids
                        .iter()
                        .filter_map(|id| ed.decorations.get(id).cloned())
                        .collect()
                })
            })
            .unwrap_or_default()
    }

    /// Remove a specific decoration type from the registry.
    ///
    /// This removes the type definition and all decoration instances using this type
    /// across all editors.
    ///
    /// # Arguments
    ///
    /// * `type_id` - The decoration type to dispose
    ///
    /// # Returns
    ///
    /// `true` if the type was removed, `false` if it didn't exist.
    pub fn dispose_decoration_type(&self, type_id: DecorationTypeId) -> bool {
        let mut inner = self.inner.write().expect("registry lock poisoned");

        // Remove the type definition
        if inner.decoration_types.remove(&type_id).is_none() {
            return false;
        }

        // Remove all decorations of this type from all editors
        for editor_decorations in inner.editor_decorations.values_mut() {
            if let Some(decoration_ids) = editor_decorations.decorations_by_type.remove(&type_id) {
                for decoration_id in decoration_ids {
                    editor_decorations.decorations.remove(&decoration_id);
                }
            }
        }

        true
    }

    /// Get the range behavior for a decoration type.
    ///
    /// This is useful when creating anchors for decorations - you can use the
    /// returned behavior's `to_bias()` method to determine the correct bias
    /// for start and end anchors.
    ///
    /// # Arguments
    ///
    /// * `type_id` - The decoration type to query
    ///
    /// # Returns
    ///
    /// The range behavior for this type, or None if the type doesn't exist.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let behavior = registry.get_range_behavior(type_id)?;
    /// let (start_bias, end_bias) = behavior.to_bias();
    /// let start = buffer_snapshot.anchor_at(start_point, start_bias);
    /// let end = buffer_snapshot.anchor_at(end_point, end_bias);
    /// ```
    pub fn get_range_behavior(&self, type_id: DecorationTypeId) -> Option<DecorationRangeBehavior> {
        let inner = self.inner.read().expect("registry lock poisoned");
        inner.decoration_types.get(&type_id).map(|type_data| type_data.options.range_behavior)
    }

    /// Clear all decorations for an editor.
    ///
    /// This is typically called when an editor is closed to free up resources.
    /// It updates reference counts for all decoration types used by this editor.
    ///
    /// # Arguments
    ///
    /// * `editor_id` - The editor to clear decorations for
    ///
    /// # Returns
    ///
    /// The number of decorations that were removed.
    pub fn clear_editor(&self, editor_id: EntityId) -> usize {
        let mut inner = self.inner.write().expect("registry lock poisoned");

        if let Some(editor_decorations) = inner.editor_decorations.remove(&editor_id) {
            let decoration_count = editor_decorations.decorations.len();

            // Update reference counts for all types used by this editor
            for (type_id, decoration_ids) in editor_decorations.decorations_by_type {
                if let Some(type_data) = inner.decoration_types.get_mut(&type_id) {
                    type_data.reference_count = type_data.reference_count
                        .saturating_sub(decoration_ids.len());
                }
            }

            decoration_count
        } else {
            0
        }
    }

    /// Get statistics about the registry state.
    ///
    /// Useful for debugging and monitoring memory usage.
    pub fn stats(&self) -> DecorationRegistryStats {
        let inner = self.inner.read().expect("registry lock poisoned");
        DecorationRegistryStats {
            decoration_type_count: inner.decoration_types.len(),
            editor_count: inner.editor_decorations.len(),
            total_decoration_count: inner.editor_decorations
                .values()
                .map(|ed| ed.decorations.len())
                .sum(),
        }
    }
}

impl Default for DecorationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DecorationRegistry {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// Statistics about the decoration registry state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecorationRegistryStats {
    /// Number of decoration types registered.
    pub decoration_type_count: usize,

    /// Number of editors with decorations.
    pub editor_count: usize,

    /// Total number of decoration instances across all editors.
    pub total_decoration_count: usize,
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    fn create_test_type() -> DecorationRenderOptions {
        DecorationRenderOptionsBuilder::before()
            .with_text("test")
            .build()
    }

    fn create_test_anchor(_offset: usize) -> Anchor {
        // Create a simple anchor for testing
        // In real usage, these would come from the buffer
        Anchor::min()
    }

    #[test]
    fn test_create_decoration_type() {
        let registry = DecorationRegistry::new();
        let options = create_test_type();

        let type_id = registry.create_decoration_type(options.clone());

        assert_eq!(type_id, DecorationTypeId(1));

        let retrieved = registry.get_decoration_type(type_id);
        assert_eq!(retrieved, Some(options));
    }

    #[test]
    fn test_multiple_decoration_types() {
        let registry = DecorationRegistry::new();

        let type_id1 = registry.create_decoration_type(create_test_type());
        let type_id2 = registry.create_decoration_type(create_test_type());
        let type_id3 = registry.create_decoration_type(create_test_type());

        assert_eq!(type_id1, DecorationTypeId(1));
        assert_eq!(type_id2, DecorationTypeId(2));
        assert_eq!(type_id3, DecorationTypeId(3));

        assert!(registry.get_decoration_type(type_id1).is_some());
        assert!(registry.get_decoration_type(type_id2).is_some());
        assert!(registry.get_decoration_type(type_id3).is_some());
    }

    #[test]
    fn test_get_nonexistent_type() {
        let registry = DecorationRegistry::new();
        let result = registry.get_decoration_type(DecorationTypeId(999));
        assert!(result.is_none());
    }

    #[test]
    fn test_set_and_get_decorations() {
        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(create_test_type());
        let editor_id = EntityId::from(1);

        let decorations = vec![
            Decoration::point(DecorationId(1), type_id, create_test_anchor(0)),
            Decoration::point(DecorationId(2), type_id, create_test_anchor(10)),
        ];

        registry.set_decorations(editor_id, type_id, decorations.clone());

        let retrieved = registry.get_decorations(editor_id);
        assert_eq!(retrieved.len(), 2);
        assert!(retrieved.iter().any(|d| *d == &decorations[0]));
        assert!(retrieved.iter().any(|d| *d == &decorations[1]));
    }

    #[test]
    fn test_get_decorations_for_type() {
        let registry = DecorationRegistry::new();
        let type_id1 = registry.create_decoration_type(create_test_type());
        let type_id2 = registry.create_decoration_type(create_test_type());
        let editor_id = EntityId::from(1);

        let decorations1 = vec![
            Decoration::point(DecorationId(1), type_id1, create_test_anchor(0)),
            Decoration::point(DecorationId(2), type_id1, create_test_anchor(10)),
        ];

        let decorations2 = vec![
            Decoration::point(DecorationId(3), type_id2, create_test_anchor(20)),
        ];

        registry.set_decorations(editor_id, type_id1, decorations1.clone());
        registry.set_decorations(editor_id, type_id2, decorations2.clone());

        let retrieved1 = registry.get_decorations_for_type(editor_id, type_id1);
        assert_eq!(retrieved1.len(), 2);

        let retrieved2 = registry.get_decorations_for_type(editor_id, type_id2);
        assert_eq!(retrieved2.len(), 1);

        // Check total decorations
        let all = registry.get_decorations(editor_id);
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_replace_decorations_of_type() {
        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(create_test_type());
        let editor_id = EntityId::from(1);

        // Set initial decorations
        let decorations1 = vec![
            Decoration::point(DecorationId(1), type_id, create_test_anchor(0)),
            Decoration::point(DecorationId(2), type_id, create_test_anchor(10)),
        ];
        registry.set_decorations(editor_id, type_id, decorations1);

        // Replace with new decorations
        let decorations2 = vec![
            Decoration::point(DecorationId(3), type_id, create_test_anchor(20)),
        ];
        registry.set_decorations(editor_id, type_id, decorations2.clone());

        let retrieved = registry.get_decorations_for_type(editor_id, type_id);
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].id, DecorationId(3));
    }

    #[test]
    fn test_multiple_editors_isolated() {
        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(create_test_type());
        let editor_id1 = EntityId::from(1);
        let editor_id2 = EntityId::from(2);

        let decorations1 = vec![
            Decoration::point(DecorationId(1), type_id, create_test_anchor(0)),
        ];

        let decorations2 = vec![
            Decoration::point(DecorationId(2), type_id, create_test_anchor(10)),
            Decoration::point(DecorationId(3), type_id, create_test_anchor(20)),
        ];

        registry.set_decorations(editor_id1, type_id, decorations1);
        registry.set_decorations(editor_id2, type_id, decorations2);

        let retrieved1 = registry.get_decorations(editor_id1);
        let retrieved2 = registry.get_decorations(editor_id2);

        assert_eq!(retrieved1.len(), 1);
        assert_eq!(retrieved2.len(), 2);
    }

    #[test]
    fn test_clear_editor() {
        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(create_test_type());
        let editor_id = EntityId::from(1);

        let decorations = vec![
            Decoration::point(DecorationId(1), type_id, create_test_anchor(0)),
            Decoration::point(DecorationId(2), type_id, create_test_anchor(10)),
        ];

        registry.set_decorations(editor_id, type_id, decorations);

        let removed_count = registry.clear_editor(editor_id);
        assert_eq!(removed_count, 2);

        let retrieved = registry.get_decorations(editor_id);
        assert_eq!(retrieved.len(), 0);
    }

    #[test]
    fn test_clear_nonexistent_editor() {
        let registry = DecorationRegistry::new();
        let editor_id = EntityId::from(999);

        let removed_count = registry.clear_editor(editor_id);
        assert_eq!(removed_count, 0);
    }

    #[test]
    fn test_dispose_decoration_type() {
        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(create_test_type());
        let editor_id = EntityId::from(1);

        let decorations = vec![
            Decoration::point(DecorationId(1), type_id, create_test_anchor(0)),
        ];

        registry.set_decorations(editor_id, type_id, decorations);

        let disposed = registry.dispose_decoration_type(type_id);
        assert!(disposed);

        // Type should be gone
        assert!(registry.get_decoration_type(type_id).is_none());

        // Decorations should be gone
        let retrieved = registry.get_decorations(editor_id);
        assert_eq!(retrieved.len(), 0);
    }

    #[test]
    fn test_dispose_nonexistent_type() {
        let registry = DecorationRegistry::new();
        let disposed = registry.dispose_decoration_type(DecorationTypeId(999));
        assert!(!disposed);
    }

    #[test]
    fn test_dispose_type_affects_all_editors() {
        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(create_test_type());
        let editor_id1 = EntityId::from(1);
        let editor_id2 = EntityId::from(2);

        registry.set_decorations(
            editor_id1,
            type_id,
            vec![Decoration::point(DecorationId(1), type_id, create_test_anchor(0))],
        );

        registry.set_decorations(
            editor_id2,
            type_id,
            vec![Decoration::point(DecorationId(2), type_id, create_test_anchor(10))],
        );

        registry.dispose_decoration_type(type_id);

        assert_eq!(registry.get_decorations(editor_id1).len(), 0);
        assert_eq!(registry.get_decorations(editor_id2).len(), 0);
    }

    #[test]
    fn test_reference_counting() {
        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(create_test_type());
        let editor_id = EntityId::from(1);

        // Add decorations
        registry.set_decorations(
            editor_id,
            type_id,
            vec![
                Decoration::point(DecorationId(1), type_id, create_test_anchor(0)),
                Decoration::point(DecorationId(2), type_id, create_test_anchor(10)),
            ],
        );

        // Check reference count via internal state
        let inner = registry.inner.read().expect("lock poisoned");
        let type_data = inner.decoration_types.get(&type_id).unwrap();
        assert_eq!(type_data.reference_count, 2);
        drop(inner);

        // Replace with fewer decorations
        registry.set_decorations(
            editor_id,
            type_id,
            vec![Decoration::point(DecorationId(3), type_id, create_test_anchor(20))],
        );

        let inner = registry.inner.read().expect("lock poisoned");
        let type_data = inner.decoration_types.get(&type_id).unwrap();
        assert_eq!(type_data.reference_count, 1);
    }

    #[test]
    fn test_stats() {
        let registry = DecorationRegistry::new();
        let type_id1 = registry.create_decoration_type(create_test_type());
        let type_id2 = registry.create_decoration_type(create_test_type());
        let editor_id1 = EntityId::from(1);
        let editor_id2 = EntityId::from(2);

        registry.set_decorations(
            editor_id1,
            type_id1,
            vec![
                Decoration::point(DecorationId(1), type_id1, create_test_anchor(0)),
                Decoration::point(DecorationId(2), type_id1, create_test_anchor(10)),
            ],
        );

        registry.set_decorations(
            editor_id2,
            type_id2,
            vec![Decoration::point(DecorationId(3), type_id2, create_test_anchor(20))],
        );

        let stats = registry.stats();
        assert_eq!(stats.decoration_type_count, 2);
        assert_eq!(stats.editor_count, 2);
        assert_eq!(stats.total_decoration_count, 3);
    }

    #[test]
    fn test_empty_decoration_set() {
        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(create_test_type());
        let editor_id = EntityId::from(1);

        // Set empty decorations
        registry.set_decorations(editor_id, type_id, vec![]);

        let retrieved = registry.get_decorations_for_type(editor_id, type_id);
        assert_eq!(retrieved.len(), 0);

        let all = registry.get_decorations(editor_id);
        assert_eq!(all.len(), 0);
    }

    #[test]
    fn test_range_decorations() {
        let registry = DecorationRegistry::new();
        let options = DecorationRenderOptionsBuilder::range()
            .with_background_color(Hsla::red())
            .build();
        let type_id = registry.create_decoration_type(options);
        let editor_id = EntityId::from(1);

        let decorations = vec![Decoration::range(
            DecorationId(1),
            type_id,
            create_test_anchor(0),
            create_test_anchor(10),
        )];

        registry.set_decorations(editor_id, type_id, decorations.clone());

        let retrieved = registry.get_decorations(editor_id);
        assert_eq!(retrieved.len(), 1);
        assert!(retrieved[0].is_range());
    }

    #[test]
    fn test_mixed_decoration_types() {
        let registry = DecorationRegistry::new();
        let type_id1 = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::before().with_text("A").build(),
        );
        let type_id2 = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::range().build(),
        );
        let editor_id = EntityId::from(1);

        registry.set_decorations(
            editor_id,
            type_id1,
            vec![Decoration::point(DecorationId(1), type_id1, create_test_anchor(0))],
        );

        registry.set_decorations(
            editor_id,
            type_id2,
            vec![Decoration::range(
                DecorationId(2),
                type_id2,
                create_test_anchor(0),
                create_test_anchor(10),
            )],
        );

        let all = registry.get_decorations(editor_id);
        assert_eq!(all.len(), 2);

        let point_decorations = registry.get_decorations_for_type(editor_id, type_id1);
        assert_eq!(point_decorations.len(), 1);
        assert!(point_decorations[0].is_point());

        let range_decorations = registry.get_decorations_for_type(editor_id, type_id2);
        assert_eq!(range_decorations.len(), 1);
        assert!(range_decorations[0].is_range());
    }

    #[test]
    fn test_clone_registry() {
        let registry1 = DecorationRegistry::new();
        let type_id = registry1.create_decoration_type(create_test_type());

        let registry2 = registry1.clone();

        // Both registries share the same underlying data
        let options1 = registry1.get_decoration_type(type_id);
        let options2 = registry2.get_decoration_type(type_id);
        assert_eq!(options1, options2);

        // Changes in one are visible in the other
        let editor_id = EntityId::from(1);
        registry1.set_decorations(
            editor_id,
            type_id,
            vec![Decoration::point(DecorationId(1), type_id, create_test_anchor(0))],
        );

        let decorations = registry2.get_decorations(editor_id);
        assert_eq!(decorations.len(), 1);
    }

    #[test]
    fn test_default_trait() {
        let registry = DecorationRegistry::default();
        let stats = registry.stats();
        assert_eq!(stats.decoration_type_count, 0);
        assert_eq!(stats.editor_count, 0);
        assert_eq!(stats.total_decoration_count, 0);
    }

    #[test]
    #[should_panic(expected = "decoration type must exist")]
    fn test_set_decorations_invalid_type() {
        let registry = DecorationRegistry::new();
        let editor_id = EntityId::from(1);
        let invalid_type_id = DecorationTypeId(999);

        registry.set_decorations(
            editor_id,
            invalid_type_id,
            vec![Decoration::point(
                DecorationId(1),
                invalid_type_id,
                create_test_anchor(0),
            )],
        );
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(create_test_type());

        // Clone registry for multiple threads
        let registry1 = registry.clone();
        let registry2 = registry.clone();

        let handle1 = thread::spawn(move || {
            for i in 0..10 {
                let editor_id = EntityId::from(i);
                registry1.set_decorations(
                    editor_id,
                    type_id,
                    vec![Decoration::point(DecorationId(i), type_id, create_test_anchor(0))],
                );
            }
        });

        let handle2 = thread::spawn(move || {
            for i in 10..20 {
                let editor_id = EntityId::from(i);
                registry2.set_decorations(
                    editor_id,
                    type_id,
                    vec![Decoration::point(DecorationId(i), type_id, create_test_anchor(0))],
                );
            }
        });

        handle1.join().expect("thread 1 panicked");
        handle2.join().expect("thread 2 panicked");

        let stats = registry.stats();
        assert_eq!(stats.editor_count, 20);
        assert_eq!(stats.total_decoration_count, 20);
    }

    #[test]
    fn test_large_number_of_decorations() {
        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(create_test_type());
        let editor_id = EntityId::from(1);

        // Create 1000 decorations
        let decorations: Vec<_> = (0..1000)
            .map(|i| Decoration::point(DecorationId(i), type_id, create_test_anchor(i)))
            .collect();

        registry.set_decorations(editor_id, type_id, decorations);

        let retrieved = registry.get_decorations(editor_id);
        assert_eq!(retrieved.len(), 1000);

        let stats = registry.stats();
        assert_eq!(stats.total_decoration_count, 1000);
    }

    #[test]
    fn test_cleanup_memory() {
        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(create_test_type());
        let editor_id = EntityId::from(1);

        // Add many decorations
        let decorations: Vec<_> = (0..100)
            .map(|i| Decoration::point(DecorationId(i), type_id, create_test_anchor(i)))
            .collect();
        registry.set_decorations(editor_id, type_id, decorations);

        // Clear editor
        registry.clear_editor(editor_id);

        let stats = registry.stats();
        assert_eq!(stats.editor_count, 0);
        assert_eq!(stats.total_decoration_count, 0);

        // Type should still exist
        assert!(registry.get_decoration_type(type_id).is_some());
    }
}

#[cfg(all(test, feature = "test-support"))]
mod integration_tests {
    use super::*;
    use crate::Editor;
    use gpui::{TestAppContext, Entity};
    use language::Buffer;
    use multi_buffer::MultiBuffer;
    use text::ToPoint;

    #[gpui::test]
    async fn test_decoration_with_real_buffer_anchor(cx: &mut TestAppContext) {
        // Test creating a decoration with a real buffer anchor from a real Editor
        let buffer = cx.new(|cx| Buffer::local("Hello, World!", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..13].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let (editor_id, anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            // Create an anchor at position 7 (the 'W' in "World")
            let point = buffer_snapshot.offset_to_point(7);
            let anchor = buffer_snapshot.anchor_at(point, text::Bias::Left);
            (cx.entity_id(), anchor)
        });

        // Create decoration registry and type
        let registry = DecorationRegistry::new();
        let hat_type = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::before()
                .with_text("^")
                .build()
        );

        // Add decoration at the anchor
        let decoration = Decoration::point(DecorationId(1), hat_type, anchor);
        registry.set_decorations(editor_id, hat_type, vec![decoration]);

        // Verify decoration was added
        let decorations = registry.get_decorations(editor_id);
        assert_eq!(decorations.len(), 1);
        assert_eq!(decorations[0].id, DecorationId(1));
        assert!(decorations[0].is_point());

        // Verify anchor is at the correct position
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let dec_point = decorations[0].start.to_point(&buffer_snapshot);
            assert_eq!(dec_point, 0.point(7));
        });
    }

    #[gpui::test]
    async fn test_decoration_survives_buffer_edits(cx: &mut TestAppContext) {
        // Test that decoration anchors correctly track through buffer edits
        let buffer = cx.new(|cx| Buffer::local("abcdefghij", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..10].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        // Create anchor at position 5 ('f')
        let (editor_id, anchor_before_edit) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let point = buffer_snapshot.offset_to_point(5);
            let anchor = buffer_snapshot.anchor_at(point, sum_tree::Bias::Left);
            (cx.entity_id(), anchor)
        });

        // Create decoration registry and add decoration
        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::range()
                .with_background_color(Hsla::red())
                .build()
        );

        let decoration = Decoration::point(DecorationId(1), type_id, anchor_before_edit);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Insert text at position 2 (before the anchor)
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([2..2]);
            });
            editor.insert("XXX", window, cx);
        });

        // Verify anchor moved correctly
        let decorations = registry.get_decorations(editor_id);
        assert_eq!(decorations.len(), 1);

        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let dec_point = decorations[0].start.to_point(&buffer_snapshot);
            // Anchor should now be at position 8 (original 5 + 3 inserted chars)
            assert_eq!(dec_point, 0.point(8));

            // Verify buffer content
            let text = buffer_snapshot.text();
            assert_eq!(text, "abXXXcdefghij");
        });
    }

    #[gpui::test]
    async fn test_decoration_insertion_after(cx: &mut TestAppContext) {
        // Test that decorations stay stable when inserting text after them
        let buffer = cx.new(|cx| Buffer::local("abcdefghij", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..10].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let (editor_id, anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let point = buffer_snapshot.offset_to_point(3);
            let anchor = buffer_snapshot.anchor_at(point, sum_tree::Bias::Left);
            (cx.entity_id(), anchor)
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::before()
                .with_text("^")
                .build()
        );

        let decoration = Decoration::point(DecorationId(1), type_id, anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Insert text at position 7 (after the anchor)
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([7..7]);
            });
            editor.insert("YYY", window, cx);
        });

        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let dec_point = decorations[0].start.to_point(&buffer_snapshot);
            // Anchor should stay at position 3
            assert_eq!(dec_point, 0.point(3));
            assert_eq!(buffer_snapshot.text(), "abcdefgYYYhij");
        });
    }

    #[gpui::test]
    async fn test_decoration_deletion_before(cx: &mut TestAppContext) {
        // Test decoration tracking when text before it is deleted
        let buffer = cx.new(|cx| Buffer::local("abcdefghij", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..10].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let (editor_id, anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let point = buffer_snapshot.offset_to_point(7);
            let anchor = buffer_snapshot.anchor_at(point, sum_tree::Bias::Left);
            (cx.entity_id(), anchor)
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::before()
                .with_text("!")
                .build()
        );

        let decoration = Decoration::point(DecorationId(1), type_id, anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Delete text from positions 2-5 (before the anchor)
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([2..5]);
            });
            editor.delete(window, cx);
        });

        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let dec_point = decorations[0].start.to_point(&buffer_snapshot);
            // Anchor should move back by 3 (deleted chars): 7 - 3 = 4
            assert_eq!(dec_point, 0.point(4));
            assert_eq!(buffer_snapshot.text(), "abfghij");
        });
    }

    #[gpui::test]
    async fn test_decoration_range_tracking(cx: &mut TestAppContext) {
        // Test that range decorations track correctly through edits
        let buffer = cx.new(|cx| Buffer::local("line1\nline2\nline3\n", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..18].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::range()
                .with_background_color(Hsla::blue())
                .build()
        );

        // Create range decoration spanning "line2" (positions 6-11)
        // Use the range behavior from the decoration type to determine anchor bias
        let (editor_id, start_anchor, end_anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let behavior = registry.get_range_behavior(type_id).unwrap();
            let (start_bias, end_bias) = behavior.to_bias();
            let start = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(6), start_bias);
            let end = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(11), end_bias);
            (cx.entity_id(), start, end)
        });

        let decoration = Decoration::range(DecorationId(1), type_id, start_anchor, end_anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Insert text before the range
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([0..0]);
            });
            editor.insert("PREFIX", window, cx);
        });

        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_point = decorations[0].start.to_point(&buffer_snapshot);
            let end_point = decorations[0].end.as_ref().unwrap().to_point(&buffer_snapshot);

            // Both anchors should have moved by 6 positions
            assert_eq!(start_point, 0.point(12)); // 6 + 6
            assert_eq!(end_point, 0.point(17));   // 11 + 6
            assert_eq!(buffer_snapshot.text(), "PREFIXline1\nline2\nline3\n");
        });
    }

    #[gpui::test]
    async fn test_decoration_multi_cursor_edits(cx: &mut TestAppContext) {
        // Test decorations with multiple cursors editing simultaneously
        let buffer = cx.new(|cx| Buffer::local("aaa\nbbb\nccc\n", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..12].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        // Place decorations at the start of each line (0, 4, 8)
        let (editor_id, anchors) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let anchors = vec![
                buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(0), sum_tree::Bias::Left),
                buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(4), sum_tree::Bias::Left),
                buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(8), sum_tree::Bias::Left),
            ];
            (cx.entity_id(), anchors)
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::before()
                .with_text(">")
                .build()
        );

        let decorations = anchors.iter().enumerate()
            .map(|(i, anchor)| Decoration::point(DecorationId(i), type_id, *anchor))
            .collect();
        registry.set_decorations(editor_id, type_id, decorations);

        // Insert "X" at position 2 (in first line)
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([2..2]);
            });
            editor.insert("X", window, cx);
        });

        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);

            // First decoration stays at 0
            assert_eq!(decorations[0].start.to_point(&buffer_snapshot), 0.point(0));
            // Second decoration moves from 4 to 5
            assert_eq!(decorations[1].start.to_point(&buffer_snapshot), 0.point(5));
            // Third decoration moves from 8 to 9
            assert_eq!(decorations[2].start.to_point(&buffer_snapshot), 0.point(9));

            assert_eq!(buffer_snapshot.text(), "aaXa\nbbb\nccc\n");
        });
    }

    #[gpui::test]
    async fn test_decoration_undo_redo(cx: &mut TestAppContext) {
        // Test that decorations track correctly through undo/redo
        let buffer = cx.new(|cx| Buffer::local("original", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..8].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let (editor_id, anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let point = buffer_snapshot.offset_to_point(4);
            let anchor = buffer_snapshot.anchor_at(point, sum_tree::Bias::Left);
            (cx.entity_id(), anchor)
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::before()
                .with_text("|")
                .build()
        );

        let decoration = Decoration::point(DecorationId(1), type_id, anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Insert text
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([0..0]);
            });
            editor.insert("START_", window, cx);
        });

        let decorations = registry.get_decorations(editor_id);

        // Verify decoration moved
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            assert_eq!(decorations[0].start.to_point(&buffer_snapshot), 0.point(10));
        });

        // Undo
        editor.update(cx, |editor, window, cx| {
            editor.undo(window, cx);
        });

        // Decoration should be back at original position
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            assert_eq!(decorations[0].start.to_point(&buffer_snapshot), 0.point(4));
            assert_eq!(buffer_snapshot.text(), "original");
        });

        // Redo
        editor.update(cx, |editor, window, cx| {
            editor.redo(window, cx);
        });

        // Decoration should move forward again
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            assert_eq!(decorations[0].start.to_point(&buffer_snapshot), 0.point(10));
            assert_eq!(buffer_snapshot.text(), "START_original");
        });
    }

    #[gpui::test]
    async fn test_decoration_bias_behavior(cx: &mut TestAppContext) {
        // Test that Left bias doesn't expand when inserting at the anchor position
        let buffer = cx.new(|cx| Buffer::local("abcdef", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..6].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let (editor_id, left_anchor, right_anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let point = buffer_snapshot.offset_to_point(3);
            let left = buffer_snapshot.anchor_at(point, sum_tree::Bias::Left);
            let right = buffer_snapshot.anchor_at(point, sum_tree::Bias::Right);
            (cx.entity_id(), left, right)
        });

        let registry = DecorationRegistry::new();
        let type_id_left = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::before()
                .with_text("L")
                .build()
        );
        let type_id_right = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::before()
                .with_text("R")
                .build()
        );

        registry.set_decorations(
            editor_id,
            type_id_left,
            vec![Decoration::point(DecorationId(1), type_id_left, left_anchor)]
        );
        registry.set_decorations(
            editor_id,
            type_id_right,
            vec![Decoration::point(DecorationId(2), type_id_right, right_anchor)]
        );

        // Insert at position 3 (exactly at the anchor position)
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([3..3]);
            });
            editor.insert("XXX", window, cx);
        });

        let left_decorations = registry.get_decorations_for_type(editor_id, type_id_left);
        let right_decorations = registry.get_decorations_for_type(editor_id, type_id_right);

        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);

            // Left bias: anchor stays before the inserted text
            assert_eq!(left_decorations[0].start.to_point(&buffer_snapshot), 0.point(3));

            // Right bias: anchor moves after the inserted text
            assert_eq!(right_decorations[0].start.to_point(&buffer_snapshot), 0.point(6));

            assert_eq!(buffer_snapshot.text(), "abcXXXdef");
        });
    }

    #[gpui::test]
    async fn test_decoration_complex_edit_sequence(cx: &mut TestAppContext) {
        // Test decorations through a complex sequence of edits
        let buffer = cx.new(|cx| Buffer::local("line1\nline2\nline3\nline4\n", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..24].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let (editor_id, anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            // Place anchor at start of line3 (position 12)
            let point = buffer_snapshot.offset_to_point(12);
            let anchor = buffer_snapshot.anchor_at(point, sum_tree::Bias::Left);
            (cx.entity_id(), anchor)
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::before()
                .with_text("*")
                .build()
        );

        let decoration = Decoration::point(DecorationId(1), type_id, anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Edit 1: Insert at beginning
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([0..0]);
            });
            editor.insert("START\n", window, cx);
        });

        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            // Anchor moved by 6 positions
            assert_eq!(decorations[0].start.to_point(&buffer_snapshot), 0.point(18));
        });

        // Edit 2: Delete a line before the anchor
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([6..12]); // Delete "line1\n"
            });
            editor.delete(window, cx);
        });

        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            // Anchor moved back by 6 positions
            assert_eq!(decorations[0].start.to_point(&buffer_snapshot), 0.point(12));
        });

        // Edit 3: Insert within the line containing the anchor
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([14..14]); // Insert after "li"
            });
            editor.insert("MIDDLE", window, cx);
        });

        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            // Anchor stays at line start
            assert_eq!(decorations[0].start.to_point(&buffer_snapshot), 0.point(12));
            assert_eq!(buffer_snapshot.text(), "START\nline2\nliMIDDLEne3\nline4\n");
        });
    }

    #[gpui::test]
    async fn test_multiple_editors_with_real_entities(cx: &mut TestAppContext) {
        // Test decorations with multiple real editor entities
        let buffer1 = cx.new(|cx| Buffer::local("Editor 1 content", cx));
        let buffer2 = cx.new(|cx| Buffer::local("Editor 2 content", cx));

        let multibuffer1 = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer1,
                [0..16].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let multibuffer2 = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer2,
                [0..16].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor1 = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer1.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let editor2 = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer2.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        // Create anchors for both editors
        let (editor1_id, anchor1) = editor1.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let anchor = buffer_snapshot.anchor_at(0.point(7), text::Bias::Left);
            (cx.entity_id(), anchor)
        });

        let (editor2_id, anchor2) = editor2.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let anchor = buffer_snapshot.anchor_at(0.point(7), text::Bias::Left);
            (cx.entity_id(), anchor)
        });

        // Create shared decoration registry and types
        let registry = DecorationRegistry::new();
        let hat_type = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::before()
                .with_text("^")
                .build()
        );
        let highlight_type = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::range()
                .with_background_color(Hsla::blue())
                .build()
        );

        // Add decorations to both editors
        registry.set_decorations(
            editor1_id,
            hat_type,
            vec![Decoration::point(DecorationId(1), hat_type, anchor1)]
        );

        registry.set_decorations(
            editor2_id,
            highlight_type,
            vec![Decoration::point(DecorationId(2), highlight_type, anchor2)]
        );

        // Verify isolation - editor1 should only have its decorations
        let editor1_decs = registry.get_decorations(editor1_id);
        assert_eq!(editor1_decs.len(), 1);
        assert_eq!(editor1_decs[0].id, DecorationId(1));
        assert_eq!(editor1_decs[0].type_id, hat_type);

        // Verify isolation - editor2 should only have its decorations
        let editor2_decs = registry.get_decorations(editor2_id);
        assert_eq!(editor2_decs.len(), 1);
        assert_eq!(editor2_decs[0].id, DecorationId(2));
        assert_eq!(editor2_decs[0].type_id, highlight_type);

        // Verify total stats
        let stats = registry.stats();
        assert_eq!(stats.editor_count, 2);
        assert_eq!(stats.decoration_type_count, 2);
        assert_eq!(stats.total_decoration_count, 2);
    }

    #[gpui::test]
    async fn test_decoration_deletion_encompasses_point(cx: &mut TestAppContext) {
        // Test that a point decoration handles deletion that encompasses it
        let buffer = cx.new(|cx| Buffer::local("0123456789abcdef", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..16].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let (editor_id, anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let anchor = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(10), text::Bias::Left);
            (cx.entity_id(), anchor)
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::before()
                .with_text("^")
                .build()
        );

        let decoration = Decoration::point(DecorationId(1), type_id, anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Delete range [5..15] that encompasses the decoration at position 10
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([5..15]);
            });
            editor.delete(window, cx);
        });

        // Verify the anchor is still valid and collapsed to deletion boundary
        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let dec_point = decorations[0].start.to_point(&buffer_snapshot);

            // Anchor should collapse to the start of the deletion (position 5)
            assert_eq!(dec_point, 0.point(5));
            assert!(decorations[0].start.is_valid(&buffer_snapshot));
            assert_eq!(buffer_snapshot.text(), "01234abcdef");
        });
    }

    #[gpui::test]
    async fn test_decoration_deletion_encompasses_range_start(cx: &mut TestAppContext) {
        // Test deletion that overlaps the start of a range decoration
        let buffer = cx.new(|cx| Buffer::local("0123456789abcdefghij", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..20].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::range()
                .with_background_color(Hsla::blue())
                .build()
        );

        // Create decoration spanning [10..20]
        let (editor_id, start_anchor, end_anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let behavior = registry.get_range_behavior(type_id).unwrap();
            let (start_bias, end_bias) = behavior.to_bias();
            let start = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(10), start_bias);
            let end = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(20), end_bias);
            (cx.entity_id(), start, end)
        });

        let decoration = Decoration::range(DecorationId(1), type_id, start_anchor, end_anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Delete range [5..15] that overlaps the start of the decoration
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([5..15]);
            });
            editor.delete(window, cx);
        });

        // Verify the decoration adjusts correctly
        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_point = decorations[0].start.to_point(&buffer_snapshot);
            let end_point = decorations[0].end.as_ref().unwrap().to_point(&buffer_snapshot);

            // Start should collapse to deletion boundary (5), end should shift back by 10
            assert_eq!(start_point, 0.point(5));
            assert_eq!(end_point, 0.point(10)); // 20 - 10 = 10
            assert!(decorations[0].start.is_valid(&buffer_snapshot));
            assert!(decorations[0].end.as_ref().unwrap().is_valid(&buffer_snapshot));
            assert_eq!(buffer_snapshot.text(), "01234abcdefghij");
        });
    }

    #[gpui::test]
    async fn test_decoration_deletion_encompasses_range_end(cx: &mut TestAppContext) {
        // Test deletion that overlaps the end of a range decoration
        let buffer = cx.new(|cx| Buffer::local("0123456789abcdefghij", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..20].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::range()
                .with_background_color(Hsla::blue())
                .build()
        );

        // Create decoration spanning [5..15]
        let (editor_id, start_anchor, end_anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let behavior = registry.get_range_behavior(type_id).unwrap();
            let (start_bias, end_bias) = behavior.to_bias();
            let start = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(5), start_bias);
            let end = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(15), end_bias);
            (cx.entity_id(), start, end)
        });

        let decoration = Decoration::range(DecorationId(1), type_id, start_anchor, end_anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Delete range [10..20] that overlaps the end of the decoration
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([10..20]);
            });
            editor.delete(window, cx);
        });

        // Verify the decoration adjusts correctly
        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_point = decorations[0].start.to_point(&buffer_snapshot);
            let end_point = decorations[0].end.as_ref().unwrap().to_point(&buffer_snapshot);

            // Start stays at 5, end should collapse to deletion boundary (10)
            assert_eq!(start_point, 0.point(5));
            assert_eq!(end_point, 0.point(10));
            assert!(decorations[0].start.is_valid(&buffer_snapshot));
            assert!(decorations[0].end.as_ref().unwrap().is_valid(&buffer_snapshot));
            assert_eq!(buffer_snapshot.text(), "0123456789");
        });
    }

    #[gpui::test]
    async fn test_decoration_deletion_encompasses_entire_range(cx: &mut TestAppContext) {
        // Test deletion that completely encompasses a range decoration
        let buffer = cx.new(|cx| Buffer::local("0123456789abcdefghij", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..20].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::range()
                .with_background_color(Hsla::blue())
                .build()
        );

        // Create decoration spanning [10..15]
        let (editor_id, start_anchor, end_anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let behavior = registry.get_range_behavior(type_id).unwrap();
            let (start_bias, end_bias) = behavior.to_bias();
            let start = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(10), start_bias);
            let end = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(15), end_bias);
            (cx.entity_id(), start, end)
        });

        let decoration = Decoration::range(DecorationId(1), type_id, start_anchor, end_anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Delete range [5..18] that completely encompasses the decoration
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([5..18]);
            });
            editor.delete(window, cx);
        });

        // Verify both anchors collapse to the deletion boundary
        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_point = decorations[0].start.to_point(&buffer_snapshot);
            let end_point = decorations[0].end.as_ref().unwrap().to_point(&buffer_snapshot);

            // Both anchors should collapse to the start of deletion (position 5)
            assert_eq!(start_point, 0.point(5));
            assert_eq!(end_point, 0.point(5));
            assert!(decorations[0].start.is_valid(&buffer_snapshot));
            assert!(decorations[0].end.as_ref().unwrap().is_valid(&buffer_snapshot));
            assert_eq!(buffer_snapshot.text(), "01234ij");
        });
    }

    #[gpui::test]
    async fn test_range_behavior_closed_closed(cx: &mut TestAppContext) {
        // Test ClosedClosed: both boundaries don't expand when text is inserted at them
        let buffer = cx.new(|cx| Buffer::local("0123456789abcdefghij", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..20].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::range()
                .with_background_color(Hsla::blue())
                .with_range_behavior(DecorationRangeBehavior::ClosedClosed)
                .build()
        );

        // Create range decoration [10..20] with ClosedClosed behavior
        let (editor_id, start_anchor, end_anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let behavior = registry.get_range_behavior(type_id).unwrap();
            let (start_bias, end_bias) = behavior.to_bias();
            let start = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(10), start_bias);
            let end = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(20), end_bias);
            (cx.entity_id(), start, end)
        });

        let decoration = Decoration::range(DecorationId(1), type_id, start_anchor, end_anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Insert text at position 10 (start boundary)
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([10..10]);
            });
            editor.insert("XXX", window, cx);
        });

        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_point = decorations[0].start.to_point(&buffer_snapshot);
            let end_point = decorations[0].end.as_ref().unwrap().to_point(&buffer_snapshot);

            // ClosedClosed: start stays at 10 (doesn't expand to include insertion)
            assert_eq!(start_point, 0.point(10));
            // End moves forward by 3
            assert_eq!(end_point, 0.point(23));
        });

        // Insert text at position 23 (end boundary)
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([23..23]);
            });
            editor.insert("YYY", window, cx);
        });

        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_point = decorations[0].start.to_point(&buffer_snapshot);
            let end_point = decorations[0].end.as_ref().unwrap().to_point(&buffer_snapshot);

            // Start stays at 10
            assert_eq!(start_point, 0.point(10));
            // ClosedClosed: end stays at 23 (doesn't expand to include insertion)
            assert_eq!(end_point, 0.point(23));
            assert_eq!(buffer_snapshot.text(), "0123456789XXXabcdefghijYYY");
        });
    }

    #[gpui::test]
    async fn test_range_behavior_open_open(cx: &mut TestAppContext) {
        // Test OpenOpen: both boundaries expand when text is inserted at them
        let buffer = cx.new(|cx| Buffer::local("0123456789abcdefghij", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..20].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::range()
                .with_background_color(Hsla::blue())
                .with_range_behavior(DecorationRangeBehavior::OpenOpen)
                .build()
        );

        // Create range decoration [10..20] with OpenOpen behavior
        let (editor_id, start_anchor, end_anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let behavior = registry.get_range_behavior(type_id).unwrap();
            let (start_bias, end_bias) = behavior.to_bias();
            let start = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(10), start_bias);
            let end = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(20), end_bias);
            (cx.entity_id(), start, end)
        });

        let decoration = Decoration::range(DecorationId(1), type_id, start_anchor, end_anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Insert text at position 10 (start boundary)
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([10..10]);
            });
            editor.insert("XXX", window, cx);
        });

        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_point = decorations[0].start.to_point(&buffer_snapshot);
            let end_point = decorations[0].end.as_ref().unwrap().to_point(&buffer_snapshot);

            // OpenOpen: start moves to after insertion (expands)
            assert_eq!(start_point, 0.point(13));
            // End moves forward by 3
            assert_eq!(end_point, 0.point(23));
        });

        // Insert text at position 23 (end boundary)
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([23..23]);
            });
            editor.insert("YYY", window, cx);
        });

        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_point = decorations[0].start.to_point(&buffer_snapshot);
            let end_point = decorations[0].end.as_ref().unwrap().to_point(&buffer_snapshot);

            // Start stays at 13
            assert_eq!(start_point, 0.point(13));
            // OpenOpen: end moves to after insertion (expands)
            assert_eq!(end_point, 0.point(26));
            assert_eq!(buffer_snapshot.text(), "0123456789XXXabcdefghijYYY");
        });
    }

    #[gpui::test]
    async fn test_range_behavior_closed_open(cx: &mut TestAppContext) {
        // Test ClosedOpen: start doesn't expand, end does expand
        let buffer = cx.new(|cx| Buffer::local("0123456789abcdefghij", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..20].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::range()
                .with_background_color(Hsla::blue())
                .with_range_behavior(DecorationRangeBehavior::ClosedOpen)
                .build()
        );

        // Create range decoration [10..20] with ClosedOpen behavior
        let (editor_id, start_anchor, end_anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let behavior = registry.get_range_behavior(type_id).unwrap();
            let (start_bias, end_bias) = behavior.to_bias();
            let start = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(10), start_bias);
            let end = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(20), end_bias);
            (cx.entity_id(), start, end)
        });

        let decoration = Decoration::range(DecorationId(1), type_id, start_anchor, end_anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Insert text at position 10 (start boundary)
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([10..10]);
            });
            editor.insert("XXX", window, cx);
        });

        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_point = decorations[0].start.to_point(&buffer_snapshot);
            let end_point = decorations[0].end.as_ref().unwrap().to_point(&buffer_snapshot);

            // ClosedOpen: start stays at 10 (doesn't expand)
            assert_eq!(start_point, 0.point(10));
            // End moves forward by 3
            assert_eq!(end_point, 0.point(23));
        });

        // Insert text at position 23 (end boundary)
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([23..23]);
            });
            editor.insert("YYY", window, cx);
        });

        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_point = decorations[0].start.to_point(&buffer_snapshot);
            let end_point = decorations[0].end.as_ref().unwrap().to_point(&buffer_snapshot);

            // Start stays at 10
            assert_eq!(start_point, 0.point(10));
            // ClosedOpen: end moves to after insertion (expands)
            assert_eq!(end_point, 0.point(26));
            assert_eq!(buffer_snapshot.text(), "0123456789XXXabcdefghijYYY");
        });
    }

    #[gpui::test]
    async fn test_range_behavior_open_closed(cx: &mut TestAppContext) {
        // Test OpenClosed: start expands, end doesn't expand
        let buffer = cx.new(|cx| Buffer::local("0123456789abcdefghij", cx));
        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                buffer.clone(),
                [0..20].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::range()
                .with_background_color(Hsla::blue())
                .with_range_behavior(DecorationRangeBehavior::OpenClosed)
                .build()
        );

        // Create range decoration [10..20] with OpenClosed behavior
        let (editor_id, start_anchor, end_anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let behavior = registry.get_range_behavior(type_id).unwrap();
            let (start_bias, end_bias) = behavior.to_bias();
            let start = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(10), start_bias);
            let end = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(20), end_bias);
            (cx.entity_id(), start, end)
        });

        let decoration = Decoration::range(DecorationId(1), type_id, start_anchor, end_anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Insert text at position 10 (start boundary)
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([10..10]);
            });
            editor.insert("XXX", window, cx);
        });

        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_point = decorations[0].start.to_point(&buffer_snapshot);
            let end_point = decorations[0].end.as_ref().unwrap().to_point(&buffer_snapshot);

            // OpenClosed: start moves to after insertion (expands)
            assert_eq!(start_point, 0.point(13));
            // End moves forward by 3
            assert_eq!(end_point, 0.point(23));
        });

        // Insert text at position 23 (end boundary)
        editor.update(cx, |editor, window, cx| {
            editor.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([23..23]);
            });
            editor.insert("YYY", window, cx);
        });

        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_point = decorations[0].start.to_point(&buffer_snapshot);
            let end_point = decorations[0].end.as_ref().unwrap().to_point(&buffer_snapshot);

            // Start stays at 13
            assert_eq!(start_point, 0.point(13));
            // OpenClosed: end stays at 23 (doesn't expand)
            assert_eq!(end_point, 0.point(23));
            assert_eq!(buffer_snapshot.text(), "0123456789XXXabcdefghijYYY");
        });
    }

    #[gpui::test]
    async fn test_decoration_concurrent_insertions(cx: &mut TestAppContext) {
        // Test decorations with concurrent insertions from multiple replicas
        use language::BufferId;
        use text::Buffer as TextBuffer;
        use clock::ReplicaId;

        // Create two separate text buffers with different replica IDs
        let mut buffer1 = TextBuffer::new(ReplicaId::new(1), BufferId::new(1).unwrap(), "0123456789abcdefghij");
        let mut buffer2 = TextBuffer::new(ReplicaId::new(2), BufferId::new(1).unwrap(), "0123456789abcdefghij");

        // Create decoration anchor at position 10 in buffer1
        let decoration_anchor = buffer1.anchor_before(10);

        // Simulate concurrent edits:
        // Replica 1 inserts at position 5
        let op1 = buffer1.edit([(5..5, "AAA")]);
        assert_eq!(buffer1.text(), "01234AAA56789abcdefghij");

        // Replica 2 inserts at position 15
        let op2 = buffer2.edit([(15..15, "BBB")]);
        assert_eq!(buffer2.text(), "0123456789abcdeBBBfghij");

        // Apply operations to converge the replicas
        buffer1.apply_op(op2.clone());
        buffer2.apply_op(op1);

        // Both buffers should converge to the same state
        assert_eq!(buffer1.text(), buffer2.text());
        assert_eq!(buffer1.text(), "01234AAA56789abcdeBBBfghij");

        // Verify the decoration anchor moved correctly after both edits
        // Original position was 10, after insert at 5 (+3) it's at 13
        // After insert at 15 (which is at 18 after first insert), position stays at 13
        let final_position = decoration_anchor.to_offset(&buffer1);
        assert_eq!(final_position, 13);

        // Verify both buffers agree on anchor position
        assert_eq!(decoration_anchor.to_offset(&buffer1), decoration_anchor.to_offset(&buffer2));
    }

    #[gpui::test]
    async fn test_decoration_concurrent_deletions(cx: &mut TestAppContext) {
        // Test decorations with concurrent deletions from multiple replicas
        use language::BufferId;
        use text::Buffer as TextBuffer;
        use clock::ReplicaId;

        // Create two separate text buffers with different replica IDs
        let mut buffer1 = TextBuffer::new(ReplicaId::new(1), BufferId::new(1).unwrap(), "0123456789abcdefghij");
        let mut buffer2 = TextBuffer::new(ReplicaId::new(2), BufferId::new(1).unwrap(), "0123456789abcdefghij");

        // Create decoration anchor at position 10
        let decoration_anchor = buffer1.anchor_before(10);

        // Simulate concurrent deletions:
        // Replica 1 deletes [3..6] (deletes "345")
        let op1 = buffer1.edit([(3..6, "")]);
        assert_eq!(buffer1.text(), "0126789abcdefghij");

        // Replica 2 deletes [12..15] (deletes "cde")
        let op2 = buffer2.edit([(12..15, "")]);
        assert_eq!(buffer2.text(), "0123456789abfghij");

        // Apply operations to converge the replicas
        buffer1.apply_op(op2.clone());
        buffer2.apply_op(op1);

        // Both buffers should converge to the same state
        assert_eq!(buffer1.text(), buffer2.text());
        assert_eq!(buffer1.text(), "0126789abfghij");

        // Verify the decoration anchor adjusted correctly
        // Original position was 10, after delete [3..6] (-3) it's at 7
        // After delete [12..15] (which is at [9..12] after first delete), position stays at 7
        let final_position = decoration_anchor.to_offset(&buffer1);
        assert_eq!(final_position, 7);

        // Verify both buffers agree on anchor position
        assert_eq!(decoration_anchor.to_offset(&buffer1), decoration_anchor.to_offset(&buffer2));
        assert!(decoration_anchor.is_valid(&buffer1));
    }

    #[gpui::test]
    async fn test_decoration_concurrent_mixed_edits(cx: &mut TestAppContext) {
        // Test decorations with mixed concurrent insertions and deletions
        use language::BufferId;
        use text::Buffer as TextBuffer;
        use clock::ReplicaId;

        // Create three replicas for a more complex scenario
        let mut buffer1 = TextBuffer::new(ReplicaId::new(1), BufferId::new(1).unwrap(), "abcdefghijklmnopqrstuvwxyz");
        let mut buffer2 = TextBuffer::new(ReplicaId::new(2), BufferId::new(1).unwrap(), "abcdefghijklmnopqrstuvwxyz");
        let mut buffer3 = TextBuffer::new(ReplicaId::new(3), BufferId::new(1).unwrap(), "abcdefghijklmnopqrstuvwxyz");

        // Create decoration anchors at different positions
        let anchor1 = buffer1.anchor_before(5);   // position 5
        let anchor2 = buffer1.anchor_before(15);  // position 15
        let anchor3 = buffer1.anchor_before(20);  // position 20

        // Concurrent edits from three replicas:
        // Replica 1: Insert at position 3
        let op1 = buffer1.edit([(3..3, "XXX")]);
        // Replica 2: Delete at positions [10..13]
        let op2 = buffer2.edit([(10..13, "")]);
        // Replica 3: Insert at position 18
        let op3 = buffer3.edit([(18..18, "YYY")]);

        // Apply all operations to all replicas to converge
        buffer1.apply_op(op2.clone());
        buffer1.apply_op(op3.clone());
        buffer2.apply_op(op1.clone());
        buffer2.apply_op(op3.clone());
        buffer3.apply_op(op1);
        buffer3.apply_op(op2);

        // All buffers should converge
        assert_eq!(buffer1.text(), buffer2.text());
        assert_eq!(buffer2.text(), buffer3.text());

        // Verify anchors maintain correct positions across all replicas
        assert_eq!(anchor1.to_offset(&buffer1), anchor1.to_offset(&buffer2));
        assert_eq!(anchor1.to_offset(&buffer2), anchor1.to_offset(&buffer3));
        assert_eq!(anchor2.to_offset(&buffer1), anchor2.to_offset(&buffer2));
        assert_eq!(anchor2.to_offset(&buffer2), anchor2.to_offset(&buffer3));
        assert_eq!(anchor3.to_offset(&buffer1), anchor3.to_offset(&buffer2));
        assert_eq!(anchor3.to_offset(&buffer2), anchor3.to_offset(&buffer3));

        // All anchors should remain valid
        assert!(anchor1.is_valid(&buffer1));
        assert!(anchor2.is_valid(&buffer1));
        assert!(anchor3.is_valid(&buffer1));
    }

    #[gpui::test]
    async fn test_decoration_crdt_convergence_with_editor(cx: &mut TestAppContext) {
        // Test that decorations maintain correct positions when editor applies
        // operations from collaborative editing
        use language::BufferId;
        use text::Buffer as TextBuffer;
        use clock::ReplicaId;

        // Create a text buffer that will be used in the editor
        let text_buffer = cx.new(|cx| {
            let mut buffer = TextBuffer::new(
                ReplicaId::new(1),
                BufferId::new(1).unwrap(),
                "line1\nline2\nline3\n",
            );
            buffer.into_language_buffer(cx)
        });

        let multibuffer = cx.new(|cx| {
            let mut mb = MultiBuffer::new(language::Capability::ReadWrite);
            mb.push_excerpts(
                text_buffer.clone(),
                [0..18].into_iter().map(multi_buffer::ExcerptRange::new),
                cx,
            );
            mb
        });

        let editor = cx.add_window(|window, cx| {
            let editor = Editor::for_buffer(multibuffer.clone(), None, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
            editor
        });

        let registry = DecorationRegistry::new();
        let type_id = registry.create_decoration_type(
            DecorationRenderOptionsBuilder::before()
                .with_text("^")
                .build()
        );

        // Create decoration at position 6 (start of "line2")
        let (editor_id, anchor) = editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let anchor = buffer_snapshot.anchor_at(buffer_snapshot.offset_to_point(6), text::Bias::Left);
            (cx.entity_id(), anchor)
        });

        let decoration = Decoration::point(DecorationId(1), type_id, anchor);
        registry.set_decorations(editor_id, type_id, vec![decoration]);

        // Simulate a remote edit that was applied to the underlying buffer
        // This represents what would happen in collaborative editing
        text_buffer.update(cx, |buffer, cx| {
            buffer.edit([(0..0, "PREFIX\n")], None, cx);
        });

        // Verify the decoration anchor adjusted correctly
        let decorations = registry.get_decorations(editor_id);
        editor.update(cx, |editor, cx| {
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);
            let dec_point = decorations[0].start.to_point(&buffer_snapshot);
            // Anchor should move from position 6 to position 13 (6 + 7 chars)
            assert_eq!(dec_point, 0.point(13));
            assert_eq!(buffer_snapshot.text(), "PREFIX\nline1\nline2\nline3\n");
        });
    }
}
