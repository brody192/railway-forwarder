//! Application values and pure catalog, naming and menu transformations.
mod account;
pub mod catalog;
mod menu;
mod names;
mod ports;
mod preferences;
mod resources;
mod snapshot;
mod startup;
pub use account::AccountPhoto;
pub use menu::{MenuLayout, MenuPort, MenuSection, menu_layout};
pub use names::{DisplayNameLabel, clean_name, display_names, project_catalog};
pub use ports::{AllowedPort, Port, TunnelState, preferred_ports};
pub use preferences::{MenuPreferences, Preferences};
pub use resources::{Environment, Project, Service, Workspace};
pub use snapshot::Snapshot;
pub use startup::LoginItemStatus;
