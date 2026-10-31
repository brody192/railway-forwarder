//! Main-thread macOS adapters. The reusable controls live in macos-status-menu.
pub(crate) mod login_item;
mod menu;
mod oauth;
mod preview;
pub(crate) use menu::{initialize, render, show_menu};
pub(crate) use oauth::{authenticate, cancel_auth, finish_auth};
use objc2::rc::Retained;
use objc2_app_kit::*;
use objc2_foundation::{MainThreadMarker, NSBundle, NSNumber, NSPoint, NSRect, NSSize, NSString};
pub(crate) use preview::preview_controls;
fn main_thread() -> MainThreadMarker {
    MainThreadMarker::new().expect("AppKit is used on GPUI's main thread")
}
fn text(value: &str) -> Retained<NSString> {
    NSString::from_str(value)
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}
fn label(value: &str, size: f64, _bold: bool, mtm: MainThreadMarker) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&text(value), mtm);
    label.setFont(Some(&NSFont::systemFontOfSize(size)));
    label
}
pub fn demo_mode() -> bool {
    info_flag("ForwarderDemo")
}
pub fn controls_preview_mode() -> bool {
    info_flag("ForwarderNativeControlsPreview")
}
fn info_flag(key: &str) -> bool {
    NSBundle::mainBundle()
        .objectForInfoDictionaryKey(&text(key))
        .and_then(|value| value.downcast::<NSNumber>().ok())
        .is_some_and(|value| value.boolValue())
}
