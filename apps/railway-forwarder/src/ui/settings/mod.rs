//! Settings view state and coordination between focused tab modules.
mod account;
mod disclosures;
mod filters;
mod general;
mod inputs;
mod menu;
mod names;
mod results;
mod services;
mod style;
mod view;
use crate::app::events;
use crate::domain::{AccountPhoto, Snapshot};
use disclosures::Disclosures;
use gpui_kit::{
    component::input::{InputEvent, InputState},
    *,
};
use names::NameField;
use results::SearchResults;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Services,
    Menu,
    General,
    Account,
}

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Self::Services => "Services",
            Self::Menu => "Menu",
            Self::General => "General",
            Self::Account => "Account",
        }
    }
}

const SIDEBAR_WIDTH: f32 = 220.;

pub struct SettingsView {
    pub(crate) snapshot: Snapshot,
    tab: Tab,
    disclosures: Disclosures,
    search_results: SearchResults,
    inputs: HashMap<String, Entity<InputState>>,
    subscriptions: Vec<Subscription>,
    client_id: Entity<InputState>,
    search: Entity<InputState>,
    query: String,
    selected_only: bool,
    _search_subscription: Subscription,
    _activation_subscription: Subscription,
    _window_chrome: Option<crate::platform::macos::settings::WindowChrome>,
    _login_item_watch: Option<Task<()>>,
    profile_image: Option<(Arc<AccountPhoto>, Arc<Image>)>,
    pub(crate) menu_selection: Vec<String>,
    name_fields: HashMap<String, NameField>,
}

impl SettingsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut snapshot = Self::read_snapshot();
        let window_chrome = match crate::platform::macos::settings::configure_window(
            window,
            SIDEBAR_WIDTH as f64,
        ) {
            Ok(chrome) => Some(chrome),
            Err(error) => {
                snapshot.error = Some(format!(
                    "Could not configure the macOS settings window. {error:#}"
                ));
                None
            }
        };
        let client_id = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Automatic registration")
                .default_value(snapshot.preferences.client_id.clone())
        });
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search names or UUIDs"));
        let search_subscription =
            cx.subscribe_in(&search, window, |this, input, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.search_changed(input.read(cx).value().to_string(), window, cx);
                }
            });
        let activation_subscription = cx.observe_window_activation(window, |_, window, cx| {
            if window.is_window_active() {
                events::emit(events::Event::RefreshLoginItem);
            }
            cx.notify();
        });
        let login_item_watch = (!snapshot.demo).then(|| Self::watch_login_item(window, cx));
        let profile_image = Self::make_profile_image(&snapshot);
        Self {
            tab: Tab::Services,
            snapshot,
            disclosures: Disclosures::new(HashSet::new()),
            search_results: SearchResults::default(),
            inputs: HashMap::new(),
            subscriptions: Vec::new(),
            client_id,
            search,
            query: String::new(),
            selected_only: false,
            _search_subscription: search_subscription,
            _activation_subscription: activation_subscription,
            _window_chrome: window_chrome,
            _login_item_watch: login_item_watch,
            profile_image,
            menu_selection: Vec::new(),
            name_fields: HashMap::new(),
        }
    }
    fn read_snapshot() -> Snapshot {
        events::snapshot().unwrap_or_else(|error| Snapshot {
            error: Some(error),
            ..Default::default()
        })
    }
    pub fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reload();
        self.sync_name_fields(window, cx);
        cx.notify();
    }
    fn reload(&mut self) {
        let snapshot = Self::read_snapshot();
        if self.snapshot.signed_in != snapshot.signed_in {
            self.inputs.clear();
            self.subscriptions.clear();
            self.disclosures = Disclosures::default();
            self.search_results = SearchResults::default();
            self.name_fields.clear();
        }
        self.snapshot = snapshot;
        self.sync_profile_image();
        let latest = self.filtered_catalog();
        self.search_results
            .receive_requested_projects(&latest, &self.snapshot.loaded_projects);
        if self.selected_only {
            self.load_selected_projects();
        }
    }
    pub(crate) fn send(&mut self, command: Value, cx: &mut Context<Self>) {
        if matches!(
            command["action"].as_str(),
            Some("refresh" | "allowPort" | "displayName" | "menuReset")
        ) {
            self.search_results.clear();
        }
        events::dispatch(command);
        self.reload();
        cx.notify();
    }

    fn filtered_catalog(&self) -> Vec<crate::domain::Workspace> {
        let catalog = if self.selected_only {
            crate::domain::catalog::selected(
                &self.snapshot.workspaces,
                &self.snapshot.preferences.allowed,
            )
        } else {
            self.snapshot.workspaces.clone()
        };
        crate::domain::catalog::filtered(&catalog, &self.query)
    }
}
