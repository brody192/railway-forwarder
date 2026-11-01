use crate::ui::controls::{ButtonCursorExt, button};
use gpui_kit::{assets::IconName, base::StyledExt, prelude::FluentBuilder};
use gpui_kit::{
    component::{ActiveTheme, Sizable, button::ButtonVariants, input::Input, spinner::Spinner},
    *,
};
use gpui_macos::ActiveTheme as _;
use serde_json::json;

use super::{SIDEBAR_WIDTH, SettingsView, Tab, inputs::input_boundary, style};
impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toolbar_inset = gpui_macos::native::native_toolbar_inset(window);
        let active = window.is_window_active();
        let mut sidebar = div()
            .id("settings-sidebar")
            .v_flex()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .p_2()
            .font_family(cx.macos().font.clone())
            .bg(if active {
                transparent_black()
            } else {
                rgb(0x242c32).into()
            })
            .gap_1()
            .pt(toolbar_inset.max(px(52.)));
        for (tab, id) in [
            (Tab::Services, "services"),
            (Tab::Menu, "menu"),
            (Tab::General, "general"),
            (Tab::Account, "account"),
        ] {
            let selected = self.tab == tab;
            let fill = style::selection_fill(selected, window, cx);
            sidebar = sidebar.child(
                gpui_macos::button(id, "", gpui_macos::ButtonVariant::Secondary, cx)
                    .accessibility_label(tab.title())
                    .aria_selected(selected)
                    .h(px(34.))
                    .w_full()
                    .px_2()
                    .py_1()
                    .rounded(px(8.))
                    .justify_start()
                    .gap_2()
                    .border_0()
                    .bg(fill)
                    .hover(move |style| style.bg(fill))
                    .child(
                        gpui_macos::icon(tab.icon())
                            .size(px(22.))
                            .text_color(if active {
                                cx.macos().accent
                            } else {
                                cx.macos().secondary
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(if selected {
                                FontWeight::MEDIUM
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(if active {
                                cx.macos().foreground
                            } else {
                                cx.macos().secondary
                            })
                            .child(tab.title()),
                    )
                    .cursor_pointer()
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
        let mut body = div()
            .v_flex()
            .size_full()
            .min_w_0()
            .gap_5()
            .p_6()
            .bg(cx.theme().background);
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
            body = body
                .child(style::page_heading(self.tab))
                .child(self.account(cx));
        } else if self.tab == Tab::Menu {
            body = body.child(self.menu_editor(window, cx));
        } else if self.tab == Tab::General {
            body = body
                .child(style::page_heading(self.tab))
                .child(self.general(window, cx));
        } else {
            body = body.child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_3()
                    .child(style::page_heading(self.tab).flex_1())
                    .child(
                        button("selected-ports")
                            .ghost()
                            .h(px(30.))
                            .rounded_full()
                            .icon(IconName::Info)
                            .label(format!(
                                "{} ports selected",
                                self.snapshot.preferences.allowed.len()
                            ))
                            .when(self.selected_only, |button| {
                                button
                                    .bg(cx.macos().accent.opacity(0.2))
                                    .text_color(cx.macos().accent)
                            })
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
                                .h(px(30.)).rounded_full(),
                            cx,
                        )
                        .min_w_0()
                        .flex_1(),
                    )
                    .child(
                        button("clear-filters")
                            .label("Clear")
                            .h(px(30.)).rounded_full()
                            .disabled_with_cursor(!self.selected_only && self.query.is_empty())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.clear_filters(window, cx)),
                            ),
                    )
                    .child(
                        button("refresh").icon(IconName::RotateCw)
                            .size(px(30.)).p_0().rounded_full()
                            .accessibility_label("Refresh").tooltip("Refresh projects")
                            .disabled_with_cursor(!self.snapshot.signed_in || self.snapshot.loading.contains(&"workspaces".into()))
                            .on_click(cx.listener(|this, _, _, cx| this.send(json!({"action":"refresh"}), cx))),
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
            .key_context("ForwarderSettings")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if !this.menu_selection.is_empty() {
                        window.blur(cx);
                        this.menu_selection.clear();
                        cx.notify();
                    }
                }),
            )
            .on_action(|_: &crate::app::CloseSettings, window, _| window.remove_window())
            .size_full()
            .child(
                gpui_macos::focus_scope("settings-focus")
                    .h_flex()
                    .size_full()
                    .text_color(cx.theme().foreground)
                    .text_size(px(13.))
                    .child(sidebar)
                    .child(div().flex_1().min_w_0().h_full().child(body)),
            )
    }
}
