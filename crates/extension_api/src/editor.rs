//! Editor state access API for extensions.

pub use crate::wit::zed::extension::editor::{
    EditorState, Position, Selection, get_active_editor_state, get_active_editor_text,
    get_active_editor_visible_range,
};
