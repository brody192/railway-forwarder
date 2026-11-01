use crate::ui::controls::{ButtonCursorExt, button};
use gpui_kit::{assets::IconName, base::StyledExt};
use gpui_kit::{
    component::{ActiveTheme, button::ButtonVariants, input::Input},
    *,
};
use serde_json::json;

use super::{AccountPhoto, Arc, SettingsView, inputs::input_boundary, style};
use crate::domain::Snapshot;
impl SettingsView {
    pub(super) fn make_profile_image(
        snapshot: &Snapshot,
    ) -> Option<(Arc<AccountPhoto>, Arc<Image>)> {
        let photo = snapshot.account_photo.as_ref()?;
        let format = ImageFormat::from_mime_type(&photo.mime_type)?;
        Some((
            photo.clone(),
            Arc::new(Image::from_bytes(format, photo.bytes.clone())),
        ))
    }

    pub(super) fn sync_profile_image(&mut self) {
        if self
            .profile_image
            .as_ref()
            .map(|(source, _)| source)
            .zip(self.snapshot.account_photo.as_ref())
            .is_some_and(|(current, next)| Arc::ptr_eq(current, next))
        {
            return;
        }
        self.profile_image = Self::make_profile_image(&self.snapshot);
    }

    pub(super) fn account(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let name = self.snapshot.account_name.as_deref().unwrap_or("Railway");
        let mut avatar = div()
            .size(px(48.))
            .flex_shrink_0()
            .rounded_full()
            .overflow_hidden()
            .h_flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().secondary);
        if let Some((_, image)) = &self.profile_image {
            avatar = avatar.child(
                img(image.clone())
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .rounded_full(),
            );
        } else {
            let initials: String = name
                .split_whitespace()
                .take(2)
                .filter_map(|part| part.chars().next())
                .flat_map(char::to_uppercase)
                .collect();
            avatar = avatar.child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(initials),
            );
        }
        let identity = if self.snapshot.signed_in {
            self.snapshot
                .email
                .as_ref()
                .map(|email| format!("Signed in to Railway as: {email}"))
                .unwrap_or_else(|| "Signed in to Railway".into())
        } else {
            "Not signed in".into()
        };
        let mut content = div().v_flex().gap_5().max_w(px(600.)).child(
            style::group(cx).p_4().child(
                div().h_flex().items_center().gap_3().child(avatar).child(
                    div()
                        .v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(name.to_owned()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .child(identity),
                        ),
                ),
            ),
        );
        if self.snapshot.signed_in {
            if let Some(error) = self.error("account", Some(json!({"action":"refreshAccount"})), cx)
            {
                content = content.child(error);
            }
            content = content.child(
                style::group(cx)
                    .child(
                        style::row("Project access").child(
                            button("change-projects")
                                .label("Manage")
                                .accessibility_label("Manage project access")
                                .rounded_full()
                                .disabled_with_cursor(self.snapshot.busy || self.snapshot.demo)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.send(json!({"action":"signIn"}), cx)
                                })),
                        ),
                    )
                    .child(style::divider(cx))
                    .child(
                        style::row("Railway session").child(
                            button("sign-out")
                                .ghost()
                                .rounded_full()
                                .text_color(cx.theme().danger)
                                .label("Sign out")
                                .disabled_with_cursor(self.snapshot.busy || self.snapshot.demo)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.send(json!({"action":"signOut"}), cx)
                                })),
                        ),
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
