//! Decoration API for rendering overlays and highlights in the editor.

pub use crate::wit::zed::extension::decoration::{
    Color, Decoration, DecorationContent, DecorationImage, DecorationRangeBehavior,
    DecorationRenderOptions, DecorationStyle, DecorationSvg, DecorationType,
    ThemedDecorationStyle, create_decoration_type, dispose_decoration_type, set_decorations,
};
