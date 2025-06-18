use crate::{
    subscription,
    window::main::{GlobalEventTrigger, Message, Trigger},
};

impl From<subscription::global_event::listener::Message> for Message {
    fn from(message: subscription::global_event::listener::Message) -> Self {
        match message {
            subscription::global_event::listener::Message::Ready(sender) => Message::Trigger(
                Trigger::GlobalEvent(GlobalEventTrigger::ListenerReady(sender)),
            ),
            subscription::global_event::listener::Message::Event(event) => {
                Message::Trigger(Trigger::GlobalEvent(GlobalEventTrigger::Event(event)))
            }
            subscription::global_event::listener::Message::ModeJustSet(mode) => Message::Trigger(
                Trigger::GlobalEvent(GlobalEventTrigger::ListenerModeJustChanged(mode)),
            ),
            subscription::global_event::listener::Message::SetNextEventsToBeIgnoredByGrabDone => {
                Message::Trigger(Trigger::GlobalEvent(
                    GlobalEventTrigger::ListenerAddGrabIgnoreListDone,
                ))
            }
        }
    }
}

impl From<subscription::global_event::player::Message> for Message {
    fn from(message: subscription::global_event::player::Message) -> Self {
        match message {
            subscription::global_event::player::Message::PlaybackDone => Message::Trigger(
                Trigger::GlobalEvent(GlobalEventTrigger::PlayerPlaybackJustEnded),
            ),
            subscription::global_event::player::Message::SenderReady(sender) => Message::Trigger(
                Trigger::GlobalEvent(GlobalEventTrigger::PlayerReady(sender)),
            ),
            subscription::global_event::player::Message::JustPlayed { index } => Message::Trigger(
                Trigger::GlobalEvent(GlobalEventTrigger::PlayerJustPlayed(index)),
            ),
            subscription::global_event::player::Message::PlaybackJustStarted => Message::Trigger(
                Trigger::GlobalEvent(GlobalEventTrigger::PlayerPlaybackJustStarted),
            ),
        }
    }
}
