use super::{CloseSettings, Quit, backend, events};
use crate::{platform::macos, ui::settings};
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
use settings::SettingsView;

fn update_theme(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.colors.primary = rgb(0x007aff).into();
        theme.colors.primary_hover = rgb(0x0066df).into();
        theme.colors.primary_foreground = rgb(0xffffff).into();
    });
}

fn open_settings(cx: &mut App) -> Result<(AnyWindowHandle, Entity<SettingsView>)> {
    let bounds = Bounds::centered(None, size(px(900.), px(700.)), cx);
    let result = gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(740.), px(520.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Railway Forwarder".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        cx,
        |window, cx| cx.new(|cx| SettingsView::new(window, cx)),
    )?;
    cx.activate(true);
    Ok(result)
}

pub(crate) fn run() {
    if let Some(index) = std::env::args().position(|arg| arg == "--check-search") {
        let result = std::env::args()
            .nth(index + 1)
            .ok_or_else(|| anyhow::anyhow!("Pass a UUID or search text after --check-search."))
            .and_then(|query| backend::Backend::check_search(&query));
        if let Err(error) = result {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--check-ssh") {
        if let Err(error) = backend::Backend::check_ssh() {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--check-catalog") {
        if let Err(error) = backend::Backend::check_catalog() {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        return;
    }
    let application = gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .with_quit_mode(QuitMode::Explicit);
    application.run(|cx| {
        gpui_kit::init(cx);
        update_theme(cx);
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-w", CloseSettings, Some("ForwarderSettings")),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.set_activation_policy(ActivationPolicy::Accessory);
        let events = events::start();
        cx.on_app_quit(|_| {
            events::shutdown();
            async {}
        })
        .detach();
        let mut settings = if std::env::args().any(|arg| arg == "--settings") {
            open_settings(cx).ok()
        } else {
            None
        };
        if macos::controls_preview_mode()
            || std::env::args().any(|arg| arg == "--native-controls-preview")
        {
            macos::preview_controls();
        }
        cx.spawn(async move |cx| {
            while let Ok(event) = events.recv().await {
                let _ = cx.update(|cx| match event {
                    events::Event::OpenSettings => {
                        if let Some((window, _)) = &settings {
                            if window
                                .update(cx, |_, window, _| window.activate_window())
                                .is_ok()
                            {
                                cx.activate(true);
                                return;
                            }
                        }
                        settings = open_settings(cx).ok();
                    }
                    events::Event::Changed => {
                        events::render_menu();
                        if let Some((window, view)) = &settings {
                            let view = view.clone();
                            let _ = window.update(cx, |_, window, cx| {
                                view.update(cx, |view, cx| view.refresh(window, cx))
                            });
                        }
                    }
                    events::Event::Authenticate { url, reply } => macos::authenticate(url, reply),
                    events::Event::AuthFinished(id) => macos::finish_auth(id),
                    events::Event::ShowMenu => macos::show_menu(),
                });
            }
        })
        .detach();
    });
}
