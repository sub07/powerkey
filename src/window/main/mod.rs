use iced::{
    Element, Subscription,
    futures::channel::mpsc::Sender,
    widget::{button, column, text},
};
use rdev::EventType;

use crate::{
    subscription::{self, global_event::ListenerCommand},
    utils::{SenderOption, SubscriptionExt},
};

mod grab_adapter;

#[derive(Debug, Default, Clone)]
pub enum GlobalEventListenerMode {
    #[default]
    Disabled,
    Listen,
    Grab,
}

#[derive(Default, Debug)]
enum PlaybackMode {
    #[default]
    Idle,
    Play,
    Record,
}

#[derive(Default)]
pub struct State {
    global_event_listener_command_sender:
        Option<Sender<subscription::global_event::ListenerCommand>>,
    current_mode: GlobalEventListenerMode,
    playback_mode: PlaybackMode,
    items: Vec<EventType>,
}

#[derive(Debug, Clone)]
pub enum Message {
    GrabbedEvent(rdev::Event),
    GlobalEventListenerCommandSender(Sender<subscription::global_event::ListenerCommand>),
    GlobalEventListenerModeChanged(GlobalEventListenerMode),
    RecordButtonPressed,
    PlayButtonPressed,
    StopButtonPressed,
}

pub fn title(_state: &State) -> String {
    "Powerkey".into()
}

pub fn update(state: &mut State, message: Message) {
    match message {
        Message::GrabbedEvent(event) => {
            if let Some(command_sender) = state.global_event_listener_command_sender.as_mut() {
                if let (GlobalEventListenerMode::Listen, PlaybackMode::Record) =
                    (&state.current_mode, &mut state.playback_mode)
                {
                    state.items.push(event.event_type);
                }
            }
        }
        Message::GlobalEventListenerCommandSender(sender) => {
            state.global_event_listener_command_sender = Some(sender)
        }
        Message::GlobalEventListenerModeChanged(new_mode) => state.current_mode = new_mode,
        Message::RecordButtonPressed => {
            state.playback_mode = PlaybackMode::Record;
            state.items.clear();
            state
                .global_event_listener_command_sender
                .try_send(ListenerCommand::SetListenMode)
                .unwrap();
        }
        Message::PlayButtonPressed => {
            state.playback_mode = PlaybackMode::Play;
            state
                .global_event_listener_command_sender
                .try_send(ListenerCommand::SetGrabMode)
                .unwrap();
        }
        Message::StopButtonPressed => {
            state.playback_mode = PlaybackMode::Idle;
            state
                .global_event_listener_command_sender
                .try_send(ListenerCommand::SetDisabledMode)
                .unwrap();
        }
    }
}

pub fn view(state: &State) -> Element<Message> {
    let items = column(
        state
            .items
            .iter()
            .map(|item| text(format!("{:?}", item)).into()),
    );

    column![
        text(format!("{:?}", state.current_mode)),
        text(format!("{:?}", state.playback_mode)),
        button(text!("Record")).on_press(Message::RecordButtonPressed),
        button(text!("Play")).on_press(Message::PlayButtonPressed),
        button(text!("Stop")).on_press(Message::StopButtonPressed),
        items,
    ]
    .into()
}

pub fn subscription(_state: &State) -> Subscription<Message> {
    Subscription::run(subscription::global_event::stream).map_into()
}
