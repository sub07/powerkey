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
