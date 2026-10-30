extern crate gpui_kit as gpui;
mod app;
mod domain;
mod integrations;
mod persistence;
mod platform;
mod ui;

fn main() {
    app::run();
}
