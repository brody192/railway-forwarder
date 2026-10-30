use gpui_kit::{
    component::input::{Input, InputEvent, InputState},
    *,
};
use serde_json::Value;

use super::SettingsView;
pub(super) fn input_boundary(state: &Entity<InputState>, input: Input, cx: &App) -> Stateful<Div> {
    let focus = state.read(cx).focus_handle(cx);
    div()
        .id(("input-boundary", state.entity_id()))
        .on_mouse_down_out(move |event, window, cx| {
            if event.button == MouseButton::Left && focus.is_focused(window) {
                window.blur(cx);
            }
        })
        .child(input)
}

impl SettingsView {
    pub(crate) fn field(
        &mut self,
        key: String,
        initial: String,
        placeholder: &'static str,
        commit: Option<Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.inputs.get(&key) {
            return input.clone();
        }
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder)
                .default_value(initial)
        });
        if let Some(command) = commit {
            self.subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this, input, event, _, cx| {
                    if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                        let mut command = command.clone();
                        command["value"] = input.read(cx).value().to_string().into();
                        this.send(command, cx);
                    }
                },
            ));
        }
        self.inputs.insert(key, input.clone());
        input
    }
}
