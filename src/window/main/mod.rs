use std::{fmt::Display, time::SystemTime};

use iced::{
    Element, Length, Subscription, Task, Theme,
    futures::channel::mpsc::Sender,
    keyboard::{Key, Modifiers, key::Named},
    widget::{button, column, container, horizontal_space, mouse_area, row, scrollable, text},
};
use itertools::Itertools;
use rdev::EventType;

use crate::{
    custom_widget::separator::separator,
    subscription::{
        self,
        global_event_listener::{Command, ListenerMode},
    },
    utils::{SenderOption, SubscriptionExt},
};

mod global_event_mapper;

#[derive(Default, Debug)]
enum PlaybackMode {
    #[default]
    Idle,
    Play,
    Record,
}

#[derive(Clone, PartialEq, Debug)]
pub struct PrintableEvent(subscription::global_event_listener::Event);

impl Display for PrintableEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0.kind {
            subscription::global_event_listener::EventKind::Input(event) => match event {
                EventType::KeyPress(key) => write!(f, "Press {key:?}"),
                EventType::KeyRelease(key) => write!(f, "Release {key:?}"),
                _ => unreachable!("mouse event not supported"),
            },
            subscription::global_event_listener::EventKind::FocusChange {
                window_title, ..
            } => {
                write!(f, "Focus changed to window \"{window_title}\"")
            }
            subscription::global_event_listener::EventKind::Delay(duration) => {
                write!(f, "{}ms delay", duration.as_millis())
            }
        }
    }
}

#[derive(Default)]
pub struct State {
    global_event_listener_command_sender:
        Option<Sender<subscription::global_event_listener::Command>>,
    global_event_player_command_sender: Option<Sender<subscription::global_event_player::Command>>,
    current_mode: subscription::global_event_listener::ListenerMode,
    playback_mode: PlaybackMode,
    items: Vec<PrintableEvent>,
    selected_item: Option<PrintableEvent>,
}

impl State {
    fn index_of_selected_item(&self) -> Option<usize> {
        self.items.iter().position(|event| {
            self.selected_item
                .clone()
                .is_some_and(|selected_event| *event == selected_event)
        })
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    GlobalEvent(subscription::global_event_listener::Event),
    GlobalEventListenerCommandSender(Sender<subscription::global_event_listener::Command>),
    GlobalEventListenerModeChanged(subscription::global_event_listener::ListenerMode),
    GlobalEventPlayerPlaybackDone,
    GlobalEventPlayerJustPlayed(subscription::global_event_listener::Event),
    GlobalEventPlayerReady(Sender<subscription::global_event_player::Command>),
    RecordButtonPressed,
    PlayButtonPressed,
    StopButtonPressed,
    Delete,
    Next,
    Previous,
    OnItemPicked(PrintableEvent),
}

pub fn title(_state: &State) -> String {
    "Powerkey".into()
}

pub fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::GlobalEvent(event) => {
            if let (ListenerMode::Listen, PlaybackMode::Record) =
                (&state.current_mode, &mut state.playback_mode)
            {
                if let Some(previous_event) = state.items.last() {
                    if let Ok(delay) = event.time.duration_since(previous_event.0.time) {
                        state.items.push(PrintableEvent(
                            subscription::global_event_listener::Event::new(
                                SystemTime::now(),
                                subscription::global_event_listener::EventKind::Delay(delay),
                            ),
                        ));
                    }
                }
                state.items.push(PrintableEvent(event));
            }
        }
        Message::GlobalEventListenerCommandSender(sender) => {
            state.global_event_listener_command_sender = Some(sender);
        }
        Message::GlobalEventListenerModeChanged(new_mode) => state.current_mode = new_mode,
        Message::RecordButtonPressed => {
            state.playback_mode = PlaybackMode::Record;
            state.items.clear();
            state
                .global_event_listener_command_sender
                .try_send(Command::SetMode(ListenerMode::Listen))
                .unwrap();
        }
        Message::PlayButtonPressed => {
            state.playback_mode = PlaybackMode::Play;
            state
                .global_event_listener_command_sender
                .try_send(Command::SetMode(ListenerMode::Disabled))
                .unwrap();
            state
                .global_event_player_command_sender
                .try_send(
                    subscription::global_event_player::Command::StartPlaybackWith(
                        state
                            .items
                            .clone()
                            .into_iter()
                            .map(|event| event.0)
                            .collect_vec(),
                    ),
                )
                .unwrap();
        }
        Message::StopButtonPressed => {
            state.playback_mode = PlaybackMode::Idle;
            state
                .global_event_listener_command_sender
                .try_send(Command::SetMode(ListenerMode::Disabled))
                .unwrap();
            state
                .global_event_player_command_sender
                .try_send(subscription::global_event_player::Command::StopPlayback)
                .unwrap();
        }
        Message::OnItemPicked(printable_event) => {
            state.selected_item = Some(printable_event);
        }
        Message::GlobalEventPlayerPlaybackDone => {
            state.playback_mode = PlaybackMode::Idle;
            state
                .global_event_listener_command_sender
                .try_send(Command::SetMode(ListenerMode::Disabled))
                .unwrap();
        }
        Message::GlobalEventPlayerReady(sender) => {
            state.global_event_player_command_sender = Some(sender);
        }
        Message::Delete => {
            if let Some(position) = state.index_of_selected_item() {
                state.items.remove(position);
                if state.items.is_empty() {
                    state.selected_item = None;
                } else {
                    state.selected_item =
                        Some(state.items[position.clamp(0, state.items.len() - 1)].clone())
                }
                return Task::done(Message::StopButtonPressed);
            }
        }
        Message::GlobalEventPlayerJustPlayed(event) => {
            state.selected_item = Some(PrintableEvent(event));
        }
        Message::Next => {
            if let Some(index) = state.index_of_selected_item() {
                let next_index = index + 1;
                let next_index = next_index.clamp(0, state.items.len() - 1);
                state.selected_item = Some(state.items[next_index].clone());
            }
        }
        Message::Previous => {
            if let Some(index) = state.index_of_selected_item() {
                let next_index = index as i32 - 1;
                let next_index = next_index.clamp(0, state.items.len() as i32 - 1);
                state.selected_item = Some(state.items[next_index as usize].clone());
            }
        }
    }

    Task::none()
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
    let global_event_listener =
        Subscription::run(subscription::global_event_listener::stream).map_into();
    let local_event_listener = iced::keyboard::on_key_press(on_key_press);
    let global_event_player =
        Subscription::run(subscription::global_event_player::stream).map_into();

    Subscription::batch([
        global_event_listener,
        local_event_listener,
        global_event_player,
    ])
}

fn on_key_press(key: Key, _modifiers: Modifiers) -> Option<Message> {
    match key {
        Key::Named(Named::Delete) => Some(Message::Delete),
        Key::Named(Named::ArrowUp) => Some(Message::Previous),
        Key::Named(Named::ArrowDown) => Some(Message::Next),
        _ => None,
    }
}
