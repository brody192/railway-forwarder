use super::{SettingsView, Tab, style};
use crate::{
    app::events::{self, Event},
    domain::LoginItemStatus,
    ui::controls::{button, switch_boundary},
};
use gpui_kit::{base::StyledExt, component::ActiveTheme, prelude::FluentBuilder, *};
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
        let login_change = cx.listener(|_: &mut Self, enabled: &bool, _, _| {
            events::emit(Event::SetStartAtLogin(*enabled));
        });
        let restore_change = cx.listener(|this: &mut Self, enabled: &bool, _, cx| {
            this.send(
                json!({"action":"restoreTunnelsOnLaunch","enabled":enabled}),
                cx,
            );
        });
        let login_switch = gpui_macos::switch(
            "start-at-login",
            "",
            self.snapshot.login_item.is_registered(),
            cx,
        )
        .accessibility_label("Start at login")
        .disabled(disabled)
        .cursor_pointer()
        .when(disabled, |switch| switch.cursor_default())
        .on_change(move |enabled, _, window, cx| login_change(&enabled, window, cx));
        let restore_switch = gpui_macos::switch(
            "restore-tunnels-on-launch",
            "",
            self.snapshot.preferences.restore_tunnels_on_launch,
            cx,
        )
        .accessibility_label("Restore active tunnels on launch")
        .cursor_pointer()
        .on_change(move |enabled, _, window, cx| restore_change(&enabled, window, cx));
        let restore_switch = gpui_macos::with_tooltip(
            restore_switch,
            "Reconnect the last enabled tunnels when the app starts",
        );
        let group = style::group(cx)
            .child(style::row("Start at login").child(switch_boundary(
                "start-at-login",
                login_switch,
                window,
                cx,
            )))
            .child(style::divider(cx))
            .child(
                style::row("Restore active tunnels on launch").child(switch_boundary(
                    "restore-tunnels-on-launch",
                    restore_switch,
                    window,
                    cx,
                )),
            );
        let mut content = div()
            .v_flex()
            .gap_3()
            .max_w(px(600.))
            .child(style::section_label("Startup", cx))
            .child(group);
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
        content.into_any_element()
    }
}
