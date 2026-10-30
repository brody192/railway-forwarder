//! Native row composition and controls, with independent hover and toggle handling.
mod copy_label;
mod glass_switch;
use crate::{Events, MenuEvent, SwitchRow, label, rect, text};
use copy_label::CopyLabel;
pub(crate) use copy_label::copy_value;
pub use glass_switch::GlassSwitch;
use objc2::{MainThreadOnly, rc::Retained};
use objc2_app_kit::{
    NSAccessibility, NSAutoresizingMaskOptions, NSColor, NSControlSize, NSMenu,
    NSProgressIndicator, NSProgressIndicatorStyle, NSView,
};
use objc2_foundation::MainThreadMarker;
// Reserve the spinner's space even while idle, so connecting never shifts the text.
pub(crate) const ROW_TEXT_RESERVE: f64 = 130.;
pub(crate) fn row_view(
    row: &SwitchRow,
    width: f64,
    menu: &NSMenu,
    events: Events,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let view = NSView::initWithFrame(NSView::alloc(mtm), rect(0., 0., width, 48.));
    // AppKit can widen a menu after measuring its standard items. Let the row
    // follow that width and keep its controls anchored to the trailing edge.
    view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    let content: Retained<NSView> = if row.copy_value.is_some() && row.enabled {
        let link = CopyLabel::new(row, menu, events.clone(), mtm);
        link.setFrame(rect(0., 0., width - 78., 48.));
        link.into_super().into_super()
    } else {
        NSView::initWithFrame(NSView::alloc(mtm), rect(0., 0., width - 78., 48.))
    };
    content.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    let title = label(&row.title, 13., false, mtm);
    title.setFrame(rect(16., 26., width - ROW_TEXT_RESERVE, 17.));
    title.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    let subtitle = label(&row.subtitle, 11., false, mtm);
    subtitle.setTextColor(Some(&NSColor::secondaryLabelColor()));
    subtitle.setFrame(rect(16., 8., width - ROW_TEXT_RESERVE, 14.));
    subtitle.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    content.addSubview(&title);
    content.addSubview(&subtitle);
    view.addSubview(&content);
    if row.busy {
        let spinner = NSProgressIndicator::initWithFrame(
            NSProgressIndicator::alloc(mtm),
            rect(0., 0., 16., 16.),
        );
        spinner.setStyle(NSProgressIndicatorStyle::Spinning);
        spinner.setControlSize(NSControlSize::Small);
        spinner.setIndeterminate(true);
        spinner.sizeToFit();
        let size = spinner.frame().size;
        spinner.setFrame(rect(
            width - 80. - size.width,
            (48. - size.height) / 2.,
            size.width,
            size.height,
        ));
        spinner.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
        spinner.setAccessibilityLabel(Some(&text("Working")));
        if let Some(help) = &row.help {
            spinner.setAccessibilityHelp(Some(&text(help)));
        }
        unsafe {
            spinner.setUsesThreadedAnimation(true);
            spinner.startAnimation(None);
        }
        view.addSubview(&spinner);
    }
    let id = row.id.clone();
    let switch = GlassSwitch::new(row.on, mtm, move |on| {
        events(MenuEvent::Toggle { id: id.clone(), on })
    });
    switch.setFrame(rect(width - 70., 12., 54., 24.));
    switch.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
    switch.setEnabled(row.enabled);
    switch.setAccessibilityLabel(Some(&text(
        row.toggle_label.as_deref().unwrap_or(&row.title),
    )));
    if let Some(help) = &row.help {
        switch.setAccessibilityHelp(Some(&text(help)));
    }
    view.addSubview(&switch);
    view
}
