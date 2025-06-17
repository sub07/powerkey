use crate::{subscription, window::main::Message};

impl From<subscription::global_event::listener::Message> for Message {
    fn from(message: subscription::global_event::listener::Message) -> Self {
        match message {
            subscription::global_event::listener::Message::Ready(sender) => {
                Message::GlobalEventListenerCommandSender(sender)
            }
            subscription::global_event::listener::Message::Event(event) => {
                Message::GlobalEvent(event)
            }
            subscription::global_event::listener::Message::ModeJustSet(mode) => {
                Message::GlobalEventListenerModeChanged(mode)
            }
        }
    }
}

impl From<subscription::global_event::player::Message> for Message {
    fn from(message: subscription::global_event::player::Message) -> Self {
        match message {
            subscription::global_event::player::Message::PlaybackDone => {
                Message::GlobalEventPlayerPlaybackDone
            }
            subscription::global_event::player::Message::SenderReady(sender) => {
                Message::GlobalEventPlayerReady(sender)
            }
            subscription::global_event::player::Message::JustPlayed { index } => {
                Message::GlobalEventPlayerJustPlayed(index)
            }
        }
    }
}
