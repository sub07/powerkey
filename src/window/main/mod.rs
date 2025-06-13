use std::{collections::BTreeSet, fmt::Display, time::SystemTime};

use iced::{
    Element, Length, Subscription, Task, Theme,
    event::Status,
    futures::channel::mpsc::Sender,
    keyboard::{Key, Modifiers, key::Named},
    widget::{
        self, button, checkbox, column, container, mouse_area, row,
        scrollable::{AbsoluteOffset, Viewport},
        text,
    },
    window::Level,
};
use itertools::Itertools;
use rdev::EventType;
use serde::{Deserialize, Serialize};

use crate::{
    custom_widget::separator::separator,
    subscription::{
        self,
        global_event_listener::{Command, ListenerMode},
    },
    utils::{OrdPairExt, SenderOption, SubscriptionExt},
};

mod global_event_mapper;

#[derive(Default, Debug)]
enum PlaybackMode {
    #[default]
    Idle,
    Play,
    Record,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Default, Debug)]
struct ItemSelectionState {
    selected_indices: BTreeSet<usize>,
}

impl ItemSelectionState {
    fn select(&mut self, index: usize) {
        self.selected_indices.clear();
        self.selected_indices.insert(index);
    }

    fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index)
    }

    fn get_first_selected(&self) -> Option<usize> {
        self.selected_indices.first().cloned()
    }

    fn get_last_selected(&self) -> Option<usize> {
        self.selected_indices.last().cloned()
    }

    fn iter(&self) -> impl Iterator<Item = usize> {
        self.selected_indices.iter().cloned()
    }

    fn unselect(&mut self) {
        self.selected_indices.clear();
    }

    fn expand_to(&mut self, index: usize) {
        let first_selected = self.selected_indices.first().cloned().unwrap_or(0);
        self.selected_indices.clear();
        let (start, end) = (first_selected, index).ordered();
        for item_index_to_be_added in start..=end {
            self.add_item_to_selection(item_index_to_be_added);
        }
    }

    fn add_item_to_selection(&mut self, index: usize) {
        self.selected_indices.insert(index);
    }
}

pub struct State {
    global_event_listener_command_sender:
        Option<Sender<subscription::global_event_listener::Command>>,
    global_event_player_command_sender: Option<Sender<subscription::global_event_player::Command>>,
    current_mode: subscription::global_event_listener::ListenerMode,
    playback_mode: PlaybackMode,
    items: Vec<PrintableEvent>,
    selected_items_state: ItemSelectionState,
    item_list_scroll_viewport: Option<Viewport>,
    item_list_scroll_id: iced::widget::scrollable::Id,
    window_id: Option<iced::window::Id>,
    always_on_top: bool,
    modifiers: Modifiers,
}

pub fn new() -> (State, Task<Message>) {
    let items = serde_json::from_str::<Vec<PrintableEvent>>(
        &std::fs::read_to_string("macro.json").unwrap(),
    )
    .unwrap();
    let state = State {
        global_event_listener_command_sender: Default::default(),
        global_event_player_command_sender: Default::default(),
        playback_mode: Default::default(),
        current_mode: Default::default(),
        items,
        selected_items_state: Default::default(),
        item_list_scroll_viewport: Default::default(),
        item_list_scroll_id: iced::widget::scrollable::Id::unique(),
        window_id: None,
        always_on_top: false,
        modifiers: Modifiers::default(),
    };
    (state, Task::none())
}

impl State {
    fn scroll_to_item_task(&self) -> Task<Message> {
        if let Some(viewport) = self.item_list_scroll_viewport {
            debug_assert_eq!(1, self.selected_items_state.selected_indices.len());
            let Some(selected_item_index) =
                self.selected_items_state.selected_indices.first().cloned()
            else {
                return Task::none();
            };

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
    ToggleAlwaysOnTop(bool),
    WindowId(iced::window::Id),
    ModifierChanged(Modifiers),

    // Item list events
    ItemClick(usize),
    SelectNext,
    SelectPrevious,
    ItemScroll(Viewport),
    ItemDelete,
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

            // std::fs::write("macro.json", serde_json::to_string(&state.items).unwrap()).unwrap();
        }
        Message::ItemClick(index) => {
            if state.modifiers.control() {
                state.selected_items_state.add_item_to_selection(index);
            } else if state.modifiers.shift() {
                state.selected_items_state.expand_to(index);
            } else {
                state.selected_items_state.select(index);
            }
        }
        Message::GlobalEventPlayerPlaybackDone => {
            return Task::done(Message::StopButtonPressed);
        }
        Message::GlobalEventPlayerReady(sender) => {
            state.global_event_player_command_sender = Some(sender);
        }
        Message::ItemDelete => {
            if let Some(first_item_selected) = state.selected_items_state.get_first_selected() {
                for (index_index, index_to_delete) in state.selected_items_state.iter().enumerate()
                {
                    state.items.remove(index_to_delete - index_index);
                }
                if state.items.is_empty() {
                    state.selected_items_state.unselect()
                } else {
                    state
                        .selected_items_state
                        .select(first_item_selected.clamp(0, state.items.len() - 1));
                }
                return Task::done(Message::StopButtonPressed);
            }
        }
        Message::GlobalEventPlayerJustPlayed(index) => {
            state.selected_items_state.select(index);
        }
        Message::SelectNext => {
            if let Some(last_item_selected) = dbg!(state.selected_items_state.get_last_selected()) {
                let next_index = last_item_selected + 1;
                let next_index = next_index.clamp(0, state.items.len() - 1);
                state.selected_items_state.select(next_index);
                return state.scroll_to_item_task();
            }
        }
        Message::SelectPrevious => {
            if let Some(last_item_selected) = state.selected_items_state.get_first_selected() {
                let next_index = last_item_selected.saturating_sub(1);
                state.selected_items_state.select(next_index);
                return state.scroll_to_item_task();
            }
        }
        Message::ItemScroll(viewport) => {
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
        Message::ModifierChanged(modifiers) => {
            state.modifiers = modifiers;
        }
    }

    Task::none()
}

pub fn theme(_state: &State) -> iced::Theme {
    Theme::Ferra
}

fn list_item<'a, 'b: 'a>(
    index: usize,
    event: &'b PrintableEvent,
    state: &'a State,
) -> Element<'a, Message> {
    let selected = state.selected_items_state.is_selected(index);
    mouse_area(
        container(
            text!("{event}").style(move |theme: &iced::Theme| text::Style {
                color: if selected {
                    Some(theme.extended_palette().secondary.base.text)
                } else {
                    None
                },
            }),
        )
        .width(Length::Fill)
        .padding([4, 4])
        .style(move |theme: &iced::Theme| {
            if selected {
                container::background(theme.extended_palette().secondary.base.color)
            } else {
                Default::default()
            }
        }),
    )
    .on_press(Message::ItemClick(index))
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
                    .on_scroll(Message::ItemScroll),
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
    let local_release_event_listener = iced::event::listen_with(on_event);
    let global_event_player =
        Subscription::run(subscription::global_event_player::stream).map_into();

    Subscription::batch([
        global_event_listener,
        local_event_listener,
        local_release_event_listener,
        global_event_player,
    ])
}

fn on_key_press(key: Key, modifiers: Modifiers) -> Option<Message> {
    match key {
        Key::Named(Named::Delete) => Some(Message::ItemDelete),
        Key::Named(Named::ArrowUp) => Some(Message::SelectPrevious),
        Key::Named(Named::ArrowDown) => Some(Message::SelectNext),
        _ => None,
    }
}

fn on_event(event: iced::Event, _status: Status, _window: iced::window::Id) -> Option<Message> {
    if let iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(modifiers)) = event {
        return Some(Message::ModifierChanged(modifiers));
    }
    None
}
