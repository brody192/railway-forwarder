//! Native settings-window presentation around the GPUI content.
use super::main_thread;
use anyhow::{Context, Result, bail};
use gpui_kit::Window;
use objc2::{msg_send, rc::Retained, sel};
use objc2_app_kit::{
    NSAccessibility, NSAutoresizingMaskOptions, NSColor, NSOutlineView, NSScrollView,
    NSSplitViewController, NSSplitViewDividerStyle, NSSplitViewItem, NSTableViewStyle, NSView,
    NSViewController, NSWindowOrderingMode, NSWindowTitleVisibility,
};
use objc2_foundation::NSObjectProtocol;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub struct WindowChrome {
    sidebar: Retained<NSSplitViewController>,
}

impl Drop for WindowChrome {
    fn drop(&mut self) {
        self.sidebar.view().removeFromSuperview();
    }
}

pub fn configure_window(window: &Window, sidebar_width: f64) -> Result<WindowChrome> {
    let main_thread = main_thread();
    let handle = HasWindowHandle::window_handle(window)?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        bail!("Settings requires an AppKit window.");
    };
    // The borrowed GPUI window owns this view for the duration of this call.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    let native = view
        .window()
        .context("The settings view is not attached to a window.")?;
    native.setToolbar(None);
    native.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    native.setTitlebarAppearsTransparent(true);
    native.setBackgroundColor(Some(&NSColor::windowBackgroundColor()));

    // Finder hosts its source list in a native sidebar split item and enables
    // base vibrancy. A standalone material/source list omits that backing.
    let parent =
        unsafe { view.superview() }.context("The settings renderer has no parent view.")?;
    let sidebar = NSScrollView::new(main_thread);
    sidebar.setAccessibilityHidden(true);
    let mut frame = view.frame();
    frame.size.width = sidebar_width;
    sidebar.setFrame(frame);
    sidebar.setHasVerticalScroller(false);
    sidebar.setHasHorizontalScroller(false);
    sidebar.setAutohidesScrollers(true);
    sidebar.setDrawsBackground(false);
    let source_list = NSOutlineView::new(main_thread);
    source_list.setStyle(NSTableViewStyle::SourceList);
    source_list.setHeaderView(None);
    source_list.setBackgroundColor(&NSColor::clearColor());
    source_list.setFrame(sidebar.bounds());
    source_list.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    sidebar.setDocumentView(Some(&source_list));
    let sidebar_controller = NSViewController::new(main_thread);
    sidebar_controller.setView(&sidebar);
    let sidebar_item = NSSplitViewItem::sidebarWithViewController(&sidebar_controller);
    sidebar_item.setMinimumThickness(sidebar_width);
    sidebar_item.setMaximumThickness(sidebar_width);
    sidebar_item.setCanCollapse(false);
    let base_vibrancy = sel!(_setHasBaseVibrancyEffect:);
    if !sidebar_item.respondsToSelector(base_vibrancy) {
        bail!("This macOS version does not provide Finder's sidebar vibrancy.");
    }
    // Finder's addSidebarColumn:containerLayoutManager: uses this internal
    // BOOL setter on NSSplitViewItem. Guard it so OS changes fail explicitly.
    unsafe {
        let _: () = msg_send![&sidebar_item, _setHasBaseVibrancyEffect: true];
    }
    let content_controller = NSViewController::new(main_thread);
    content_controller.setView(&NSView::new(main_thread));
    let content_item = NSSplitViewItem::splitViewItemWithViewController(&content_controller);
    let split_controller = NSSplitViewController::new(main_thread);
    let split = split_controller.splitView();
    split.setVertical(true);
    split.setDividerStyle(NSSplitViewDividerStyle::Thin);
    split.setWantsLayer(true);
    let underlay = split_controller.view();
    underlay.setFrame(view.frame());
    underlay.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    underlay.setAccessibilityHidden(true);
    split_controller.addSplitViewItem(&sidebar_item);
    split_controller.addSplitViewItem(&content_item);
    parent.addSubview_positioned_relativeTo(&underlay, NSWindowOrderingMode::Below, Some(view));
    underlay.layoutSubtreeIfNeeded();
    Ok(WindowChrome {
        sidebar: split_controller,
    })
}
