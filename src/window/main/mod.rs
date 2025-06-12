use std::{fmt::Display, time::SystemTime};

use iced::{
    Element, Length, Subscription, Task, Theme,
    futures::channel::mpsc::Sender,
    keyboard::{Key, Modifiers, key::Named},
    widget::{
        self, button, checkbox, column, container, horizontal_space, mouse_area, row,
        scrollable::{self, AbsoluteOffset, Viewport},
        text,
    },
    window::Level,
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
    window,
};

mod global_event_mapper;

#[derive(Default, Debug)]
enum PlaybackMode {
    #[default]
    Idle,
    Play,
    Record,
}

#[derive(Clone, Debug)]
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

pub struct State {
    global_event_listener_command_sender:
        Option<Sender<subscription::global_event_listener::Command>>,
    global_event_player_command_sender: Option<Sender<subscription::global_event_player::Command>>,
    current_mode: subscription::global_event_listener::ListenerMode,
    playback_mode: PlaybackMode,
    items: Vec<PrintableEvent>,
    selected_item_index: Option<usize>,
    item_list_scroll_viewport: Option<Viewport>,
    item_list_scroll_id: iced::widget::scrollable::Id,
    window_id: Option<iced::window::Id>,
    always_on_top: bool,
}

pub fn new() -> (State, Task<Message>) {
    let state = State {
        global_event_listener_command_sender: Default::default(),
        global_event_player_command_sender: Default::default(),
        playback_mode: Default::default(),
        current_mode: Default::default(),
        items: Default::default(),
        selected_item_index: Default::default(),
        item_list_scroll_viewport: Default::default(),
        item_list_scroll_id: iced::widget::scrollable::Id::unique(),
        window_id: None,
        always_on_top: false,
    };
    (state, Task::none())
}

impl State {
    fn scroll_to_item_task(&self) -> Task<Message> {
        if let Some((viewport, selected_item_index)) =
            self.item_list_scroll_viewport.zip(self.selected_item_index)
        {
            let item_height = viewport.content_bounds().height / self.items.len() as f32;
            let top = viewport.absolute_offset().y;
            let bottom = viewport.absolute_offset().y + viewport.bounds().height;
            let item_top = item_height * selected_item_index as f32;
            let item_bottom = item_top + item_height;

            let y_scroll = if item_top < top {
                item_top - top
            } else if item_bottom > bottom {
                item_bottom - bottom
            } else {
                0.0
            };

            if y_scroll != 0.0 {
                return iced::widget::scrollable::scroll_by(
                    self.item_list_scroll_id.clone(),
                    AbsoluteOffset {
                        x: 0.0,
                        y: y_scroll,
                    },
                );
            }
        }

        Task::none()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    GlobalEvent(subscription::global_event_listener::Event),
    GlobalEventListenerCommandSender(Sender<subscription::global_event_listener::Command>),
    GlobalEventListenerModeChanged(subscription::global_event_listener::ListenerMode),
    GlobalEventPlayerPlaybackDone,
    GlobalEventPlayerJustPlayed(usize),
    GlobalEventPlayerReady(Sender<subscription::global_event_player::Command>),
    RecordButtonPressed,
    PlayButtonPressed,
    StopButtonPressed,
    Delete,
    Next,
    Previous,
    OnItemClicked(usize),
    OnItemListScroll(Viewport),
    ToggleAlwaysOnTop(bool),
    WindowId(iced::window::Id),
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
            if !matches!(state.current_mode, ListenerMode::Disabled) {
                state
                    .global_event_listener_command_sender
                    .try_send(subscription::global_event_listener::Command::SetMode(
                        ListenerMode::Disabled,
                    ))
                    .unwrap();
            }

            if !matches!(state.playback_mode, PlaybackMode::Idle) {
                state
                    .global_event_player_command_sender
                    .try_send(subscription::global_event_player::Command::StopPlayback)
                    .unwrap();
            }

            state.playback_mode = PlaybackMode::Idle;
        }
        Message::OnItemClicked(index) => {
            state.selected_item_index = Some(index);
        }
        Message::GlobalEventPlayerPlaybackDone => {
            return Task::done(Message::StopButtonPressed);
        }
        Message::GlobalEventPlayerReady(sender) => {
            state.global_event_player_command_sender = Some(sender);
        }
        Message::Delete => {
            if let Some(index) = state.selected_item_index {
                state.items.remove(index);
                if state.items.is_empty() {
                    state.selected_item_index = None;
                } else {
                    state.selected_item_index = Some(index.clamp(0, state.items.len() - 1))
                }
                return Task::done(Message::StopButtonPressed);
            }
        }
        Message::GlobalEventPlayerJustPlayed(index) => {
            state.selected_item_index = Some(index);
        }
        Message::Next => {
            if let Some(index) = state.selected_item_index {
                let next_index = index + 1;
                let next_index = next_index.clamp(0, state.items.len() - 1);
                state.selected_item_index = Some(next_index);
                return state.scroll_to_item_task();
            }
        }
        Message::Previous => {
            if let Some(index) = state.selected_item_index {
                let next_index = index as i32 - 1;
                let next_index = next_index.clamp(0, state.items.len() as i32 - 1);
                state.selected_item_index = Some(next_index as usize);
                return state.scroll_to_item_task();
            }
        }
        Message::OnItemListScroll(viewport) => {
            state.item_list_scroll_viewport = Some(viewport);
        }
        Message::ToggleAlwaysOnTop(always_on_top) => {
            if let Some(window_id) = state.window_id {
                state.always_on_top = always_on_top;
                return iced::window::change_level(
                    window_id,
                    if always_on_top {
                        Level::AlwaysOnTop
                    } else {
                        Level::Normal
                    },
                );
            } else {
                return iced::window::get_oldest()
                    .and_then(|id| Task::done(Message::WindowId(id)))
                    .chain(Task::done(Message::ToggleAlwaysOnTop(always_on_top)));
            }
        }
        Message::WindowId(id) => state.window_id = Some(id),
    }

    Task::none()
}

pub fn theme(_state: &State) -> iced::Theme {
    Theme::TokyoNightLight
}

fn list_item<'a, 'b: 'a>(
    index: usize,
    event: &'b PrintableEvent,
    state: &'a State,
) -> Element<'a, Message> {
    mouse_area(
        container(text!("{event}").style(move |theme: &iced::Theme| {
            text::Style {
                color: if state
                    .selected_item_index
                    .is_some_and(|selected| selected == index)
                {
                    Some(theme.extended_palette().secondary.base.text)
                } else {
                    None
                },
            }
        }))
        .width(Length::Fill)
        .padding([4, 4])
        .style(move |theme: &iced::Theme| {
            if state
                .selected_item_index
                .is_some_and(|selected| selected == index)
            {
                container::background(theme.extended_palette().secondary.base.color)
            } else {
                Default::default()
            }
        }),
    )
    .on_press(Message::OnItemClicked(index))
    .into()
}

pub fn view(state: &State) -> Element<Message> {
    let items = column(
        state
            .items
            .iter()
            .enumerate()
            .map(|(index, event)| list_item(index, event, state))
            .intersperse_with(|| separator().into()),
    );

    column![
        row![
            column![
                text(format!("{:?}", state.current_mode)),
                text(format!("{:?}", state.playback_mode)),
            ],
            checkbox("Always on top", state.always_on_top).on_toggle(Message::ToggleAlwaysOnTop)
        ]
        .spacing(8.0)
        .height(Length::Shrink),
        row![
            button(text!("Record")).on_press(Message::RecordButtonPressed),
            button(text!("Play")).on_press(Message::PlayButtonPressed),
            button(text!("Stop")).on_press(Message::StopButtonPressed),
        ]
        .spacing(4.0),
        if state.items.is_empty() {
            Element::new(container(text("Press record !").size(24.0)).center(Length::Fill))
        } else {
            Element::new(
                widget::scrollable(items)
                    .spacing(8.0)
                    .id(state.item_list_scroll_id.clone())
                    .on_scroll(Message::OnItemListScroll),
            )
        },
    ]
    .spacing(4.0)
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
