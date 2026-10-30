//! Shared interaction styling for settings controls.
use gpui_kit::{
    ElementId, Styled,
    component::{Disableable, button::Button},
};

pub(super) fn button(id: impl Into<ElementId>) -> Button {
    Button::new(id).cursor_pointer()
}

pub(super) trait ButtonCursorExt {
    fn disabled_with_cursor(self, disabled: bool) -> Self;
}

impl ButtonCursorExt for Button {
    fn disabled_with_cursor(self, disabled: bool) -> Self {
        let button = self.disabled(disabled);
        if disabled {
            button.cursor_default()
        } else {
            button.cursor_pointer()
        }
    }
}
