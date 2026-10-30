use crate::controls::{ROW_TEXT_RESERVE, copy_value, row_view};
use crate::{Action, Events, Item, Menu, MenuEvent, Section, StatusItem, label, rect, text};
use objc2::{
    DefinedClass, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::*;
use objc2_foundation::{
    MainThreadMarker, NSObject, NSObjectNSDelayedPerforming, NSObjectProtocol, NSPoint, NSString,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
/// Owns one status item and its native menu. It is intentionally main-thread-only.
/// Dropping it removes the status item, dismisses the menu and releases hover resources.
pub struct StatusMenu {
    controller: Retained<Controller>,
}
impl StatusMenu {
    /// Installs a status item and delivers events synchronously on the main thread.
    pub fn new(
        status: StatusItem,
        mtm: MainThreadMarker,
        on_event: impl Fn(MenuEvent) + 'static,
    ) -> Self {
        let controller = Controller::new(mtm, Rc::new(on_event));
        controller.set_status(&status);
        Self { controller }
    }
    /// Replaces the menu description, including the current values of its switches.
    pub fn set_menu(&self, menu: Menu) {
        *self.controller.ivars().model.borrow_mut() = menu;
        self.controller.rebuild();
    }
    /// Updates the menu bar icon and accessibility label.
    pub fn set_status(&self, status: StatusItem) {
        self.controller.set_status(&status);
    }
    /// Activates the application and schedules native menu tracking.
    pub fn show(&self) {
        self.controller.request_menu();
    }
    /// Presents the same menu anchored in another native view, useful for previews.
    pub fn show_in_view(&self, view: &NSView, point: NSPoint) {
        self.controller
            .ivars()
            .menu
            .popUpMenuPositioningItem_atLocation_inView(None, point, Some(view));
    }
}
impl Drop for StatusMenu {
    fn drop(&mut self) {
        let controller = &self.controller;
        controller.ivars().disposed.set(true);
        unsafe {
            NSObject::cancelPreviousPerformRequestsWithTarget(controller);
        }
        controller.ivars().menu.cancelTracking();
        controller.ivars().menu.removeAllItems();
        controller.ivars().menu.setDelegate(None);
        if let Some(button) = controller.ivars().item.button(controller.mtm()) {
            unsafe {
                button.setTarget(None);
                button.setAction(None);
            }
        }
        NSStatusBar::systemStatusBar().removeStatusItem(&controller.ivars().item);
    }
}
struct MenuIvars {
    item: Retained<NSStatusItem>,
    menu: Retained<NSMenu>,
    model: RefCell<Menu>,
    opening: Cell<bool>,
    disposed: Cell<bool>,
    events: Events,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind=MainThreadOnly]
    #[name="RSMStatusMenuController"]
    #[ivars=MenuIvars]
    struct Controller;
    unsafe impl NSObjectProtocol for Controller {}
    unsafe impl NSMenuDelegate for Controller {
        #[unsafe(method(menuWillOpen:))]
        fn menu_will_open(&self,_menu:&NSMenu) {
            let _owner = unsafe {Retained::retain(self as *const Self as *mut Self)}.unwrap();
            (self.ivars().events.clone())(MenuEvent::WillOpen);
            self.rebuild();
        }
    }
    impl Controller {
        #[unsafe(method(openStatusMenu:))]
        fn open_status_menu(&self,_sender:Option<&AnyObject>) {self.request_menu();}
        #[unsafe(method(presentStatusMenu:))]
        fn present_status_menu(&self,_sender:Option<&AnyObject>) {
            self.ivars().opening.set(false);
            if self.ivars().disposed.get() {return;}
            #[allow(deprecated)]
            self.ivars().item.popUpStatusItemMenu(&self.ivars().menu);
        }
        #[unsafe(method(activateMenuItem:))]
        fn activate_menu_item(&self,sender:&NSMenuItem) {
            let _owner = unsafe {Retained::retain(self as *const Self as *mut Self)}.unwrap();
            let Some(id) = sender.representedObject().and_then(|value|value.downcast::<NSString>().ok()) else {return;};
            let item = self.ivars().model.borrow().items.iter().find(|item| match item {
                Item::Action(action) => action.id == id.to_string(),
                Item::SwitchRow(row) => row.id == id.to_string(), _ => false,
            }).cloned();
            match item {
                Some(Item::Action(action)) if action.enabled => (self.ivars().events.clone())(MenuEvent::Action(action.id)),
                Some(Item::SwitchRow(row)) if row.enabled => {
                    if let Some(value) = row.copy_value {
                        copy_value(&value, Some(&self.ivars().menu));
                        (self.ivars().events.clone())(MenuEvent::Copied {id: row.id, value});
                    }
                }
                _ => {}
            }
        }
    }
);
impl Controller {
    fn new(mtm: MainThreadMarker, events: Events) -> Retained<Self> {
        unsafe {
            let item = NSStatusBar::systemStatusBar().statusItemWithLength(-1.);
            let menu = NSMenu::new(mtm);
            menu.setAutoenablesItems(false);
            let this = Self::alloc(mtm).set_ivars(MenuIvars {
                item,
                menu,
                model: RefCell::new(Menu::default()),
                opening: Cell::new(false),
                disposed: Cell::new(false),
                events,
            });
            let this: Retained<Self> = msg_send![super(this), init];
            this.ivars()
                .menu
                .setDelegate(Some(ProtocolObject::from_ref(&*this)));
            if let Some(button) = this.ivars().item.button(mtm) {
                button.setTarget(Some(&this));
                button.setAction(Some(sel!(openStatusMenu:)));
                button.sendActionOn(NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown);
            }
            this
        }
    }
    fn set_status(&self, status: &StatusItem) {
        if let Some(button) = self.ivars().item.button(self.mtm()) {
            if let Some(image) = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &text(&status.symbol),
                Some(&text(&status.label)),
            ) {
                image.setTemplate(true);
                button.setImage(Some(&image));
            }
            button.setAccessibilityLabel(Some(&text(&status.label)));
        }
    }
    fn request_menu(&self) {
        if self.ivars().disposed.get() || self.ivars().opening.replace(true) {
            return;
        }
        // Activation must finish before the menu's nested tracking loop owns the cursor.
        #[allow(deprecated)]
        NSApplication::sharedApplication(self.mtm()).activateIgnoringOtherApps(true);
        unsafe {
            self.performSelector_withObject_afterDelay(sel!(presentStatusMenu:), None, 0.);
        }
    }
    fn command(&self, action: &Action) -> Retained<NSMenuItem> {
        unsafe {
            let item = NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(self.mtm()),
                &text(&action.label),
                Some(sel!(activateMenuItem:)),
                &text(""),
            );
            item.setRepresentedObject(Some(&text(&action.id)));
            item.setTarget(Some(self));
            item.setEnabled(action.enabled);
            item.setImage(None);
            if item.respondsToSelector(sel!(setPreferredImageVisibility:)) {
                let _: () = msg_send![&*item,setPreferredImageVisibility:2isize];
            }
            item
        }
    }
    fn rebuild(&self) {
        if self.ivars().disposed.get() {
            return;
        }
        let model = self.ivars().model.borrow();
        let menu = &self.ivars().menu;
        let mtm = self.mtm();
        menu.removeAllItems();
        if !model.title.is_empty() {
            menu.addItem(&NSMenuItem::sectionHeaderWithTitle(
                &text(&model.title),
                mtm,
            ));
        }
        let width = model
            .items
            .iter()
            .filter_map(|item| match item {
                Item::SwitchRow(row) => Some(
                    label(&row.title, 13., false, mtm)
                        .intrinsicContentSize()
                        .width
                        .max(
                            label(&row.subtitle, 11., false, mtm)
                                .intrinsicContentSize()
                                .width,
                        )
                        + ROW_TEXT_RESERVE,
                ),
                Item::Section(section) => Some(
                    label(&section.title, 12., true, mtm)
                        .intrinsicContentSize()
                        .width
                        + label(&section.detail, 12., false, mtm)
                            .intrinsicContentSize()
                            .width
                        + 56.,
                ),
                _ => None,
            })
            .fold(350., f64::max)
            .ceil()
            .min(520.);
        for item in &model.items {
            match item {
                Item::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
                Item::Action(action) => menu.addItem(&self.command(action)),
                Item::Label(label) => {
                    let item = NSMenuItem::new(mtm);
                    item.setTitle(&text(label));
                    item.setEnabled(false);
                    menu.addItem(&item);
                }
                Item::Section(section) => {
                    let item = NSMenuItem::new(mtm);
                    item.setTitle(&text(&format!("{}, {}", section.title, section.detail)));
                    item.setEnabled(false);
                    item.setView(Some(&section_view(section, width, mtm)));
                    menu.addItem(&item);
                }
                Item::SwitchRow(row) => {
                    let item = NSMenuItem::new(mtm);
                    item.setTitle(&text(&row.title));
                    item.setToolTip(None);
                    item.setEnabled(row.enabled);
                    if row.copy_value.is_some() {
                        unsafe {
                            item.setRepresentedObject(Some(&text(&row.id)));
                            item.setTarget(Some(self));
                            item.setAction(Some(sel!(activateMenuItem:)));
                        }
                    }
                    item.setView(Some(&row_view(
                        row,
                        width,
                        menu,
                        self.ivars().events.clone(),
                        mtm,
                    )));
                    menu.addItem(&item);
                }
            }
        }
    }
}
fn section_view(section: &Section, width: f64, mtm: MainThreadMarker) -> Retained<NSView> {
    unsafe {
        let view = NSView::initWithFrame(NSView::alloc(mtm), rect(0., 0., width, 30.));
        view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        let project = label(&section.title, 12., true, mtm);
        let workspace = label(&section.detail, 12., false, mtm);
        // Round text measurements up and leave room for the text field's cell insets.
        let workspace_width =
            (workspace.intrinsicContentSize().width.ceil() + 4.).min(width - 128.);
        workspace.setAlignment(NSTextAlignment::Right);
        project.setFrame(rect(16., 7., width - workspace_width - 48., 16.));
        project.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        workspace.setFrame(rect(
            width - workspace_width - 16.,
            7.,
            workspace_width,
            16.,
        ));
        workspace.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
        workspace.setTextColor(Some(&NSColor::secondaryLabelColor()));
        view.addSubview(&project);
        view.addSubview(&workspace);
        view.setAccessibilityElement(true);
        view.setAccessibilityRole(Some(NSAccessibilityStaticTextRole));
        view.setAccessibilityLabel(Some(&text(&format!(
            "{}, {}",
            section.title, section.detail
        ))));
        view
    }
}
