//! Shared interaction styling for settings controls.
use gpui_kit::{
    component::{Disableable, button::Button, switch::Switch},
    *,
};

pub(super) fn button(id: impl Into<ElementId>) -> Button {
    Button::new(id).cursor_pointer()
}

/// Keep pointer activation separate from keyboard focus, as settings buttons do.
pub(super) fn switch_boundary(
    id: impl Into<ElementId>,
    switch: Switch,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let id = (id.into(), "switch-focus-boundary");
    let focus = window
        .use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle().tab_stop(false))
        .read(cx)
        .clone();
    div()
        .id(id)
        .track_focus(&focus)
        .capture_any_mouse_down(|event, window, cx| {
            if event.button == MouseButton::Left {
                window.blur(cx);
                window.prevent_default();
            }
        })
        .on_mouse_down_out(move |event, window, cx| {
            if event.button == MouseButton::Left && focus.contains_focused(window, cx) {
                window.blur(cx);
            }
        })
        .child(switch)
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
