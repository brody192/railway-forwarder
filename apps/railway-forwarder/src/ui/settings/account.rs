use crate::ui::controls::{ButtonCursorExt, button};
use gpui_kit::{assets::IconName, base::StyledExt};
use gpui_kit::{
    component::{ActiveTheme, button::ButtonVariants, input::Input},
    *,
};
use serde_json::json;

use super::{SettingsView, inputs::input_boundary};
impl SettingsView {
    pub(super) fn account(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div()
            .v_flex()
            .gap_5()
            .max_w(px(560.))
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Account"),
            )
            .child(if self.snapshot.signed_in {
                self.snapshot
                    .email
                    .as_ref()
                    .map(|email| format!("Signed in to Railway as: {email}"))
                    .unwrap_or_else(|| "Signed in to Railway".into())
            } else {
                "Not signed in".into()
            });
        if self.snapshot.signed_in {
            if let Some(error) = self.error("account", Some(json!({"action":"refreshAccount"})), cx)
            {
                content = content.child(error);
            }
            content = content.child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        button("change-projects")
                            .label("Project access")
                            .disabled_with_cursor(self.snapshot.busy || self.snapshot.demo)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.send(json!({"action":"signIn"}), cx)
                            })),
                    )
                    .child(
                        button("sign-out")
                            .label("Sign out")
                            .disabled_with_cursor(self.snapshot.busy || self.snapshot.demo)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.send(json!({"action":"signOut"}), cx)
                            })),
                    ),
            );
        } else {
            content = content.child(self.sign_in(cx));
        }
        content = content.child(
            div()
                .border_t_1()
                .border_color(cx.theme().border)
                .pt_4()
                .child(self.disclosure(
                    "advanced".into(),
                    "Advanced".into(),
                    IconName::Settings,
                    0,
                    None,
                    cx,
                )),
        );
        if self.disclosures.contains("advanced") {
            let input = self.client_id.clone();
            content = content.child(div().v_flex().gap_2().child("OAuth client ID")
                .child(input_boundary(&self.client_id, Input::new(&self.client_id).disabled(self.snapshot.signed_in).aria_label("OAuth client ID"), cx).w_full())
                .child(button("save-client").label("Save").disabled_with_cursor(self.snapshot.signed_in).on_click(cx.listener(move |this, _, _, cx| {
                    this.send(json!({"action":"clientID", "value":input.read(cx).value().to_string()}), cx);
                })))
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Redirect URI"))
                .child(div().text_sm().child("com.brody.railway-forwarder://oauth/callback")));
        }
        content.into_any_element()
    }
    pub(super) fn sign_in(&self, cx: &mut Context<Self>) -> AnyElement {
        button("sign-in")
            .primary()
            .label(if self.snapshot.busy {
                "Signing in…"
            } else {
                "Sign in with Railway"
            })
            .disabled_with_cursor(self.snapshot.busy || self.snapshot.demo)
            .on_click(cx.listener(|this, _, _, cx| this.send(json!({"action":"signIn"}), cx)))
            .into_any_element()
    }
}
