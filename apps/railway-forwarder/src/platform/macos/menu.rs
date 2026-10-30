use super::main_thread;
use crate::{app::events, domain::Snapshot};
use macos_status_menu::{Action, Menu, MenuEvent, Section, StatusItem, StatusMenu, SwitchRow};
use objc2_app_kit::*;
use objc2_foundation::NSPoint;
use std::cell::RefCell;
thread_local! {static MENU: RefCell<Option<StatusMenu>> = const {RefCell::new(None)};}
pub fn initialize() {
    let menu = StatusMenu::new(
        StatusItem::new("Railway Forwarder").symbol("point.3.filled.connected.trianglepath.dotted"),
        main_thread(),
        |event| match event {
            MenuEvent::WillOpen => events::render_menu(),
            MenuEvent::Action(id) => match id.as_str() {
                "settings" | "sign-in" | "select-services" => events::open_settings(),
                "disconnect" => events::dispatch(serde_json::json!({"action":"disconnect"})),
                "quit" => {
                    events::shutdown();
                    NSApplication::sharedApplication(main_thread()).terminate(None);
                }
                _ => {}
            },
            MenuEvent::Toggle { id, on } => {
                events::dispatch(serde_json::json!({"action":"forwardPort","id":id,"enabled":on}))
            }
            MenuEvent::Copied { .. } => {}
        },
    );
    MENU.with(|slot| *slot.borrow_mut() = Some(menu));
}
fn menu_model(snapshot: &Snapshot) -> (StatusItem, Menu) {
    let layout = &snapshot.menu;
    let mut menu = Menu::new(&layout.header);
    if !snapshot.signed_in {
        menu = menu.item(Action::new("sign-in", "Sign in with Railway"));
    } else if layout.sections.is_empty() {
        menu = menu.item(Action::new("select-services", "Choose services"));
    } else {
        for (index, section) in layout.sections.iter().enumerate() {
            if index > 0 {
                menu = menu.separator();
            }
            menu = menu.item(Section::new(&section.project_name).detail(&section.workspace_name));
            for port in &section.ports {
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
                let help = if port.state.as_deref() == Some("connecting") {
                    format!("Connecting\n{}\n{detail}", port.context)
                } else {
                    format!("{}\n{detail}", port.context)
                };
                menu = menu.item(
                    SwitchRow::new(&port.id, title)
                        .subtitle(&port.context)
                        .on(port.is_on)
                        .busy(port.state.as_deref() == Some("connecting"))
                        .copy_value(format!("127.0.0.1:{}", port.port.local_port))
                        .toggle_label(format!("Forward {}", port.title))
                        .help(help),
                );
            }
        }
    }
    let active = layout
        .sections
        .iter()
        .flat_map(|section| &section.ports)
        .any(|port| port.is_on);
    menu = menu
        .separator()
        .item(Action::new("disconnect", &layout.disconnect).enabled(active))
        .separator()
        .item(Action::new("settings", &layout.settings))
        .item(Action::new("quit", &layout.quit));
    let count = layout
        .sections
        .iter()
        .flat_map(|section| &section.ports)
        .filter(|port| port.state.as_deref() == Some("connected"))
        .count();
    let symbol = if count > 0 {
        "point.3.connected.trianglepath.dotted"
    } else {
        "point.3.filled.connected.trianglepath.dotted"
    };
    (
        StatusItem::new(format!("Railway Forwarder, {count} active tunnels")).symbol(symbol),
        menu,
    )
}
pub fn render(snapshot: &Snapshot) {
    let (status, model) = menu_model(snapshot);
    MENU.with(|slot| {
        if let Some(menu) = slot.borrow().as_ref() {
            menu.set_status(status);
            menu.set_menu(model);
        }
    });
}
pub fn show_menu() {
    MENU.with(|slot| {
        if let Some(menu) = slot.borrow().as_ref() {
            menu.show();
        }
    });
}

pub(super) fn show_in_view(sender: &NSView) {
    MENU.with(|slot| {
        if let Some(menu) = slot.borrow().as_ref() {
            menu.show_in_view(sender, NSPoint::new(0., -8.));
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    use macos_status_menu::Item;
    #[test]
    fn sdk_menu_keeps_app_actions_and_uses_the_local_port_for_copying() {
        let backend = crate::app::backend::Backend::new(true).unwrap();
        let first = backend.snapshot().preferences.allowed[0].id();
        backend.dispatch(serde_json::json!({"action":"localPort","id":first,"value":"15432"}));
        let snapshot = backend.snapshot();
        let (status, menu) = menu_model(&snapshot);
        assert_eq!(menu.title, "Railway Forwarder");
        let rows: Vec<_> = menu
            .items
            .iter()
            .filter_map(|item| {
                if let Item::SwitchRow(row) = item {
                    Some(row)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows.iter()
                .find(|row| row.id == first)
                .unwrap()
                .copy_value
                .as_deref(),
            Some("127.0.0.1:15432")
        );
        let actions: Vec<_> = menu
            .items
            .iter()
            .filter_map(|item| {
                if let Item::Action(action) = item {
                    Some((action.id.as_str(), action.label.as_str()))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            actions,
            [
                ("disconnect", "Disconnect"),
                ("settings", "Settings"),
                ("quit", "Quit")
            ]
        );
        assert!(status.label.contains("0 active tunnels"));
        assert!(snapshot.preferences.menu.names.is_empty());

        let title = rows
            .iter()
            .find(|row| row.id == first)
            .unwrap()
            .title
            .clone();
        for (state, busy, on) in [
            (crate::domain::TunnelState::Connecting, true, true),
            (crate::domain::TunnelState::Connected, false, true),
            (
                crate::domain::TunnelState::Failed("Connection failed".into()),
                false,
                false,
            ),
        ] {
            backend.tunnel_state(&first, Some(state));
            let (_, menu) = menu_model(&backend.snapshot());
            let row = menu
                .items
                .iter()
                .find_map(|item| match item {
                    Item::SwitchRow(row) if row.id == first => Some(row),
                    _ => None,
                })
                .unwrap();
            assert_eq!(row.busy, busy);
            assert_eq!(row.on, on);
            assert!(row.enabled);
            assert_eq!(
                row.title,
                if on {
                    title.clone()
                } else {
                    format!("{title} (Failed)")
                }
            );
        }
    }
}
