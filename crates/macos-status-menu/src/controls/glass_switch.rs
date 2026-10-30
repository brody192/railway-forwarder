use crate::rect;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::*;
use objc2_foundation::{MainThreadMarker, NSNumber, NSObjectProtocol, NSPoint};
use std::{cell::Cell, rc::Rc};
/// A reusable native Liquid Glass switch. It dereferences to `NSControl` for AppKit integration.
pub struct GlassSwitch {
    control: Retained<GlassSwitchControl>,
}
impl GlassSwitch {
    pub fn new(on: bool, mtm: MainThreadMarker, on_change: impl Fn(bool) + 'static) -> Self {
        Self {
            control: GlassSwitchControl::new(on, mtm, on_change),
        }
    }
    pub fn set_on(&self, on: bool) {
        self.control.set_on(on);
    }
    pub fn is_on(&self) -> bool {
        self.control.is_on()
    }
}
impl std::ops::Deref for GlassSwitch {
    type Target = NSControl;
    fn deref(&self) -> &NSControl {
        &self.control
    }
}
struct SwitchIvars {
    on_change: Rc<dyn Fn(bool)>,
    on: Cell<bool>,
    thumb: Retained<NSGlassEffectView>,
}
define_class!(
    #[unsafe(super(NSControl))]
    #[name="RSMStatusMenuGlassSwitch"]
    #[ivars=SwitchIvars]
    struct GlassSwitchControl;
    unsafe impl NSObjectProtocol for GlassSwitchControl {}
    impl GlassSwitchControl {
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self)->bool {true}
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self,_event:&NSEvent) {self.toggle();}
        #[unsafe(method(keyDown:))]
        fn key_down(&self,event:&NSEvent) {
            if event.charactersIgnoringModifiers().is_some_and(|value|matches!(value.to_string().as_str()," "|"\r")) {self.toggle();}else{unsafe {let _:()=msg_send![super(self),keyDown:event];}}
        }
        #[unsafe(method(accessibilityPerformPress))]
        fn accessibility_press(&self)->bool {self.toggle();true}
        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self,point:NSPoint)->Option<Retained<NSView>> {
            unsafe {let hit:Option<Retained<NSView>>=msg_send![super(self),hitTest:point];hit.and_then(|_|Retained::retain(self as *const Self as *mut NSView))}
        }
    }
);
impl GlassSwitchControl {
    /// Creates a standalone native Liquid Glass switch with a main-thread callback.
    pub fn new(
        on: bool,
        mtm: MainThreadMarker,
        on_change: impl Fn(bool) + 'static,
    ) -> Retained<Self> {
        unsafe {
            let thumb = NSGlassEffectView::initWithFrame(
                NSGlassEffectView::alloc(mtm),
                rect(if on { 20. } else { 2. }, 2., 32., 20.),
            );
            thumb.setCornerRadius(10.);
            thumb.setStyle(NSGlassEffectViewStyle::Regular);
            thumb.setTintColor(Some(&NSColor::whiteColor()));
            if thumb.respondsToSelector(sel!(setEffectIsInteractive:)) {
                let _: () = msg_send![&*thumb,setEffectIsInteractive:true];
            }
            let fill = NSView::initWithFrame(NSView::alloc(mtm), rect(0., 0., 32., 20.));
            fill.setWantsLayer(true);
            let layer = fill.layer().unwrap();
            layer.setCornerRadius(10.);
            layer.setMasksToBounds(true);
            layer.setBackgroundColor(Some(&NSColor::whiteColor().CGColor()));
            thumb.setContentView(Some(&fill));
            let this = Self::alloc(mtm).set_ivars(SwitchIvars {
                on_change: Rc::new(on_change),
                on: Cell::new(on),
                thumb,
            });
            let this: Retained<Self> = msg_send![super(this),initWithFrame:rect(0.,0.,54.,24.)];
            this.setWantsLayer(true);
            let layer = this.layer().unwrap();
            layer.setCornerRadius(12.);
            this.addSubview(&this.ivars().thumb);
            this.setAccessibilityElement(true);
            this.setAccessibilityRole(Some(NSAccessibilityCheckBoxRole));
            this.setAccessibilitySubrole(Some(NSAccessibilitySwitchSubrole));
            this.paint();
            this
        }
    }
    /// Changes the visual value without emitting a toggle event.
    pub fn set_on(&self, on: bool) {
        self.ivars().on.set(on);
        self.paint();
    }
    pub fn is_on(&self) -> bool {
        self.ivars().on.get()
    }
    fn paint(&self) {
        unsafe {
            let on = self.ivars().on.get();
            let color = if on {
                NSColor::colorWithSRGBRed_green_blue_alpha(0., 122. / 255., 1., 1.)
            } else {
                NSColor::secondaryLabelColor().colorWithAlphaComponent(0.3)
            };
            self.layer()
                .unwrap()
                .setBackgroundColor(Some(&color.CGColor()));
            self.ivars()
                .thumb
                .setFrame(rect(if on { 20. } else { 2. }, 2., 32., 20.));
            self.setAccessibilityValue(Some(&NSNumber::numberWithBool(on)));
        }
    }
    fn toggle(&self) {
        if self.isEnabled() {
            let on = !self.ivars().on.get();
            self.ivars().on.set(on);
            self.paint();
            let _owner = unsafe { Retained::retain(self as *const Self as *mut Self) }.unwrap();
            (self.ivars().on_change.clone())(on);
        }
    }
}
