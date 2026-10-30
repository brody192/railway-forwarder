use super::{label, main_thread, rect, text};
use objc2::{MainThreadOnly, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::*;
use objc2_foundation::{NSObject, NSObjectProtocol};
use std::cell::RefCell;
thread_local! {
static PREVIEW: RefCell<Option<Retained<NSWindow>>> = const {RefCell::new(None)};
static PREVIEW_ACTIONS: RefCell<Option<Retained<PreviewActions>>> = const {RefCell::new(None)};
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind=MainThreadOnly]
    #[name="RailwayForwarderPreviewActions"]
    struct PreviewActions;
    unsafe impl NSObjectProtocol for PreviewActions {}
    impl PreviewActions {
        #[unsafe(method(openPreviewMenu:))]
        fn open_preview_menu(&self,sender:&NSView) {
            super::menu::show_in_view(sender);
        }
    }
);
pub fn preview_controls() {
    unsafe {
        let mtm = main_thread();
        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            rect(0., 0., 620., 650.),
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
            NSBackingStoreType::Buffered,
            false,
        );
        window.setReleasedWhenClosed(false);
        window.setTitle(&text("Native Liquid Glass controls"));
        let content = window.contentView().unwrap();
        let actions: Retained<PreviewActions> = msg_send![PreviewActions::alloc(mtm), init];
        let button = NSButton::buttonWithTitle_target_action(
            &text("Open native menu"),
            Some(&*actions),
            Some(sel!(openPreviewMenu:)),
            mtm,
        );
        button.setFrame(rect(20., 600., 180., 32.));
        content.addSubview(&button);
        PREVIEW_ACTIONS.with(|slot| *slot.borrow_mut() = Some(actions));
        for (y, on) in [(100., true), (45., false)] {
            let label = label(
                if on { "Connected" } else { "Disconnected" },
                13.,
                false,
                mtm,
            );
            label.setFrame(rect(20., y + 3., 200., 20.));
            content.addSubview(&label);
            let switch = macos_status_menu::GlassSwitch::new(on, mtm, |_| {});
            switch.setFrame(rect(280., y, 54., 24.));
            content.addSubview(&switch);
        }
        window.center();
        window.makeKeyAndOrderFront(None);
        NSApplication::sharedApplication(mtm).activate();
        PREVIEW.with(|slot| *slot.borrow_mut() = Some(window));
    }
}
