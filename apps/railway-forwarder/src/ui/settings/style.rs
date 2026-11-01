//! Shared macOS settings composition, independent of settings behavior.
use super::Tab;
use gpui_kit::{assets::IconName, base::StyledExt, *};
use gpui_macos::ActiveTheme;

impl Tab {
    pub(super) fn icon(self) -> IconName {
        match self {
            Self::Services => IconName::Server,
            Self::Menu => IconName::List,
            Self::General => IconName::Settings,
            Self::Account => IconName::CircleUser,
        }
    }
}

pub(super) fn selection_fill(selected: bool, window: &Window, cx: &App) -> Hsla {
    if selected {
        cx.macos().foreground.opacity(if window.is_window_active() {
            0.16
        } else {
            0.08
        })
    } else {
        transparent_black()
    }
}

pub(super) fn page_heading(tab: Tab) -> Div {
    div().h_flex().items_center().min_h(px(34.)).child(
        div()
            .text_size(px(22.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(tab.title()),
    )
}

pub(super) fn section_label(label: &'static str, cx: &App) -> Div {
    div()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.macos().secondary)
        .child(label)
}

pub(super) fn group(cx: &App) -> Div {
    gpui_macos::settings_group("", cx)
        .bg(cx.macos().inset)
        .border_1()
        .border_color(cx.macos().border.opacity(0.5))
}

pub(super) fn row(label: impl Into<SharedString>) -> Div {
    div()
        .h_flex()
        .items_center()
        .justify_between()
        .gap_4()
        .min_h(px(50.))
        .px(px(14.))
        .py_3()
        .child(div().flex_1().min_w_0().child(label.into()))
}

pub(super) fn divider(cx: &App) -> Div {
    div().mx(px(14.)).h(px(1.)).bg(cx.macos().divider())
}
