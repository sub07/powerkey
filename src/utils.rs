#[easy_ext::ext(SubscriptionExt)]
impl<T> iced::Subscription<T> {
    pub fn map_into<O>(self) -> iced::Subscription<O>
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

#[easy_ext::ext(SenderOption)]
impl<T> Option<iced::futures::channel::mpsc::Sender<T>> {
    pub fn try_send(&mut self, t: T) -> Result<(), SendError<T>> {
        if let Some(sender) = self {
            sender.try_send(t).map_err(|e| SendError::InnerError(e))
        } else {
            Err(SendError::NoSender)
        }
    }
}

pub fn get_window_title_from_hwnd(
    window: windows::Win32::Foundation::HWND,
) -> Result<String, std::string::FromUtf16Error> {
    unsafe {
        let len = windows::Win32::UI::WindowsAndMessaging::GetWindowTextLengthA(window) + 1; // + 1 for null terminator
        let mut title = vec![0u16; len as usize];
        windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(window, title.as_mut_slice());
        windows_strings::PWSTR::from_raw(title.as_mut_ptr()).to_string()
    }
}

pub fn get_focused_window_title() -> Result<String, std::string::FromUtf16Error> {
    unsafe {
        let window = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        get_window_title_from_hwnd(window)
    }
}

pub fn set_focused_window_by_title<S: AsRef<str>>(title: S) {
    unsafe {
        if let Ok(window) = windows::Win32::UI::WindowsAndMessaging::FindWindowW(
            windows_strings::PCWSTR::null(),
            &windows_strings::HSTRING::from(title.as_ref()),
        ) {
            windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(window).unwrap();
        }
    }
}

#[easy_ext::ext(OrdPairExt)]
impl<T: PartialOrd> (T, T) {
    pub fn ordered(self) -> (T, T) {
        let (a, b) = self;
        if a < b { (a, b) } else { (b, a) }
    }
}
