//! Application startup, event delivery and state orchestration.
pub(crate) mod backend;
pub(crate) mod events;
mod lifecycle;
pub(crate) use lifecycle::run;
gpui_kit::actions!(forwarder, [Quit, CloseSettings]);
