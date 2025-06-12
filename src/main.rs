use std::{
    thread::{self, sleep},
    time::Duration,
};

use crate::utils::get_focused_window_title;

mod custom_widget;
mod subscription;
mod utils;
mod window;

fn main() {
    iced::application(
        window::main::title,
        window::main::update,
        window::main::view,
    )
    .theme(window::main::theme)
    .subscription(window::main::subscription)
    .run()
    .unwrap();
}
