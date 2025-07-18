#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod custom_widget;
mod subscription;
mod utils;
mod window;

fn main() {
    pretty_env_logger::formatted_timed_builder()
        .filter_level(log::LevelFilter::Trace)
        .filter_module("wgpu", log::LevelFilter::Off)
        .filter_module("naga", log::LevelFilter::Off)
        .filter_module("async_io", log::LevelFilter::Off)
        .filter_module("cosmic_text", log::LevelFilter::Off)
        .filter_module("iced", log::LevelFilter::Off)
        .filter_module("polling", log::LevelFilter::Off)
        .init();

    iced::application(
        window::main::State::title,
        window::main::State::update,
        window::main::State::view,
    )
    .theme(window::main::theme)
    .subscription(window::main::subscription)
    .run_with(window::main::State::new)
    .unwrap();
}
