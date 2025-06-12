use crate::{subscription, window::main::Message};

impl From<subscription::global_event_listener::Message> for Message {
    fn from(message: subscription::global_event_listener::Message) -> Self {
        match message {
            subscription::global_event_listener::Message::Ready(sender) => {
                Message::GlobalEventListenerCommandSender(sender)
            }
            subscription::global_event_listener::Message::Event(event) => {
                Message::GlobalEvent(event)
            }
            subscription::global_event_listener::Message::ModeJustSet(mode) => {
                Message::GlobalEventListenerModeChanged(mode)
            }
        }
    }
}

impl From<subscription::global_event_player::Message> for Message {
    fn from(message: subscription::global_event_player::Message) -> Self {
        match message {
            subscription::global_event_player::Message::PlaybackDone => {
                Message::GlobalEventPlayerPlaybackDone
            }
            subscription::global_event_player::Message::SenderReady(sender) => {
                Message::GlobalEventPlayerReady(sender)
            }
            subscription::global_event_player::Message::JustPlayed(event) => {
                Message::GlobalEventPlayerJustPlayed(event)
            }
        }
    }
}
