use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use editor::Editor;
use editor::decorations::{
    Decoration, DecorationId, DecorationRangeBehavior, DecorationRenderOptions, DecorationStyle,
    DecorationTypeId, ThemedDecorationStyle,
};
use extension::{
    ExtensionDecoration, ExtensionDecorationContent, ExtensionDecorationProxy,
    ExtensionDecorationRangeBehavior, ExtensionDecorationRenderOptions,
    ExtensionDecorationStyle, ExtensionDecorationType, ExtensionHostProxy,
    ExtensionThemedDecorationStyle,
};
use gpui::{App, SharedString};
use multi_buffer::MultiBufferOffset;
use workspace::MultiWorkspace;

pub fn init(extension_host_proxy: Arc<ExtensionHostProxy>) {
    extension_host_proxy.register_decoration_proxy(DecorationProxyImpl::new());
}

struct DecorationProxyImpl {
    next_decoration_id: AtomicUsize,
}

impl DecorationProxyImpl {
    fn new() -> Self {
        Self {
            next_decoration_id: AtomicUsize::new(1),
        }
    }

    fn with_active_editor<R>(
        &self,
        cx: &mut App,
        f: impl FnOnce(&mut Editor, &mut gpui::Context<Editor>) -> R,
    ) -> Option<R> {
        // Try active window first, fall back to any MultiWorkspace window.
        // During startup the platform may not yet report an active window.
        let multi_workspace = cx
            .active_window()
            .and_then(|w| w.downcast::<MultiWorkspace>())
            .or_else(|| {
                cx.windows()
                    .into_iter()
                    .find_map(|w| w.downcast::<MultiWorkspace>())
            })?;
        multi_workspace
            .update(cx, |mw, _window, cx| {
                let workspace = mw.workspace().read(cx);
                let editor = workspace.active_item_as::<Editor>(cx)?;
                Some(editor.update(cx, |editor, cx| f(editor, cx)))
            })
            .ok()?
    }
}

fn convert_style(style: ExtensionDecorationStyle) -> DecorationStyle {
    DecorationStyle {
        background_color: style.background_color,
        border_color: style.border_color,
        border_style: style.border_style,
        border_width: style.border_width,
        border_radius: style.border_radius,
        margin: style.margin,
        z_index: style.z_index,
    }
}

fn convert_themed_style(style: ExtensionThemedDecorationStyle) -> ThemedDecorationStyle {
    ThemedDecorationStyle {
        base: convert_style(style.base),
        light: style.light.map(convert_style),
        dark: style.dark.map(convert_style),
    }
}

fn convert_render_options(options: ExtensionDecorationRenderOptions) -> DecorationRenderOptions {
    DecorationRenderOptions {
        decoration_type: match options.decoration_type {
            ExtensionDecorationType::Before => editor::decorations::DecorationType::Before,
            ExtensionDecorationType::After => editor::decorations::DecorationType::After,
            ExtensionDecorationType::Range => editor::decorations::DecorationType::Range,
            ExtensionDecorationType::WholeLine => editor::decorations::DecorationType::WholeLine,
        },
        content: options.content.map(|c| match c {
            ExtensionDecorationContent::Text(t) => {
                editor::decorations::DecorationContent::Text(SharedString::from(t))
            }
            ExtensionDecorationContent::Svg {
                source,
                width_px,
                height_px,
            } => editor::decorations::DecorationContent::Svg {
                source: SharedString::from(source),
                width_px,
                height_px,
            },
            ExtensionDecorationContent::Image { .. } => {
                // Image not yet supported in editor, fall back to empty text
                editor::decorations::DecorationContent::Text(SharedString::default())
            }
        }),
        style: convert_themed_style(options.style),
        range_behavior: match options.range_behavior {
            ExtensionDecorationRangeBehavior::OpenOpen => DecorationRangeBehavior::OpenOpen,
            ExtensionDecorationRangeBehavior::OpenClosed => DecorationRangeBehavior::OpenClosed,
            ExtensionDecorationRangeBehavior::ClosedOpen => DecorationRangeBehavior::ClosedOpen,
            ExtensionDecorationRangeBehavior::ClosedClosed => DecorationRangeBehavior::ClosedClosed,
        },
    }
}

impl ExtensionDecorationProxy for DecorationProxyImpl {
    fn create_decoration_type(
        &self,
        options: ExtensionDecorationRenderOptions,
        cx: &mut App,
    ) -> u64 {
        let editor_options = convert_render_options(options);
        self.with_active_editor(cx, |editor, _cx| {
            editor.create_decoration_type(editor_options).0 as u64
        })
        .unwrap_or(0)
    }

    fn set_decorations(
        &self,
        type_id: u64,
        decorations: Vec<ExtensionDecoration>,
        cx: &mut App,
    ) -> Vec<u64> {
        let next_id = &self.next_decoration_id;

        self.with_active_editor(cx, |editor, cx| {
            let type_id = DecorationTypeId(type_id as usize);
            let behavior = editor
                .decoration_registry
                .get_range_behavior(type_id)
                .unwrap_or_default();
            let (start_bias, end_bias) = behavior.to_bias();

            let buffer = editor.buffer().read(cx);
            let snapshot = buffer.snapshot(cx);

            let mut ids = Vec::with_capacity(decorations.len());
            let mut editor_decorations = Vec::with_capacity(decorations.len());

            for ext_dec in &decorations {
                let start_offset = ext_dec.range_start as usize;
                let end_offset = ext_dec.range_end as usize;
                let dec_id = DecorationId(next_id.fetch_add(1, Ordering::Relaxed));

                let start = snapshot.anchor_at(MultiBufferOffset(start_offset), start_bias);
                let decoration = if start_offset == end_offset {
                    Decoration::point(dec_id, type_id, start)
                } else {
                    let end = snapshot.anchor_at(MultiBufferOffset(end_offset), end_bias);
                    Decoration::range(dec_id, type_id, start, end)
                };

                ids.push(dec_id.0 as u64);
                editor_decorations.push(decoration);
            }

            editor.set_decorations(type_id, editor_decorations, cx);
            ids
        })
        .unwrap_or_default()
    }

    fn dispose_decoration_type(&self, type_id: u64, cx: &mut App) -> bool {
        self.with_active_editor(cx, |editor, _cx| {
            editor.dispose_decoration_type(DecorationTypeId(type_id as usize))
        })
        .unwrap_or(false)
    }
}
