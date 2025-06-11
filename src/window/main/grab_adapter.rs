use crate::{subscription, window::main::Message};

impl From<subscription::global_event::Message> for Message {
    fn from(event: subscription::global_event::Message) -> Self {
        match event {
            subscription::global_event::Message::Ready(sender) => {
                Message::GlobalEventListenerCommandSender(sender)
            }
            subscription::global_event::Message::Event(event) => Message::GrabbedEvent(event),
            subscription::global_event::Message::ModeJustSet(mode) => {
                Message::GlobalEventListenerModeChanged(mode)
            }
        }
    }
}
