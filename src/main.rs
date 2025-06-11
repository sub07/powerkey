use std::{
    ptr::null_mut,
    thread::{self, sleep},
    time::Duration,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowInfo, GetWindowTextLengthA, GetWindowTextW,
};
use windows_strings::w;

mod custom_widget;
mod subscription;
mod utils;
mod window;

fn main() {
    thread::spawn(|| unsafe {
        loop {
            let window = GetForegroundWindow();
            let len = GetWindowTextLengthA(window);
            let mut title = vec![0u16; len as usize];
            GetWindowTextW(window, title.as_mut_slice());
            let title = String::from_utf16_lossy(&title);
            println!("{title}");
            sleep(Duration::from_secs(1));
        }
    });

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
