# Decoration WASM API Implementation Plan

## Overview
This document outlines the implementation plan for exposing Zed's decoration API to extensions via WASM bindings.

## Context
We have successfully implemented the core decoration API in Zed:
- Type system in `/home/trillium/code/zed/crates/editor/src/decorations.rs`
- Rendering integration in `/home/trillium/code/zed/crates/editor/src/element.rs`
- Public API methods on Editor struct in `/home/trillium/code/zed/crates/editor/src/editor.rs`

Now we need to expose this to extensions via WASM bindings.

## Work Completed

### 1. WIT Interface Definition (`/home/trillium/code/zed/crates/extension_api/wit/since_v0.8.0/decoration.wit`)

Created a comprehensive WIT interface that defines:
- `decoration-type` enum (before, after, range, whole-line)
- `decoration-range-behavior` enum (open-open, open-closed, closed-open, closed-closed)
- `decoration-content` variant (text, svg, image)
- `color` record (HSLA format)
- `decoration-style` record (background-color, borders, margin, z-index)
- `themed-decoration-style` record (base, light, dark styles)
- `decoration-render-options` record (complete decoration specification)
- Three main functions:
  - `create-decoration-type(options) -> decoration-type-id`
  - `set-decorations(type-id, decorations) -> list<decoration-id>`
  - `dispose-decoration-type(type-id) -> bool`

### 2. Extension World Update (`/home/trillium/code/zed/crates/extension_api/wit/since_v0.8.0/extension.wit`)

Added `import decoration;` to the extension world to make the decoration interface available to extensions.

## Critical Design Decision Needed

The current implementation has a significant architectural question that must be resolved:

**How do extensions target specific editors/buffers for decorations?**

Options:
1. **Global Active Editor**: Extensions can only set decorations on the currently active editor (simplest)
2. **Editor Resource Parameter**: Add an `editor` resource parameter to decoration functions
3. **Buffer ID Parameter**: Extensions specify buffer ID to target specific buffers
4. **Extension Context**: Extensions maintain their own editor context via a separate API

### Recommended Approach: Global Active Editor

For the first iteration, the simplest and most practical approach is option 1:
- Extensions call decoration functions without specifying an editor
- Functions operate on the currently active editor
- This matches how most editor extensions work (they augment the current editing context)
- Can be extended later with explicit targeting if needed

This requires updating the WIT interface to clarify this behavior in the function documentation.

## Remaining Implementation Work

### 1. WASM Host Implementation (`/home/trillium/code/zed/crates/extension_host/src/wasm_host/wit/since_v0_8_0.rs`)

Need to add:

```rust
use editor::Editor;
use gpui::Hsla;

// Type conversions
impl From<decoration::Color> for Hsla {
    fn from(value: decoration::Color) -> Self {
        Hsla {
            h: value.h,
            s: value.s,
            l: value.l,
            a: value.a,
        }
    }
}

impl From<Hsla> for decoration::Color {
    fn from(value: Hsla) -> Self {
        Self {
            h: value.h,
            s: value.s,
            l: value.l,
            a: value.a,
        }
    }
}

impl From<decoration::DecorationType> for editor::DecorationType {
    fn from(value: decoration::DecorationType) -> Self {
        match value {
            decoration::DecorationType::Before => Self::Before,
            decoration::DecorationType::After => Self::After,
            decoration::DecorationType::Range => Self::Range,
            decoration::DecorationType::WholeLine => Self::WholeLine,
        }
    }
}

impl From<decoration::DecorationRangeBehavior> for editor::DecorationRangeBehavior {
    fn from(value: decoration::DecorationRangeBehavior) -> Self {
        match value {
            decoration::DecorationRangeBehavior::OpenOpen => Self::OpenOpen,
            decoration::DecorationRangeBehavior::OpenClosed => Self::OpenClosed,
            decoration::DecorationRangeBehavior::ClosedOpen => Self::ClosedOpen,
            decoration::DecorationRangeBehavior::ClosedClosed => Self::ClosedClosed,
        }
    }
}

// Similar conversions for DecorationContent, DecorationStyle, etc.

// Host trait implementation
#[async_trait]
impl decoration::Host for WasmState {
    async fn create_decoration_type(
        &mut self,
        options: decoration::DecorationRenderOptions,
    ) -> wasmtime::Result<u64> {
        self.on_main_thread(|cx| {
            async move {
                cx.update(|cx| {
                    // Get active editor
                    let active_editor = /* ... get active editor ... */;

                    // Convert WIT types to Zed types
                    let render_options = editor::DecorationRenderOptions {
                        decoration_type: options.decoration_type.into(),
                        content: options.content.map(Into::into),
                        style: options.style.into(),
                        range_behavior: options.range_behavior.into(),
                    };

                    // Call editor API
                    let type_id = active_editor.create_decoration_type(render_options);
                    type_id.0 as u64
                })
            }
            .boxed_local()
        })
        .await
    }

    async fn set_decorations(
        &mut self,
        type_id: u64,
        decorations: Vec<decoration::Decoration>,
    ) -> wasmtime::Result<Vec<u64>> {
        self.on_main_thread(|cx| {
            async move {
                cx.update(|cx| {
                    // Get active editor and set decorations
                    // ...
                })
            }
            .boxed_local()
        })
        .await
    }

    async fn dispose_decoration_type(
        &mut self,
        type_id: u64,
    ) -> wasmtime::Result<bool> {
        self.on_main_thread(|cx| {
            async move {
                cx.update(|cx| {
                    // Get active editor and dispose decoration type
                    // ...
                })
            }
            .boxed_local()
        })
        .await
    }
}
```

### 2. Extension API Layer (`/home/trillium/code/zed/crates/extension_api/src/extension_api.rs`)

Add Rust-side convenience API for extension authors:

```rust
pub use wit::zed::extension::decoration::{
    Color, Decoration, DecorationContent, DecorationId, DecorationRangeBehavior,
    DecorationRenderOptions, DecorationStyle, DecorationSvg, DecorationType,
    DecorationTypeId, ThemedDecorationStyle,
};

/// Creates a new decoration type.
pub fn create_decoration_type(options: DecorationRenderOptions) -> DecorationTypeId {
    wit::zed::extension::decoration::create_decoration_type(&options)
}

/// Sets decorations in the active editor.
pub fn set_decorations(
    type_id: DecorationTypeId,
    decorations: Vec<Decoration>,
) -> Vec<DecorationId> {
    wit::zed::extension::decoration::set_decorations(type_id, &decorations)
}

/// Disposes a decoration type.
pub fn dispose_decoration_type(type_id: DecorationTypeId) -> bool {
    wit::zed::extension::decoration::dispose_decoration_type(type_id)
}
```

### 3. Active Editor Access

The main challenge is accessing the active editor from the WASM context. This requires:

1. Adding a method to `ExtensionHostProxy` to get the active editor
2. Ensuring thread safety when accessing editor from WASM thread
3. Handling the case where there is no active editor

Possible implementation in `ExtensionHostProxy`:

```rust
pub fn with_active_editor<F, R>(&self, f: F) -> Option<R>
where
    F: FnOnce(&mut Editor, &mut Window, &mut Context<Editor>) -> R,
{
    // Access active pane and editor
    // Call the closure with editor context
}
```

### 4. Tests

Add tests in `/home/trillium/code/zed/crates/extension_host/src/wasm_host/wit/since_v0_8_0.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoration_type_conversion() {
        // Test WIT type -> Zed type conversions
    }

    #[test]
    fn test_color_conversion() {
        // Test HSLA color conversion
    }

    #[gpui::test]
    async fn test_create_decoration_type_wasm(cx: &mut TestAppContext) {
        // Test actual WASM binding
    }
}
```

### 5. Example Extension

Create example in `/home/trillium/code/zed/extensions/cursorless-example/`:

```rust
use zed_extension_api::{self as zed, Result};

struct CursorlessExtension;

impl zed::Extension for CursorlessExtension {
    fn new() -> Self {
        Self
    }
}

// Example: Create hat decorations
fn create_hat_decoration() {
    use zed::decoration::*;

    let options = DecorationRenderOptions {
        decoration_type: DecorationType::Before,
        content: Some(DecorationContent::Svg(DecorationSvg {
            source: "data:image/svg+xml;utf8,<svg>...</svg>".into(),
            width_px: 20.0,
            height_px: 20.0,
        })),
        style: ThemedDecorationStyle {
            base: DecorationStyle {
                margin: Some("-20px 0 0 0".into()),
                z_index: Some(100),
                ..Default::default()
            },
            light: None,
            dark: None,
        },
        range_behavior: DecorationRangeBehavior::ClosedClosed,
    };

    let type_id = zed::create_decoration_type(options);

    let decorations = vec![
        Decoration {
            range: Range { start: 0, end: 1 },
        },
    ];

    zed::set_decorations(type_id, decorations);
}
```

## Next Steps

1. Resolve the editor targeting design decision (recommend: global active editor)
2. Implement the Host trait in `since_v0_8_0.rs` with proper type conversions
3. Add `get_active_editor` or similar to `ExtensionHostProxy`
4. Implement conversion traits for all decoration types
5. Add comprehensive tests
6. Create example extension
7. Document the API in extension documentation

## Alternative: Deferred Implementation

If the editor targeting mechanism proves too complex for the current architecture, consider:
1. Documenting the decoration API as "internal only" for now
2. Implementing a simpler command-based API where extensions send decoration commands via JSON
3. Revisiting WASM bindings in a future iteration when editor/buffer access patterns are better established

## Files Modified

- `/home/trillium/code/zed/crates/extension_api/wit/since_v0.8.0/decoration.wit` (created)
- `/home/trillium/code/zed/crates/extension_api/wit/since_v0.8.0/extension.wit` (modified)

## Files To Modify

- `/home/trillium/code/zed/crates/extension_host/src/wasm_host/wit/since_v0_8_0.rs`
- `/home/trillium/code/zed/crates/extension_api/src/extension_api.rs`
- `/home/trillium/code/zed/crates/extension_host/src/extension_host_proxy.rs` (or similar)
