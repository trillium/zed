//! Editor state and mutation API for extensions.

pub use crate::wit::zed::extension::editor::{
    EditorState, Position, Selection, get_active_editor_state, get_active_editor_text,
    get_active_editor_visible_range,
    set_selections, replace_text_in_range, insert_text, reveal_line,
    copy_selections_to_clipboard,
};
