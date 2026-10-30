# macos-status-menu

A Rust SDK for native AppKit menu bar menus with Liquid Glass switches, section headings, copyable rows, pointing-hand cursors and tooltips sized to their text. It does not depend on Railway, GPUI, an async runtime or application globals.

Requires macOS 26 or later and a host executable built against macOS SDK 26 or later. The interactive glass effect is enabled when supported by the SDK and runtime, including macOS 27.

Use it from another application with a path dependency:

```toml
[dependencies]
macos-status-menu = { path = "/path/to/railway forwarder/crates/macos-status-menu" }
```

```rust,no_run
use macos_status_menu::{
    Action, MainThreadMarker, Menu, MenuEvent, Section, StatusItem, StatusMenu, SwitchRow,
};

let mtm = MainThreadMarker::new().expect("Run on the main thread");
let status = StatusMenu::new(StatusItem::new("Example").symbol("network"), mtm, |event| {
    match event {
        MenuEvent::Toggle { id, on } => println!("{id}: {on}"),
        MenuEvent::Action(id) => println!("Command: {id}"),
        _ => {}
    }
});
status.set_menu(Menu::new("Example")
    .item(Section::new("Development").detail("Local"))
    .item(SwitchRow::new("web", "Web · 8080")
        .subtitle("development")
        .copy_value("http://127.0.0.1:8080"))
    .separator()
    .item(Action::new("quit", "Quit")));
// Keep `status` alive while your AppKit-compatible event loop runs.
```

`Menu`, `Section`, `Action` and `SwitchRow` are ordinary Rust descriptions. The SDK renders them into native `NSMenu` items and views. Actions have no automatic icon or keyboard shortcut. Item IDs must be unique within each menu. Section details appear right aligned, while port-style rows remain at the top level.

`StatusMenu` must be created, updated and dropped on the AppKit main thread. The caller owns the application and its event loop, whether AppKit or GPUI. Each instance owns an independent status item, menu and event callback. `set_menu` replaces its description and switch values, `set_status` changes the icon and accessibility label, and `show` opens it. `show_in_view` anchors the same native menu to an existing AppKit view for previews. Dropping the owner dismisses its menu and removes the status item.

Callbacks run synchronously on the main thread. Handle `Toggle { id, on }` in your own state and supply the resulting state with `set_menu`. The switch responds visually immediately; the supplied model determines its value when rebuilt or reopened. Handle commands such as Quit yourself. `WillOpen` lets you refresh state before display. A copyable row writes its value to the system clipboard, closes the menu and emits `Copied`.

Use `SwitchRow::busy(true)` to show a native indeterminate spinner beside the switch. The title and switch remain available, and the row reserves space so loading does not move its text. Set it to `false` when work completes or fails.

`GlassSwitch` is also available as a standalone native control. It dereferences to `NSControl`, so it can be positioned and added to an existing AppKit view. `set_on` changes its value without emitting a callback. This supports native control previews without constructing a status menu.

The standalone example supplies its own AppKit event loop. From the workspace root, stamp the example executable with the selected SDK version to enable Liquid Glass:

```sh
cargo rustc -p macos-status-menu --example status-menu -- \
  -C "link-arg=-Wl,-platform_version,macos,26.0,$(xcrun --sdk macosx --show-sdk-version)"
./target/debug/examples/status-menu
```

The example logs toggle and copy events. A full application should persist toggle state and update its menu model. Railway Forwarder's build script applies the same SDK stamping to its executable.
