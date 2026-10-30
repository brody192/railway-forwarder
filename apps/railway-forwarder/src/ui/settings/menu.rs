use crate::ui::controls::button;
use crate::{
    domain::{MenuPort, MenuSection},
    ui::settings::{SettingsView, inputs::input_boundary},
};
use gpui_kit::{
    assets::IconName,
    base::StyledExt,
    component::{
        ActiveTheme, Disableable, Sizable, button::ButtonVariants, input::Input, spinner::Spinner,
        switch::Switch,
    },
    prelude::FluentBuilder,
    *,
};
use serde_json::json;

#[derive(Clone)]
struct MenuDrag {
    kind: &'static str,
    id: String,
    section: String,
    title: String,
}
impl Render for MenuDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_4()
            .py_2()
            .rounded_lg()
            .bg(cx.theme().secondary)
            .border_1()
            .border_color(cx.theme().primary)
            .text_color(cx.theme().foreground)
            .child(self.title.clone())
    }
}

fn handle(drag: MenuDrag) -> AnyElement {
    div()
        .id(SharedString::from(format!(
            "drag-area/{}/{}",
            drag.kind, drag.id
        )))
        .cursor_grab()
        .child(
            button(SharedString::from(format!(
                "drag/{}/{}",
                drag.kind, drag.id
            )))
            .ghost()
            .small()
            .icon(IconName::GripVertical)
            .cursor_grab()
            .accessibility_label(format!("Reorder {}", drag.title)),
        )
        .on_drag(drag, |drag, _, _, cx| {
            cx.stop_propagation();
            cx.new(|_| drag.clone())
        })
        .into_any_element()
}

impl SettingsView {
    pub(crate) fn menu_editor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let menu = self.snapshot.menu.clone();
        let mut preview = div()
            .v_flex()
            .flex_shrink_0()
            .w_full()
            .max_w(px(540.))
            .py_2()
            .rounded_xl()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover);
        preview = preview.child(self.fixed_menu_label(menu.header.clone(), true, cx));
        if menu.sections.is_empty() {
            preview = preview.child(
                div()
                    .px_4()
                    .py_4()
                    .text_color(cx.theme().muted_foreground)
                    .child("No ports selected"),
            );
        }
        for (index, section) in menu.sections.iter().enumerate() {
            if index > 0 {
                preview = preview.child(self.menu_divider(cx));
            }
            preview = preview.child(self.section_heading(section, cx));
            for port in &section.ports {
                preview = preview.child(self.preview_port(port, cx));
            }
        }
        preview = preview
            .child(self.menu_divider(cx))
            .child(self.fixed_menu_label(menu.disconnect.clone(), false, cx))
            .child(self.menu_divider(cx))
            .child(self.fixed_menu_label(menu.settings.clone(), false, cx))
            .child(self.fixed_menu_label(menu.quit.clone(), false, cx));

        let has_selection = self.menu_selection.iter().any(|key| {
            self.snapshot
                .display_names
                .iter()
                .any(|label| &label.key == key)
        });
        let mut inspector = div()
            .v_flex()
            .flex_shrink_0()
            .gap_3()
            .w_full()
            .max_w(px(540.))
            .pt_3()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Display names"),
            );
        let names = self.snapshot.display_names.clone();
        for key in self.menu_selection.clone() {
            if let Some(label) = names.iter().find(|label| label.key == key) {
                let input = self.name_field(label, window, cx);
                let mut row = div()
                    .h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .w(px(94.))
                            .flex_shrink_0()
                            .text_color(cx.theme().muted_foreground)
                            .child(label.label.clone()),
                    )
                    .child(
                        input_boundary(
                            &input,
                            Input::new(&input).aria_label(format!("{} display name", label.label)),
                            cx,
                        )
                        .flex_1()
                        .min_w_0(),
                    );
                if label.custom {
                    row = row.child(
                        button(SharedString::from(format!("reset-name/{key}")))
                            .ghost()
                            .small()
                            .label("Reset")
                            .tooltip(format!("Original name: {}", label.original))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.reset_display_name(&key, window, cx);
                            })),
                    );
                }
                inspector = inspector.child(row);
            }
        }
        div()
            .v_flex()
            .gap_4()
            .h_full()
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Menu"),
                    )
                    .child(
                        button("reset-menu")
                            .ghost()
                            .label("Reset")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.reset_menu(window, cx);
                            })),
                    )
                    .child(button("open-native-menu").label("Open menu").on_click(
                        cx.listener(|this, _, _, cx| this.send(json!({"action":"showMenu"}), cx)),
                    )),
            )
            .child(
                div()
                    .id("menu-editor-scroll")
                    .v_flex()
                    .gap_4()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(preview)
                    .when(has_selection, |view| view.child(inspector)),
            )
            .into_any_element()
    }

    fn menu_divider(&self, cx: &App) -> AnyElement {
        div()
            .mx_3()
            .my_2()
            .h(px(1.))
            .bg(cx.theme().border)
            .into_any_element()
    }

    fn fixed_menu_label(&self, title: String, header: bool, cx: &App) -> AnyElement {
        div()
            .w_full()
            .px_4()
            .py_2()
            .text_sm()
            .when(header, |view| {
                view.font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().muted_foreground)
            })
            .child(title)
            .into_any_element()
    }

    fn section_heading(&self, section: &MenuSection, cx: &mut Context<Self>) -> AnyElement {
        let drag = MenuDrag {
            kind: "section",
            id: section.id.clone(),
            section: String::new(),
            title: section.project_name.clone(),
        };
        let keys = section.name_keys.clone();
        let target = section.id.clone();
        div()
            .id(SharedString::from(format!("section/{}", section.id)))
            .h_flex()
            .gap_1()
            .px_2()
            .py_1()
            .border_1()
            .border_color(transparent_black())
            .child(handle(drag))
            .child(
                button(SharedString::from(format!("section-name/{}", section.id)))
                    .ghost()
                    .flex_1()
                    .accessibility_label(format!("Edit {} section", section.project_name))
                    .child(
                        div()
                            .h_flex()
                            .w_full()
                            .gap_3()
                            .text_sm()
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(section.project_name.clone()),
                            )
                            .child(
                                div()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(section.workspace_name.clone()),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.menu_selection = keys.clone();
                        cx.notify();
                    })),
            )
            .can_drop(|value, _, _| {
                value
                    .downcast_ref::<MenuDrag>()
                    .is_some_and(|drag| drag.kind == "section")
            })
            .drag_over::<MenuDrag>(|view, drag, _, cx| {
                if drag.kind == "section" {
                    view.border_color(cx.theme().primary)
                } else {
                    view
                }
            })
            .on_drop(cx.listener(move |this, drag: &MenuDrag, _, cx| {
                this.send(
                    json!({"action":"menuMove","kind":"section","id":drag.id,"targetID":target}),
                    cx,
                )
            }))
            .into_any_element()
    }

    fn preview_port(&self, port: &MenuPort, cx: &mut Context<Self>) -> AnyElement {
        let drag = MenuDrag {
            kind: "port",
            id: port.id.clone(),
            section: port.section_id.clone(),
            title: port.title.clone(),
        };
        let keys = port.name_keys.clone();
        let target = port.id.clone();
        let section = port.section_id.clone();
        let hover_section = section.clone();
        let title = match port.state.as_deref() {
            Some("failed") => format!("{} (Failed)", port.title),
            _ => port.title.clone(),
        };
        let detail = port.error.clone().unwrap_or_else(|| {
            format!(
                "127.0.0.1:{} → {}:{}",
                port.port.local_port, port.port.remote.host, port.port.remote.port
            )
        });
        div()
            .id(SharedString::from(format!("preview-port/{}", port.id)))
            .h_flex()
            .gap_1()
            .px_2()
            .min_h(px(54.))
            .border_1()
            .border_color(transparent_black())
            .child(handle(drag))
            .child(
                button(SharedString::from(format!("port-name/{}", port.id)))
                    .ghost()
                    .flex_1()
                    .h(px(48.))
                    .accessibility_label(format!("Edit display names for {}", port.title))
                    .tooltip(detail)
                    .child(
                        div().v_flex().w_full().gap_1().child(title).child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(port.context.clone()),
                        ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.menu_selection = keys.clone();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .w(px(24.))
                    .flex_shrink_0()
                    .h_flex()
                    .items_center()
                    .justify_center()
                    .when(port.state.as_deref() == Some("connecting"), |view| {
                        view.child(Spinner::new().small())
                    }),
            )
            .child(
                Switch::new(SharedString::from(format!("preview-switch/{}", port.id)))
                    .checked(port.is_on)
                    .disabled(true)
                    .mr_2(),
            )
            .can_drop(move |value, _, _| {
                value
                    .downcast_ref::<MenuDrag>()
                    .is_some_and(|drag| drag.kind == "port" && drag.section == section)
            })
            .drag_over::<MenuDrag>(move |view, drag, _, cx| {
                if drag.kind == "port" && drag.section == hover_section {
                    view.border_color(cx.theme().primary)
                } else {
                    view
                }
            })
            .on_drop(cx.listener(move |this, drag: &MenuDrag, _, cx| {
                this.send(
                    json!({"action":"menuMove","kind":"port","id":drag.id,"targetID":target}),
                    cx,
                )
            }))
            .into_any_element()
    }
}
