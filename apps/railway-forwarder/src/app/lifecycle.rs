use super::{CloseSettings, Quit, backend, events};
use crate::{platform::macos, ui::settings};
use gpui_kit::*;
use settings::SettingsView;

fn update_theme(cx: &mut App) {
    let mut theme = gpui_macos::Theme::dark().with_accent(rgb(0x007aff).into());
    theme.background = rgb(0x202326).into();
    theme.surface = rgb(0x292a2e).into();
    theme.inset = rgb(0x2a2b2f).into();
    theme.apply(cx);
}

fn open_settings(cx: &mut App) -> Result<(AnyWindowHandle, Entity<SettingsView>)> {
    let bounds = Bounds::centered(None, size(px(920.), px(660.)), cx);
    let mut options = gpui_macos::native_window_options("Settings", cx);
    if let Some(titlebar) = &mut options.titlebar {
        // Match Finder's inset while keeping the native AppKit window buttons.
        titlebar.traffic_light_position = Some(point(px(19.), px(19.)));
    }
    let result = gpui_macos::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(740.), px(520.))),
            ..options
        },
        cx,
        |window, cx| cx.new(|cx| SettingsView::new(window, cx)),
    )?;
    cx.activate(true);
    Ok(result)
}

pub(crate) fn run() {
    if std::env::args().any(|arg| arg == "--check-start-at-login") {
        println!("{}", macos::login_item::diagnostic_status());
        return;
    }
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
    let application = gpui_macos::application()
        .with_assets(gpui_macos::NativeAssets::new(gpui_kit::assets::AllAssets))
        .with_quit_mode(QuitMode::Explicit);
    application.run(|cx| {
        gpui_macos::init(cx);
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
                    events::Event::RefreshLoginItem => events::refresh_login_item(),
                    events::Event::SetStartAtLogin(enabled) => events::set_start_at_login(enabled),
                    events::Event::OpenLoginItems => events::open_login_items(),
                });
            }
        })
        .detach();
    });
}
