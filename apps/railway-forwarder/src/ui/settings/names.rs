use crate::domain::DisplayNameLabel;
use gpui_kit::{
    component::input::{InputEvent, InputState},
    *,
};
use serde_json::json;

use super::SettingsView;
pub(super) struct NameField {
    input: Entity<InputState>,
    source: String,
    _subscription: Subscription,
}

impl SettingsView {
    pub(crate) fn name_field(
        &mut self,
        label: &DisplayNameLabel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(field) = self.name_fields.get(&label.key) {
            return field.input.clone();
        }
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Display name")
                .default_value(label.value.clone())
        });
        let key = label.key.clone();
        let subscription = cx.subscribe_in(&input, window, move |this, input, event, window, cx| {
            if matches!(event, InputEvent::Change) {
                this.send(json!({"action":"displayName", "id":key, "value":input.read(cx).value().to_string()}), cx);
                // Keep the active draft intact (for example its trailing space)
                // while all labels already show the resolved global name.
                if let (Some(field), Some(label)) = (this.name_fields.get_mut(&key), this.snapshot.display_names.iter().find(|label| label.key == key)) {
                    field.source = label.value.clone();
                }
            } else if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                if let Some(label) = this.snapshot.display_names.iter().find(|label| label.key == key) {
                    input.update(cx, |state, cx| state.set_value(label.value.clone(), window, cx));
                }
            }
        });
        self.name_fields.insert(
            label.key.clone(),
            NameField {
                input: input.clone(),
                source: label.value.clone(),
                _subscription: subscription,
            },
        );
        input
    }
    pub(super) fn sync_name_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for label in &self.snapshot.display_names {
            if let Some(field) = self.name_fields.get_mut(&label.key) {
                if field.source != label.value {
                    field.input.update(cx, |state, cx| {
                        state.set_value(label.value.clone(), window, cx)
                    });
                    field.source = label.value.clone();
                }
            }
        }
    }
    pub(crate) fn reset_display_name(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.send(json!({"action":"displayName", "id":key, "value":""}), cx);
        self.sync_name_fields(window, cx);
    }
    pub(crate) fn reset_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.send(json!({"action":"menuReset"}), cx);
        self.sync_name_fields(window, cx);
    }
}
