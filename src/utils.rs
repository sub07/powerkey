use easy_ext::ext;
use iced::Subscription;

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
