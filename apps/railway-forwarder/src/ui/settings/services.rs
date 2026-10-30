use crate::domain::{Environment, Port, Project, Service, Workspace, preferred_ports};
use crate::ui::controls::{ButtonCursorExt, button};
use gpui_kit::{assets::IconName, base::StyledExt, prelude::FluentBuilder};
use gpui_kit::{
    component::{
        ActiveTheme, Sizable, button::ButtonVariants, checkbox::Checkbox, input::Input,
        spinner::Spinner,
    },
    *,
};
use serde_json::{Value, json};

use super::{SettingsView, inputs::input_boundary};

const PORT_CONTENT_SPACING: f32 = 14.;

impl SettingsView {
    pub(super) fn disclosure(
        &self,
        id: String,
        label: String,
        icon: IconName,
        level: usize,
        command: Option<Value>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rendered_open = self.disclosures.contains(&id);
        let loading = rendered_open
            && id
                .strip_prefix("p/")
                .or_else(|| id.strip_prefix("s/"))
                .is_some_and(|key| self.snapshot.loading.iter().any(|loading| loading == key));
        let chevron = if rendered_open {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };
        div()
            .id(SharedString::from(format!("disclosure-row/{id}")))
            .h_flex()
            .items_center()
            .gap_2()
            .pl(px(level as f32 * 22.))
            .py_1()
            .child(
                button(SharedString::from(id.clone()))
                    .ghost()
                    .accessibility_label(label.clone())
                    .child(
                        div()
                            .h_flex()
                            .w_full()
                            .gap_2()
                            .child(gpui_kit::component::Icon::new(chevron).size_4())
                            .child(
                                gpui_kit::component::Icon::new(icon)
                                    .size_4()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(label)
                            .when(loading, |row| row.child(Spinner::new().small())),
                    )
                    .flex_1()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // Apply the intent from the displayed row. Repeated
                        // activations of that frame must not invert newer state.
                        if this.set_disclosure_open(&id, !rendered_open, cx) && !rendered_open {
                            if let Some(command) = command.clone() {
                                this.send(command, cx);
                            }
                        }
                    })),
            )
            .into_any_element()
    }
    pub(super) fn error(
        &self,
        key: &str,
        retry: Option<Value>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.snapshot.node_errors.get(key).map(|message| {
            let mut row = div()
                .h_flex()
                .w_full()
                .min_w_0()
                .gap_3()
                .items_center()
                .px_3()
                .py_2()
                .text_sm()
                .text_color(cx.theme().danger)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .whitespace_normal()
                        .child(message.clone()),
                );
            if let Some(command) = retry {
                row = row.child(
                    button(SharedString::from(format!("retry/{key}")))
                        .small()
                        .label("Retry")
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.send(command.clone(), cx)),
                        ),
                );
            }
            row.into_any_element()
        })
    }
    pub(super) fn tree(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut list = div().v_flex().flex_shrink_0().gap_1();
        let latest = self.filtered_catalog();
        let workspaces = self
            .search_results
            .display(|| latest, &self.snapshot.workspaces);

        if (self.selected_only || !self.query.is_empty())
            && workspaces.is_empty()
            && !self.snapshot.loading.contains(&"search".into())
            && !self.snapshot.node_errors.contains_key("search")
        {
            let message = if self.selected_only && self.query.is_empty() {
                "No ports selected"
            } else if self.selected_only {
                "No matching selected ports"
            } else {
                "No matching services"
            };
            list = list.child(self.note(message, 0, cx));
        }
        for workspace in workspaces {
            let key = format!("w/{}", workspace.id);
            list = list.child(self.disclosure(
                key.clone(),
                workspace.name.clone(),
                IconName::Layers,
                0,
                None,
                cx,
            ));
            if self.disclosures.contains(&key) {
                if workspace.projects.is_empty() {
                    list = list.child(self.note("No projects", 1, cx));
                }
                for project in &workspace.projects {
                    let key = format!("p/{}", project.id);
                    let command = json!({"action":"loadProject", "workspaceID":workspace.id, "projectID":project.id});
                    let load = if self.snapshot.loaded_projects.contains(&project.id) {
                        None
                    } else {
                        Some(command.clone())
                    };
                    list = list.child(self.disclosure(
                        key.clone(),
                        project.name.clone(),
                        IconName::Folder,
                        1,
                        load,
                        cx,
                    ));
                    if self.disclosures.contains(&key) {
                        if let Some(error) = self.error(&project.id, Some(command), cx) {
                            list = list.child(error);
                        }
                        if self.snapshot.loaded_projects.contains(&project.id)
                            && project.environments.is_empty()
                            && self
                                .snapshot
                                .workspaces
                                .iter()
                                .flat_map(|workspace| &workspace.projects)
                                .find(|source| source.id == project.id)
                                .is_some_and(|source| source.environments.is_empty())
                        {
                            list = list.child(self.note("No environments", 2, cx));
                        }
                        for environment in &project.environments {
                            let key = format!("e/{}/{}", project.id, environment.id);
                            list = list.child(self.disclosure(
                                key.clone(),
                                environment.name.clone(),
                                IconName::Circle,
                                2,
                                None,
                                cx,
                            ));
                            if self.disclosures.contains(&key) {
                                if environment.services.is_empty() {
                                    list = list.child(self.note("No services", 3, cx));
                                }
                                for service in &environment.services {
                                    list = list.child(self.service(
                                        &workspace,
                                        project,
                                        environment,
                                        service,
                                        window,
                                        cx,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        list.into_any_element()
    }
    pub(super) fn note(&self, text: &str, level: usize, cx: &App) -> AnyElement {
        div()
            .pl(px(14. + level as f32 * 22.))
            .py_2()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(text.to_owned())
            .into_any_element()
    }
    pub(super) fn service(
        &mut self,
        workspace: &Workspace,
        project: &Project,
        environment: &Environment,
        service: &Service,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = format!("{}/{}", environment.id, service.id);
        let expand_key = format!("s/{key}");
        let context = json!({"workspaceID":workspace.id, "projectID":project.id, "environmentID":environment.id, "serviceID":service.id});
        let mut scan = context.clone();
        scan["action"] = "scanService".into();
        let load = if self.selected_only || self.snapshot.scanned.contains(&key) {
            None
        } else {
            Some(scan.clone())
        };
        let mut column = div().v_flex().child(self.disclosure(
            expand_key.clone(),
            service.name.clone(),
            IconName::Server,
            3,
            load,
            cx,
        ));
        if !self.disclosures.contains(&expand_key) {
            return column.into_any_element();
        }
        let busy = !self.selected_only && self.snapshot.loading.contains(&key);
        let mut body = div()
            .v_flex()
            .gap_2()
            .ml(px(100.))
            .mr_3()
            .mb_2()
            .pt_2()
            .pl(px(15.))
            .pr(px(10.))
            .pb(px(PORT_CONTENT_SPACING))
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg();
        if busy {
            body = body.child(
                div()
                    .py_1()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Scanning ports…"),
            );
        }
        if !self.selected_only {
            if let Some(error) = self.error(&key, None, cx) {
                body = body.child(error);
            }
        }
        let mut ports = self.snapshot.ports.get(&key).cloned().unwrap_or_default();
        for allowed in &self.snapshot.preferences.allowed {
            if allowed.project_id == project.id
                && allowed.environment_id == environment.id
                && allowed.service_id == service.id
                && !ports.contains(&allowed.remote)
            {
                ports.push(allowed.remote.clone());
            }
        }
        if self.selected_only {
            ports.retain(|port| {
                self.snapshot.preferences.allowed.iter().any(|allowed| {
                    allowed.project_id == project.id
                        && allowed.environment_id == environment.id
                        && allowed.service_id == service.id
                        && allowed.remote == *port
                })
            });
        }
        let ports = preferred_ports(ports);
        if ports.is_empty() && !busy && self.snapshot.scanned.contains(&key) {
            body = body.child(
                div()
                    .py_1()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("No listening TCP ports"),
            );
        }
        for port in ports {
            body = body.child(self.port_row(
                &context,
                project,
                environment,
                service,
                &port,
                window,
                cx,
            ));
        }
        if self.selected_only {
            return column.child(body.pb_2()).into_any_element();
        }
        let input = self.field(
            format!("manual/{key}"),
            String::new(),
            "Port",
            None,
            window,
            cx,
        );
        let input_for_click = input.clone();
        let mut manual = context.clone();
        manual["action"] = "addManual".into();
        let mut controls = div()
            .h_flex()
            .items_center()
            .gap(px(PORT_CONTENT_SPACING))
            .border_t_1()
            .border_color(cx.theme().border)
            .pt(px(PORT_CONTENT_SPACING))
            .child(
                input_boundary(
                    &input,
                    Input::new(&input).small().aria_label("Manual remote port"),
                    cx,
                )
                .w(px(94.)),
            )
            .child(
                button(SharedString::from(format!("add/{key}")))
                    .small()
                    .label("Add port")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let mut command = manual.clone();
                        command["value"] = input_for_click.read(cx).value().to_string().into();
                        this.send(command, cx);
                        if !this.snapshot.node_errors.contains_key(&key) {
                            input_for_click.update(cx, |state, cx| state.set_value("", window, cx));
                        }
                    })),
            );
        controls = controls.child(div().flex_1()).child(
            button(SharedString::from(format!(
                "scan/{}/{}",
                environment.id, service.id
            )))
            .small()
            .ghost()
            .label("Rescan")
            .disabled_with_cursor(busy)
            .on_click(cx.listener(move |this, _, _, cx| this.send(scan.clone(), cx))),
        );
        column = column.child(body.child(controls));
        column.into_any_element()
    }
    pub(super) fn port_row(
        &mut self,
        context: &Value,
        project: &Project,
        environment: &Environment,
        service: &Service,
        port: &Port,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selections: Vec<_> = self
            .snapshot
            .preferences
            .allowed
            .iter()
            .filter(|allowed| {
                allowed.project_id == project.id
                    && allowed.environment_id == environment.id
                    && allowed.service_id == service.id
                    && (allowed.remote == *port
                        || (port.host == "::1" && allowed.remote.port == port.port))
            })
            .cloned()
            .collect();
        let selection = selections.first().cloned();
        let id = format!(
            "{}/{}/{}/{}:{}",
            project.id, environment.id, service.id, port.host, port.port
        );
        let mut command = context.clone();
        command["action"] = "allowPort".into();
        command["port"] = json!(port);
        let field_id = format!("local/{id}");
        let mut row = div()
            .h_flex()
            .items_center()
            .gap(px(PORT_CONTENT_SPACING))
            .min_h(px(28.))
            .child(
                Checkbox::new(SharedString::from(format!("allow/{id}")))
                    .cursor_pointer()
                    .label(port.port.to_string())
                    .checked(selection.is_some())
                    .on_change(cx.listener(move |this, checked: &bool, _, cx| {
                        if !checked {
                            this.inputs.remove(&field_id);
                            // One visible IPv6 row can represent saved listeners
                            // on several addresses. Clear all of its selections.
                            for selection in &selections {
                                let mut command = command.clone();
                                command["port"] = json!(selection.remote);
                                command["enabled"] = false.into();
                                this.send(command, cx);
                            }
                            return;
                        }
                        let mut command = command.clone();
                        command["enabled"] = (*checked).into();
                        this.send(command, cx);
                    })),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(port.host.clone()),
            )
            .child(div().flex_1());
        if let Some(selection) = selection {
            let input = self.field(
                format!("local/{id}"),
                selection.local_port.to_string(),
                "Local port",
                Some(json!({"action":"localPort", "id":selection.id()})),
                window,
                cx,
            );
            row = row
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("Local port"),
                )
                .child(
                    input_boundary(
                        &input,
                        Input::new(&input)
                            .small()
                            .aria_label(format!("Local port for {} {}", service.name, port.port)),
                        cx,
                    )
                    .w(px(90.)),
                );
        }
        row.into_any_element()
    }
}
