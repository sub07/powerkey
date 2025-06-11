use std::{fmt::Display, time::SystemTime};

use iced::{
    Element, Length, Subscription, Theme,
    futures::channel::mpsc::Sender,
    widget::{button, column, container, horizontal_space, mouse_area, row, scrollable, text},
};
use itertools::Itertools;
use rdev::{EventType, Key};

use crate::{
    custom_widget::separator::separator,
    subscription::{
        self,
        global_event::{ListenerCommand, ListenerMode},
    },
    utils::{SenderOption, SubscriptionExt},
};

mod grab_adapter;

#[derive(Default, Debug)]
enum PlaybackMode {
    #[default]
    Idle,
    Play,
    Record,
}

#[derive(Clone, PartialEq, Debug)]
pub struct PrintableEvent(rdev::Event);

impl Display for PrintableEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0.event_type {
            EventType::KeyPress(key) => write!(f, "Press {key:?}"),
            EventType::KeyRelease(key) => write!(f, "Release {key:?}"),
            _ => unreachable!("mouse event not supported"),
        }
    }
}

#[derive(Default)]
pub struct State {
    global_event_listener_command_sender:
        Option<Sender<subscription::global_event::ListenerCommand>>,
    current_mode: subscription::global_event::ListenerMode,
    playback_mode: PlaybackMode,
    items: Vec<PrintableEvent>,
    selected_item: Option<PrintableEvent>,
}

#[derive(Debug, Clone)]
pub enum Message {
    GrabbedEvent(rdev::Event),
    GlobalEventListenerCommandSender(Sender<subscription::global_event::ListenerCommand>),
    GlobalEventListenerModeChanged(subscription::global_event::ListenerMode),
    RecordButtonPressed,
    PlayButtonPressed,
    StopButtonPressed,
    OnItemPicked(PrintableEvent),
}

pub fn title(_state: &State) -> String {
    "Powerkey".into()
}

pub fn update(state: &mut State, message: Message) {
    match message {
        Message::GrabbedEvent(event) => {
            if let (ListenerMode::Listen, PlaybackMode::Record) =
                (&state.current_mode, &mut state.playback_mode)
            {
                state.items.push(PrintableEvent(event));
            }
        }
        Message::GlobalEventListenerCommandSender(sender) => {
            state.global_event_listener_command_sender = Some(sender);
            state.items = vec![
                PrintableEvent(rdev::Event {
                    event_type: EventType::KeyPress(Key::KeyA),
                    name: None,
                    time: SystemTime::now(),
                }),
                PrintableEvent(rdev::Event {
                    event_type: EventType::KeyPress(Key::KeyA),
                    name: None,
                    time: SystemTime::now(),
                }),
            ]
        }
        Message::GlobalEventListenerModeChanged(new_mode) => state.current_mode = new_mode,
        Message::RecordButtonPressed => {
            state.playback_mode = PlaybackMode::Record;
            state.items.clear();
            state
                .global_event_listener_command_sender
                .try_send(ListenerCommand::SetMode(ListenerMode::Listen))
                .unwrap();
        }
        Message::PlayButtonPressed => {
            state.playback_mode = PlaybackMode::Play;
            state
                .global_event_listener_command_sender
                .try_send(ListenerCommand::SetMode(ListenerMode::Grab))
                .unwrap();
        }
        Message::StopButtonPressed => {
            state.playback_mode = PlaybackMode::Idle;
            state
                .global_event_listener_command_sender
                .try_send(ListenerCommand::SetMode(ListenerMode::Disabled))
                .unwrap();
        }
        Message::OnItemPicked(printable_event) => {
            state.selected_item = Some(printable_event);
        }
    }
}

pub fn theme(_state: &State) -> iced::Theme {
    Theme::Oxocarbon
}

fn list_item<'a, 'b: 'a>(state: &'a State, value: &'b PrintableEvent) -> Element<'a, Message> {
    mouse_area(
        container(text!("{value}").style(|theme: &iced::Theme| {
            text::Style {
                color: if state
                    .selected_item
                    .clone()
                    .is_some_and(|selected| selected == value.clone())
                {
                    Some(theme.extended_palette().secondary.base.text)
                } else {
                    None
                },
            }
        }))
        .width(Length::Fill)
        .padding([8, 4])
        .style(|theme: &iced::Theme| {
            if state
                .selected_item
                .clone()
                .is_some_and(|selected| selected == value.clone())
            {
                container::background(theme.extended_palette().secondary.base.color)
            } else {
                Default::default()
            }
        }),
    )
    .on_press(Message::OnItemPicked(value.clone()))
    .into()
}

pub fn view(state: &State) -> Element<Message> {
    let items = column(
        state
            .items
            .iter()
            .map(|value| list_item(state, value))
            .intersperse_with(|| separator().into()),
    );

    column![
        row![
            text(format!("{:?}", state.current_mode)),
            horizontal_space().width(Length::Fixed(6.0)),
            text(format!("{:?}", state.playback_mode)),
        ]
        .height(Length::Shrink),
        row![
            button(text!("Record")).on_press(Message::RecordButtonPressed),
            button(text!("Play")).on_press(Message::PlayButtonPressed),
            button(text!("Stop")).on_press(Message::StopButtonPressed),
        ]
        .spacing(4.0),
        scrollable(items).spacing(4.0),
    ]
    .into()
}

pub fn subscription(_state: &State) -> Subscription<Message> {
    Subscription::run(subscription::global_event::stream).map_into()
}
