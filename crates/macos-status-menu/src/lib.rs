//! Native AppKit status menus with Liquid Glass switches, copyable rows and hover feedback.
//!
//! This crate owns no application state or event loop. Create and update `StatusMenu`
//! on the AppKit main thread, and translate `MenuEvent`s into your own state updates.
//! Requires macOS 26 or later. Build the host executable against SDK 26 or later;
//! the interactive glass effect is used when SDK/runtime 27 supports it.
//!
//! ```no_run
//! use macos_status_menu::{Action, Menu, MenuEvent, Section, StatusItem, StatusMenu, SwitchRow};
//! use macos_status_menu::MainThreadMarker;
//! let mtm = MainThreadMarker::new().expect("main thread");
//! let menu = StatusMenu::new(StatusItem::new("Example").symbol("network"), mtm, |event| {
//!     if let MenuEvent::Toggle {id, on} = event {println!("{id}: {on}");}
//! });
//! menu.set_menu(Menu::new("Example")
//!     .item(Section::new("Development").detail("Local"))
//!     .item(SwitchRow::new("web", "Web · 8080").subtitle("development").copy_value("http://127.0.0.1:8080"))
//!     .separator().item(Action::new("quit", "Quit")));
//! // Keep `menu` alive while your AppKit-compatible event loop runs.
//! ```

#[cfg(not(target_os = "macos"))]
compile_error!("macos-status-menu requires macOS");
mod controls;
mod menu;
mod model;
pub use controls::GlassSwitch;
pub use menu::StatusMenu;
pub use model::*;
use objc2::rc::Retained;
use objc2_app_kit::{NSFont, NSLineBreakMode, NSTextField};
use objc2_foundation::NSString;
pub use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};
use std::rc::Rc;
type Events = Rc<dyn Fn(MenuEvent)>;
fn text(value: &str) -> Retained<NSString> {
    NSString::from_str(value)
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}
fn label(value: &str, size: f64, bold: bool, mtm: MainThreadMarker) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&text(value), mtm);
    let font = if bold {
        NSFont::boldSystemFontOfSize(size)
    } else {
        NSFont::systemFontOfSize(size)
    };
    label.setFont(Some(&font));
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
    label
}
