use crate::{Events, MenuEvent, SwitchRow, label, rect, text};
use objc2::{
    DefinedClass, MainThreadOnly, define_class, msg_send,
    rc::{Retained, Weak},
    sel,
};
use objc2_app_kit::*;
use objc2_foundation::{
    MainThreadMarker, NSObjectProtocol, NSPoint, NSRunLoop, NSRunLoopCommonModes, NSTimer,
};
use std::{
    cell::{Cell, RefCell},
    time::{Duration, Instant},
};
pub(super) struct CopyLabelIvars {
    id: String,
    value: String,
    copy_hint: String,
    menu: Weak<NSMenu>,
    events: Events,
    timer: RefCell<Option<Retained<NSTimer>>>,
    hint: RefCell<Option<Retained<NSPanel>>>,
    entered: Cell<Option<Instant>>,
    cursor_pushed: Cell<bool>,
}
define_class!(
    #[unsafe(super(NSControl))]
    #[name="RSMStatusMenuCopyLabel"]
    #[ivars=CopyLabelIvars]
    pub(super) struct CopyLabel;
    unsafe impl NSObjectProtocol for CopyLabel {}
    impl CopyLabel {
        #[unsafe(method(resetCursorRects))]
        fn reset_cursor_rects(&self) {
            unsafe {let _:()=msg_send![super(self),resetCursorRects];}
            self.addCursorRect_cursor(self.bounds(), &NSCursor::pointingHandCursor());
        }
        #[unsafe(method(cursorUpdate:))]
        fn cursor_update(&self,_event:&NSEvent) {NSCursor::pointingHandCursor().set();}
        #[unsafe(method(viewDidMoveToWindow))]
        fn view_did_move_to_window(&self) {
            unsafe {let _:()=msg_send![super(self),viewDidMoveToWindow];}
            self.stop_hover();
            if self.window().is_some() {
                unsafe {
                    let timer = NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(
                        1. / 30., self, sel!(updateHover:), None, true);
                    let run_loop = NSRunLoop::mainRunLoop();
                    run_loop.addTimer_forMode(&timer, NSEventTrackingRunLoopMode);
                    run_loop.addTimer_forMode(&timer, NSRunLoopCommonModes);
                    *self.ivars().timer.borrow_mut() = Some(timer);
                }
            }
        }
        #[unsafe(method(updateHover:))]
        fn update_hover(&self,_timer:&NSTimer) {self.poll_hover();}
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self)->bool {true}
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self,_event:&NSEvent) {self.copy();}
        #[unsafe(method(keyDown:))]
        fn key_down(&self,event:&NSEvent) {
            if event.charactersIgnoringModifiers().is_some_and(|value|matches!(value.to_string().as_str()," "|"\r")) {self.copy();}else{unsafe {let _:()=msg_send![super(self),keyDown:event];}}
        }
        #[unsafe(method(accessibilityPerformPress))]
        fn accessibility_press(&self)->bool {self.copy();true}
        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self,point:NSPoint)->Option<Retained<NSView>> {
            unsafe {let hit:Option<Retained<NSView>>=msg_send![super(self),hitTest:point];hit.and_then(|_|Retained::retain(self as *const Self as *mut NSView))}
        }
    }
);
impl CopyLabel {
    pub(super) fn new(
        row: &SwitchRow,
        menu: &NSMenu,
        events: Events,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        unsafe {
            let this = Self::alloc(mtm).set_ivars(CopyLabelIvars {
                id: row.id.clone(),
                value: row.copy_value.clone().unwrap_or_default(),
                copy_hint: row.copy_hint.clone().unwrap_or_else(|| {
                    format!(
                        "Click to copy {}",
                        row.copy_value.as_deref().unwrap_or_default()
                    )
                }),
                menu: Weak::new(menu),
                events,
                timer: RefCell::new(None),
                hint: RefCell::new(None),
                entered: Cell::new(None),
                cursor_pushed: Cell::new(false),
            });
            let this: Retained<Self> = msg_send![super(this), init];
            this.setToolTip(None);
            this.setAccessibilityHelp(Some(&text(&this.ivars().copy_hint)));
            this.setAccessibilityElement(true);
            this.setAccessibilityRole(Some(NSAccessibilityButtonRole));
            this.setAccessibilityLabel(Some(&text(&format!("Copy {}", this.ivars().value))));
            this
        }
    }
    fn copy(&self) {
        let _owner = unsafe { Retained::retain(self as *const Self as *mut Self) }.unwrap();
        let menu = self.ivars().menu.load();
        copy_value(&self.ivars().value, menu.as_deref());
        (self.ivars().events.clone())(MenuEvent::Copied {
            id: self.ivars().id.clone(),
            value: self.ivars().value.clone(),
        });
    }
    fn stop_hover(&self) {
        if let Some(timer) = self.ivars().timer.borrow_mut().take() {
            timer.invalidate();
        }
        self.hide_hint();
    }
    fn hide_hint(&self) {
        self.ivars().entered.set(None);
        if self.ivars().cursor_pushed.replace(false) {
            NSCursor::pointingHandCursor().pop();
        }
        if let Some(hint) = self.ivars().hint.borrow_mut().take() {
            hint.orderOut(None);
        }
    }
    fn poll_hover(&self) {
        let Some(window) = self.window() else {
            self.stop_hover();
            return;
        };
        let mouse = NSEvent::mouseLocation();
        let local = self.convertPoint_fromView(window.convertPointFromScreen(mouse), None);
        let bounds = self.visibleRect();
        let inside = !self.isHiddenOrHasHiddenAncestor()
            && local.x >= bounds.origin.x
            && local.x < bounds.origin.x + bounds.size.width
            && local.y >= bounds.origin.y
            && local.y < bounds.origin.y + bounds.size.height;
        if !inside {
            self.hide_hint();
            return;
        }
        if !window.areCursorRectsEnabled() {
            window.enableCursorRects();
            window.invalidateCursorRectsForView(self);
        }
        if !self.ivars().cursor_pushed.replace(true) {
            window.invalidateCursorRectsForView(self);
            NSCursor::pointingHandCursor().push();
        }
        NSCursor::pointingHandCursor().set();
        let since = self.ivars().entered.get().unwrap_or_else(|| {
            let now = Instant::now();
            self.ivars().entered.set(Some(now));
            now
        });
        if since.elapsed() >= Duration::from_millis(500) && self.ivars().hint.borrow().is_none() {
            unsafe {
                let mtm = self.mtm();
                let message = label(&self.ivars().copy_hint, 11., false, mtm);
                message.setLineBreakMode(NSLineBreakMode::ByClipping);
                message.sizeToFit();
                // Use the native text-field measurement plus padding, without a minimum width.
                let width = message.frame().size.width.ceil() + 28.;
                let height = 30.;
                let hint = NSPanel::initWithContentRect_styleMask_backing_defer(
                    NSPanel::alloc(mtm),
                    rect(0., 0., width, height),
                    NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
                    NSBackingStoreType::Buffered,
                    false,
                );
                hint.setReleasedWhenClosed(false);
                hint.setOpaque(false);
                hint.setBackgroundColor(Some(&NSColor::clearColor()));
                hint.setHasShadow(true);
                hint.setIgnoresMouseEvents(true);
                hint.setLevel(NSPopUpMenuWindowLevel + 1);
                let effect = NSVisualEffectView::initWithFrame(
                    NSVisualEffectView::alloc(mtm),
                    rect(0., 0., width, height),
                );
                effect.setMaterial(NSVisualEffectMaterial::ToolTip);
                effect.setState(NSVisualEffectState::Active);
                effect.setWantsLayer(true);
                effect.layer().unwrap().setCornerRadius(6.);
                effect.layer().unwrap().setMasksToBounds(true);
                message.setFrame(rect(12., 7., width - 24., 16.));
                effect.addSubview(&message);
                hint.setContentView(Some(&effect));
                let mut origin = NSPoint::new(mouse.x + 12., mouse.y - height - 16.);
                if let Some(screen) = window.screen() {
                    let screen = screen.visibleFrame();
                    origin.x = origin.x.clamp(
                        screen.origin.x + 4.,
                        (screen.origin.x + screen.size.width - width - 4.)
                            .max(screen.origin.x + 4.),
                    );
                    origin.y = origin.y.clamp(
                        screen.origin.y + 4.,
                        screen.origin.y + screen.size.height - height - 4.,
                    );
                }
                hint.setFrameOrigin(origin);
                hint.orderFrontRegardless();
                *self.ivars().hint.borrow_mut() = Some(hint);
            }
        }
    }
}

pub(crate) fn copy_value(value: &str, menu: Option<&NSMenu>) {
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    unsafe {
        pasteboard.setString_forType(&text(value), NSPasteboardTypeString);
    }
    if let Some(menu) = menu {
        menu.cancelTracking();
    }
}
