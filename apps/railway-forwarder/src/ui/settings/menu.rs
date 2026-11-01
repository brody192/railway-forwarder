use crate::ui::controls::button;
use crate::{
    domain::{MenuPort, MenuSection},
    ui::settings::{SettingsView, Tab, inputs::input_boundary, style},
};
use gpui_kit::{
    assets::IconName,
    base::StyledExt,
    component::{ActiveTheme, Sizable, button::ButtonVariants, input::Input},
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
        let wide = window.viewport_size().width >= px(900.);
        let mut preview = div()
            .v_flex()
            .flex_shrink_0()
            .w_full()
            .max_w(px(350.))
            .py_2()
            .rounded_xl()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            // Clicks within either editor panel retain the current selection.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        preview = preview.child(self.preview_header(menu.header.clone(), cx));
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
            preview = preview.child(self.section_heading(section, window, cx));
            for port in &section.ports {
                preview = preview.child(self.preview_port(port, window, cx));
            }
        }

        let has_selection = self.menu_selection.iter().any(|key| {
            self.snapshot
                .display_names
                .iter()
                .any(|label| &label.key == key)
        });
        let mut inspector = div()
            .v_flex()
            .gap_4()
            .w_full()
            .min_w_0()
            .p_4()
            .rounded_xl()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        if !has_selection {
            inspector = inspector.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Select a project or service to edit its names."),
            );
        }
        let names = self.snapshot.display_names.clone();
        for key in self.menu_selection.clone() {
            if let Some(label) = names.iter().find(|label| label.key == key) {
                let input = self.name_field(label, window, cx);
                let mut label_row = div().h_flex().gap_2().items_center().child(
                    div()
                        .flex_1()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .child(label.label.clone()),
                );
                if label.custom {
                    label_row = label_row.child(
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
                inspector = inspector.child(
                    div().v_flex().gap_2().child(label_row).child(
                        input_boundary(
                            &input,
                            Input::new(&input)
                                .h(px(28.))
                                .aria_label(format!("{} display name", label.label)),
                            cx,
                        )
                        .w_full()
                        .min_w_0(),
                    ),
                );
            }
        }
        div()
            .v_flex()
            .gap_5()
            .h_full()
            .min_h_0()
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .items_center()
                    .child(style::page_heading(Tab::Menu).flex_1())
                    .child(
                        button("reset-menu")
                            .ghost()
                            .h(px(30.))
                            .rounded_full()
                            .label("Reset")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.reset_menu(window, cx);
                            })),
                    )
                    .child(
                        button("open-native-menu")
                            .label("Open menu")
                            .h(px(30.))
                            .rounded_full()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.send(json!({"action":"showMenu"}), cx)
                            })),
                    ),
            )
            .child(
                div()
                    .id("menu-editor-scroll")
                    .v_flex()
                    .when(wide, |view| view.flex_row())
                    .items_start()
                    .gap_5()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .v_flex()
                            .gap_3()
                            .w_full()
                            .max_w(px(350.))
                            .flex_shrink_0()
                            .child(style::section_label("Menu preview", cx))
                            .child(preview),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_3()
                            .w_full()
                            .min_w_0()
                            .when(wide, |view| view.w(px(0.)).flex_1())
                            .child(style::section_label("Display names", cx))
                            .child(inspector),
                    ),
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

    fn preview_header(&self, title: String, cx: &App) -> AnyElement {
        div()
            .w_full()
            .px_4()
            .py_2()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cx.theme().muted_foreground)
            .child(title)
            .into_any_element()
    }

    fn section_heading(
        &self,
        section: &MenuSection,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
            .rounded(px(6.))
            .child(handle(drag))
            .child(
                button(SharedString::from(format!("section-name/{}", section.id)))
                    .ghost()
                    .flex_1()
                    .bg(style::selection_fill(
                        self.menu_selection == section.name_keys,
                        window,
                        cx,
                    ))
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

    fn preview_port(&self, port: &MenuPort, window: &Window, cx: &mut Context<Self>) -> AnyElement {
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
            .rounded(px(6.))
            .child(handle(drag))
            .child(
                button(SharedString::from(format!("port-name/{}", port.id)))
                    .ghost()
                    .flex_1()
                    .h(px(48.))
                    .bg(style::selection_fill(
                        self.menu_selection == port.name_keys,
                        window,
                        cx,
                    ))
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
