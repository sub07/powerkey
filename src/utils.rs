use std::{ptr::null_mut, string::FromUtf16Error};

use easy_ext::ext;
use iced::Subscription;
use itertools::Itertools;
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        FindWindowW, GetForegroundWindow, GetWindowTextLengthA, GetWindowTextW, SetForegroundWindow,
    },
};
use windows_strings::{HSTRING, PCWSTR};

#[ext(SubscriptionExt)]
impl<T> Subscription<T> {
    pub fn map_into<O>(self) -> Subscription<O>
    where
        O: From<T> + 'static,
        T: 'static,
    {
        self.map(Into::into)
    }
}

#[derive(Debug)]
pub enum SendError<T> {
    NoSender,
    InnerError(iced::futures::channel::mpsc::TrySendError<T>),
}

#[ext(SenderOption)]
impl<T> Option<iced::futures::channel::mpsc::Sender<T>> {
    pub fn try_send(&mut self, t: T) -> Result<(), SendError<T>> {
        if let Some(sender) = self {
            sender.try_send(t).map_err(|e| SendError::InnerError(e))
        } else {
            Err(SendError::NoSender)
        }
    }
}

pub fn get_focused_window_title() -> Result<String, FromUtf16Error> {
    unsafe {
        let window = GetForegroundWindow();
        let len = GetWindowTextLengthA(window) + 1; // + 1 for null terminator
        let mut title = vec![0u16; len as usize];
        GetWindowTextW(window, title.as_mut_slice());
        windows_strings::PWSTR::from_raw(title.as_mut_ptr()).to_string()
    }
}

pub fn set_focused_window_by_title(title: &str) {
    unsafe {
        if let Ok(window) = FindWindowW(PCWSTR::null(), &HSTRING::from(title)) {
            SetForegroundWindow(window).unwrap();
        }
    }
}
