//! Menu descriptions are ordinary Rust values and can be built outside the UI thread.

/// A status item's accessibility label and SF Symbol name.
#[derive(Clone, Debug, PartialEq)]
pub struct StatusItem {
    pub label: String,
    pub symbol: String,
}
impl StatusItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            symbol: "circle".into(),
        }
    }
    pub fn symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = symbol.into();
        self
    }
}
/// A flat native menu. Sections add visual hierarchy without creating submenus.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Menu {
    pub title: String,
    pub items: Vec<Item>,
}
impl Menu {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            items: Vec::new(),
        }
    }
    pub fn item(mut self, item: impl Into<Item>) -> Self {
        self.items.push(item.into());
        self
    }
    pub fn separator(self) -> Self {
        self.item(Item::Separator)
    }
}
/// A native command, section heading, switch row, separator or static label.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Action(Action),
    Section(Section),
    SwitchRow(SwitchRow),
    Separator,
    Label(String),
}
impl From<Action> for Item {
    fn from(value: Action) -> Self {
        Self::Action(value)
    }
}
impl From<Section> for Item {
    fn from(value: Section) -> Self {
        Self::Section(value)
    }
}
impl From<SwitchRow> for Item {
    fn from(value: SwitchRow) -> Self {
        Self::SwitchRow(value)
    }
}
/// A command with no icon, key equivalent or automatic text decoration.
#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}
impl Action {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            enabled: true,
        }
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}
/// A heading with optional right-aligned secondary text.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub title: String,
    pub detail: String,
}
impl Section {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: String::new(),
        }
    }
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }
}
/// A two-line native row with a Liquid Glass switch and an optional copy action.
/// IDs must be unique within a menu. Update `on` through `StatusMenu::set_menu` after handling a toggle.
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchRow {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub on: bool,
    /// Shows an indeterminate spinner beside the switch without changing the row title.
    pub busy: bool,
    pub enabled: bool,
    pub copy_value: Option<String>,
    pub copy_hint: Option<String>,
    pub toggle_label: Option<String>,
    pub help: Option<String>,
}
impl SwitchRow {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: String::new(),
            on: false,
            busy: false,
            enabled: true,
            copy_value: None,
            copy_hint: None,
            toggle_label: None,
            help: None,
        }
    }
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }
    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn copy_value(mut self, value: impl Into<String>) -> Self {
        self.copy_value = Some(value.into());
        self
    }
    pub fn copy_hint(mut self, hint: impl Into<String>) -> Self {
        self.copy_hint = Some(hint.into());
        self
    }
    pub fn toggle_label(mut self, label: impl Into<String>) -> Self {
        self.toggle_label = Some(label.into());
        self
    }
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}
/// Events run on the AppKit main thread. No application or async runtime is assumed.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuEvent {
    WillOpen,
    Action(String),
    Toggle { id: String, on: bool },
    Copied { id: String, value: String },
}
