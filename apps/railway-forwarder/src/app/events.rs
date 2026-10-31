use crate::app::backend::Backend;
use crate::domain::Snapshot;
use anyhow::{Context, Result};
use serde_json::Value;
use std::sync::{Arc, OnceLock};
use tokio::sync::oneshot;
use uuid::Uuid;

pub enum Event {
    OpenSettings,
    Changed,
    Authenticate {
        url: String,
        reply: oneshot::Sender<Result<String>>,
    },
    AuthFinished(Uuid),
    ShowMenu,
    RefreshLoginItem,
    SetStartAtLogin(bool),
    OpenLoginItems,
}
static EVENTS: OnceLock<async_channel::Sender<Event>> = OnceLock::new();
static BACKEND: OnceLock<Arc<Backend>> = OnceLock::new();
pub fn start() -> async_channel::Receiver<Event> {
    let (sender, receiver) = async_channel::unbounded();
    EVENTS.set(sender).expect("event loop initialized once");
    let demo = std::env::args().any(|arg| arg == "--demo") || crate::platform::macos::demo_mode();
    let backend = Backend::new(demo).expect("Could not initialize Railway Forwarder");
    BACKEND
        .set(backend.clone())
        .ok()
        .expect("backend initialized once");
    crate::platform::macos::initialize();
    refresh_login_item();
    render_menu();
    backend.warm_up();
    receiver
}
pub fn emit(event: Event) {
    if let Some(sender) = EVENTS.get() {
        let _ = sender.try_send(event);
    }
}
pub fn open_settings() {
    emit(Event::OpenSettings);
}
pub fn changed() {
    emit(Event::Changed);
}
pub fn refresh_login_item() {
    if let Some(backend) = BACKEND.get() {
        if let Some(current) = backend.login_item_status() {
            let status = crate::platform::macos::login_item::status();
            if status != current {
                backend.update_login_item(status, None);
            }
        }
    }
}
pub fn set_start_at_login(enabled: bool) {
    if let Some(backend) = BACKEND
        .get()
        .filter(|backend| backend.login_item_status().is_some())
    {
        let error = crate::platform::macos::login_item::set_enabled(enabled)
            .err()
            .map(|error| error.to_string());
        backend.update_login_item(crate::platform::macos::login_item::status(), error);
    }
}
pub fn open_login_items() {
    if BACKEND
        .get()
        .is_some_and(|backend| backend.login_item_status().is_some())
    {
        crate::platform::macos::login_item::open_settings();
    }
}
pub fn show_menu() {
    emit(Event::ShowMenu);
}
pub fn render_menu() {
    if let Some(backend) = BACKEND.get() {
        crate::platform::macos::render(&backend.snapshot());
    }
}
pub fn snapshot() -> std::result::Result<Snapshot, String> {
    BACKEND
        .get()
        .map(|backend| backend.snapshot())
        .ok_or_else(|| "The Railway backend is unavailable.".into())
}
pub fn dispatch(command: Value) {
    if let Some(backend) = BACKEND.get() {
        backend.dispatch(command);
    }
}
pub async fn authenticate(url: String) -> Result<String> {
    let (reply, receiver) = oneshot::channel();
    emit(Event::Authenticate { url, reply });
    receiver.await.context("Sign in was canceled.")?
}
pub fn cancel_authentication() {
    crate::platform::macos::cancel_auth();
}
pub fn shutdown() {
    cancel_authentication();
    if let Some(backend) = BACKEND.get() {
        backend.shutdown();
    }
}
