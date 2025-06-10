use crate::{
    subscription,
    window::main::{GlobalEventListenerMode, Message},
};

impl From<subscription::global_event::Message> for Message {
    fn from(event: subscription::global_event::Message) -> Self {
        match event {
            subscription::global_event::Message::Ready(sender) => {
                Message::GlobalEventListenerCommandSender(sender)
            }
            subscription::global_event::Message::Event(event) => Message::GrabbedEvent(event),
            subscription::global_event::Message::DisabledModeSet => {
                Message::GlobalEventListenerModeChanged(GlobalEventListenerMode::Disabled)
            }
            subscription::global_event::Message::ListenModeSet => {
                Message::GlobalEventListenerModeChanged(GlobalEventListenerMode::Listen)
            }
            subscription::global_event::Message::GrabModeSet => {
                Message::GlobalEventListenerModeChanged(GlobalEventListenerMode::Grab)
            }
        }
    }
}
