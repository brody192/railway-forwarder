use macos_status_menu::{Action, Menu, MenuEvent, Section, StatusItem, StatusMenu, SwitchRow};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::MainThreadMarker;

fn main() {
    let mtm = MainThreadMarker::new().expect("Run on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let menu = StatusMenu::new(
        StatusItem::new("SDK example").symbol("network"),
        mtm,
        move |event| match event {
            MenuEvent::Action(id) if id == "quit" => {
                NSApplication::sharedApplication(mtm).terminate(None)
            }
            MenuEvent::Toggle { id, on } => println!("{id}: {on}"),
            MenuEvent::Copied { value, .. } => println!("Copied {value}"),
            _ => {}
        },
    );
    menu.set_menu(
        Menu::new("SDK example")
            .item(Section::new("Development").detail("Local"))
            .item(
                SwitchRow::new("web", "Web · 8080")
                    .subtitle("development")
                    .copy_value("http://127.0.0.1:8080"),
            )
            .separator()
            .item(Action::new("quit", "Quit")),
    );
    app.run();
}
