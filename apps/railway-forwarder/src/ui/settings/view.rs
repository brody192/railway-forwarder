use crate::ui::controls::{ButtonCursorExt, button};
use gpui_kit::{assets::IconName, base::StyledExt, prelude::FluentBuilder};
use gpui_kit::{
    component::{ActiveTheme, Sizable, button::ButtonVariants, input::Input, spinner::Spinner},
    *,
};
use serde_json::json;

use super::{SettingsView, Tab, inputs::input_boundary};
impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut sidebar = div()
            .v_flex()
            .gap_2()
            .h_full()
            .w(px(172.))
            .flex_shrink_0()
            .p_3()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(
                div()
                    .px_2()
                    .pt_2()
                    .pb_4()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Railway Forwarder"),
            );
        for (tab, id, label, icon) in [
            (Tab::Services, "services", "Services", IconName::Server),
            (Tab::Menu, "menu", "Menu", IconName::List),
            (Tab::General, "general", "General", IconName::Settings),
            (Tab::Account, "account", "Account", IconName::User),
        ] {
            sidebar = sidebar.child(
                button(id)
                    .ghost()
                    .accessibility_label(label)
                    .child(
                        div()
                            .h_flex()
                            .w_full()
                            .gap_2()
                            .child(gpui_kit::component::Icon::new(icon).size_4())
                            .child(label),
                    )
                    .w_full()
                    .when(self.tab == tab, |button| button.bg(cx.theme().secondary))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tab = tab;
                        if tab == Tab::General {
                            crate::app::events::emit(crate::app::events::Event::RefreshLoginItem);
                        }
                        cx.notify();
                    })),
            );
        }
        sidebar = sidebar.child(div().flex_1());
        if self.snapshot.demo {
            sidebar = sidebar.child(
                div()
                    .px_2()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Preview"),
            );
        }
        let mut body = div().v_flex().h_full().flex_1().min_w_0().gap_4().p_6();
        if let Some(error) = &self.snapshot.error {
            body =
                body.child(
                    div()
                        .h_flex()
                        .gap_3()
                        .items_center()
                        .p_3()
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().danger)
                        .text_sm()
                        .child(div().flex_1().child(error.clone()))
                        .child(button("dismiss-error").ghost().label("Dismiss").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.send(json!({"action":"dismissError"}), cx)
                            }),
                        )),
                );
        }
        if self.tab == Tab::Account {
            body = body.child(self.account(cx));
        } else if self.tab == Tab::Menu {
            body = body.child(self.menu_editor(window, cx));
        } else if self.tab == Tab::General {
            body = body.child(self.general(window, cx));
        } else {
            body = body.child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .flex_1()
                            .child("Services"),
                    )
                    .child(
                        button("selected-ports")
                            .ghost()
                            .icon(IconName::Info)
                            .label(format!(
                                "{} ports selected",
                                self.snapshot.preferences.allowed.len()
                            ))
                            .when(self.selected_only, |button| button.bg(cx.theme().secondary))
                            .disabled_with_cursor(
                                self.snapshot.preferences.allowed.is_empty() && !self.selected_only,
                            )
                            .tooltip(if self.selected_only {
                                "Show all ports"
                            } else {
                                "Show only selected ports"
                            })
                            .on_click(
                                cx.listener(|this, _, window, cx| this.toggle_selected(window, cx)),
                            ),
                    )
                    .child(
                        button("refresh")
                            .label("Refresh")
                            .disabled_with_cursor(
                                !self.snapshot.signed_in
                                    || self.snapshot.loading.contains(&"workspaces".into()),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.send(json!({"action":"refresh"}), cx);
                            })),
                    ),
            );
            body = body.child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        input_boundary(
                            &self.search,
                            Input::new(&self.search)
                                .prefix(gpui_kit::component::Icon::new(IconName::Search))
                                .suffix(
                                    div().size_4().when_else(
                                        self.snapshot.loading.contains(&"search".into()),
                                        |view| view.child(Spinner::new().small()),
                                        |view| view.when_some(self.snapshot.node_errors.get("search"), |view, error| {
                                            view.child(button("retry-search")
                                                .ghost().size_4().icon(IconName::Info)
                                                .tooltip(format!("{error} Click to retry."))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.send(json!({"action":"searchCatalog", "enabled":true, "value":this.query}), cx);
                                                })))
                                        }),
                                    ),
                                )
                                .disabled(!self.snapshot.signed_in)
                                .aria_label("Search services")
                                .h(px(32.)),
                            cx,
                        )
                        .min_w_0()
                        .flex_1(),
                    )
                    .child(
                        button("clear-filters")
                            .label("Clear")
                            .h(px(32.))
                            .disabled_with_cursor(!self.selected_only && self.query.is_empty())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.clear_filters(window, cx)),
                            ),
                    ),
            );
            if !self.snapshot.signed_in {
                body = body.child(self.sign_in(cx));
            } else {
                if self.snapshot.loading.contains(&"workspaces".into()) {
                    body = body.child(self.note("Loading workspaces…", 0, cx));
                }
                if let Some(error) = self.error("workspaces", Some(json!({"action":"refresh"})), cx)
                {
                    body = body.child(error);
                }
                if self.snapshot.workspaces.is_empty()
                    && self.snapshot.loading.is_empty()
                    && !self.snapshot.node_errors.contains_key("workspaces")
                {
                    body = body.child(self.note("No accessible projects", 0, cx));
                }
                body = body.child(
                    div()
                        .id("catalog")
                        .overflow_y_scroll()
                        .flex_1()
                        .min_h_0()
                        .child(self.tree(window, cx)),
                );
            }
        }
        div()
            .h_flex()
            .key_context("ForwarderSettings")
            .on_action(|_: &crate::app::CloseSettings, window, _| window.remove_window())
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_sm()
            .child(sidebar)
            .child(body)
    }
}
