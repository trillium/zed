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
