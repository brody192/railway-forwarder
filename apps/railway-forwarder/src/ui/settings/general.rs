use super::{SettingsView, Tab};
use crate::{
    app::events::{self, Event},
    domain::LoginItemStatus,
    ui::controls::{button, switch_boundary},
};
use gpui_kit::{
    base::StyledExt,
    component::{ActiveTheme, Disableable, switch::Switch},
    prelude::FluentBuilder,
    *,
};
use serde_json::json;

impl SettingsView {
    pub(super) fn watch_login_item(window: &Window, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                // Use the window-bound context so closing Settings stops this
                // watcher even while the app retains its last view handle.
                if view
                    .update_in(cx, |view, _, _| {
                        if view.tab == Tab::General {
                            events::emit(Event::RefreshLoginItem);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    pub(super) fn general(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let disabled =
            self.snapshot.demo || self.snapshot.login_item == LoginItemStatus::Unavailable;
        let mut content = div()
            .v_flex()
            .gap_5()
            .max_w(px(560.))
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("General"),
            )
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child("Start at login")
                    .child(switch_boundary(
                        "start-at-login",
                        Switch::new("start-at-login")
                            .accessibility_label("Start at login")
                            .checked(self.snapshot.login_item.is_registered())
                            .disabled(disabled)
                            .cursor_pointer()
                            .when(disabled, |switch| switch.cursor_default())
                            .on_change(cx.listener(|_, enabled: &bool, _, _| {
                                events::emit(Event::SetStartAtLogin(*enabled));
                            })),
                        window,
                        cx,
                    )),
            );
        if self.snapshot.login_item == LoginItemStatus::RequiresApproval {
            content = content.child(div().v_flex().gap_3()
                .child("Allow Railway Forwarder in System Settings → General → Login Items to enable automatic startup.")
                .child(button("open-login-items").label("Open Login Items")
                    .on_click(|_, _, _| events::emit(Event::OpenLoginItems))));
        } else if self.snapshot.login_item == LoginItemStatus::Unavailable && !self.snapshot.demo {
            content = content.child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("Run the signed Railway Forwarder.app to configure start at login."),
            );
        }
        if let Some(error) = self.error("login-item", None, cx) {
            content = content.child(error);
        }
        content = content.child(
            div()
                .h_flex()
                .justify_between()
                .py_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child("Restore active tunnels on launch")
                .child(switch_boundary(
                    "restore-tunnels-on-launch",
                    Switch::new("restore-tunnels-on-launch")
                        .accessibility_label("Restore active tunnels on launch")
                        .tooltip("Reconnect the last enabled tunnels when the app starts")
                        .checked(self.snapshot.preferences.restore_tunnels_on_launch)
                        .cursor_pointer()
                        .on_change(cx.listener(|this, enabled: &bool, _, cx| {
                            this.send(
                                json!({"action":"restoreTunnelsOnLaunch","enabled":enabled}),
                                cx,
                            );
                        })),
                    window,
                    cx,
                )),
        );
        content.into_any_element()
    }
}
