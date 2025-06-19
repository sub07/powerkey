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
use log::trace;
use rdev::EventType;
use serde::{Deserialize, Serialize};

use crate::{
    custom_widget::separator::separator,
    subscription::global_event::{self, Input, player},
    utils::{OrdPairExt, SenderOption, SubscriptionExt},
};

mod mapper;

#[derive(Default, Debug)]
enum PlaybackMode {
    #[default]
    Idle,
    PlayerWaitsForGrab,
    Play,
    Record,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PrintableEvent(global_event::Event);

impl Display for PrintableEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0.kind {
            global_event::EventKind::Input(Input(event)) => match event {
                EventType::KeyPress(key) => write!(f, "Press {key:?}"),
                EventType::KeyRelease(key) => write!(f, "Release {key:?}"),
                _ => unreachable!("mouse event not supported"),
            },
            global_event::EventKind::FocusChange { window_title, .. } => {
                write!(f, "Window changed to \"{window_title}\"")
            }
            global_event::EventKind::Delay(duration) => {
                write!(f, "{}ms delay", duration.as_millis())
            }
            global_event::EventKind::YieldFocus => {
                write!(f, "Restore previous window and simulate its inputs")
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
    global_event_listener_command_sender: Option<Sender<global_event::listener::Command>>,
    global_event_player_command_sender: Option<Sender<global_event::player::Command>>,
    current_listener_mode: global_event::listener::Mode,
    playback_mode: PlaybackMode,
    items: Vec<PrintableEvent>,
    selected_items_state: ItemSelectionState,
    item_list_scroll_viewport: Option<Viewport>,
    item_list_scroll_id: iced::widget::scrollable::Id,
    window_id: Option<iced::window::Id>,
    always_on_top: bool,
    modifiers: Modifiers,
}

#[derive(Debug, Clone)]
pub enum GlobalEventTrigger {
    ListenerReady(Sender<global_event::listener::Command>),
    ListenerModeJustChanged(global_event::listener::Mode),
    ListenerAddGrabIgnoreListDone,

    PlayerReady(Sender<global_event::player::Command>),
    PlayerPlaybackJustStarted,
    PlayerPlaybackJustEnded,
    PlayerJustPlayed(usize),

    Event(global_event::Event),
}

#[derive(Debug, Clone)]
pub enum Command {
    StartRecording,
    StartPlayback,
    Stop,
    SetAlwaysOnTop(bool),
    TriggerWindowId,
    SetWindowId(iced::window::Id),
    UpdateModifiers(Modifiers),
    AddYieldEventAfterSelected,
    ItemList(ListCommand),
}

#[derive(Debug, Clone)]
pub enum Trigger {
    RecordButton,
    PlayButton,
    StopButton,
    AddYieldButton,
    AlwaysOnTopCheckbox(bool),
    WindowId(iced::window::Id),
    GlobalEvent(GlobalEventTrigger),
}

#[derive(Debug, Clone)]
pub enum ListCommand {
    SelectItem(usize),
    SelectNext,
    SelectPrevious,
    DeleteItem,
    SetScrollableViewport(Viewport),
}

#[derive(Debug, Clone)]
pub enum Message {
    Command(Command),
    Trigger(Trigger),
}

impl State {
    pub fn new() -> (State, Task<Message>) {
        let items = std::fs::read_to_string("macro.json")
            .map_err(|e| e.to_string())
            .and_then(|content| {
                serde_json::from_str::<Vec<PrintableEvent>>(&content).map_err(|e| e.to_string())
            })
            .unwrap_or_default();
        let always_on_top = true;
        let state = State {
            global_event_listener_command_sender: Default::default(),
            global_event_player_command_sender: Default::default(),
            playback_mode: Default::default(),
            current_listener_mode: Default::default(),
            items,
            selected_items_state: Default::default(),
            item_list_scroll_viewport: Default::default(),
            item_list_scroll_id: iced::widget::scrollable::Id::unique(),
            window_id: None,
            always_on_top,
            modifiers: Modifiers::default(),
        };
        (
            state,
            Task::done(Message::Command(Command::SetAlwaysOnTop(true))),
        )
    }

    pub fn title(_state: &State) -> String {
        "Powerkey".into()
    }

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
        } else {
            log::warn!("Attempt to scroll but scrollable viewport isnt't set");
        }
        Task::none()
    }

    fn handle_command(&mut self, command: Command) -> Task<Message> {
        match command {
            Command::StartRecording => {
                self.playback_mode = PlaybackMode::Record;
                self.items.clear();
                self.global_event_listener_command_sender
                    .try_send(global_event::listener::Command::ChangeMode(
                        global_event::listener::Mode::Listen,
                    ))
                    .unwrap();
            }
            Command::StartPlayback => {
                if let Some(listener_command_sender) =
                    self.global_event_listener_command_sender.as_ref().cloned()
                {
                    self.global_event_player_command_sender
                        .try_send(global_event::player::Command::InitializePlayback(
                            self.items
                                .clone()
                                .into_iter()
                                .map(|event| event.0)
                                .collect_vec(),
                            listener_command_sender,
                        ))
                        .unwrap();
                    self.playback_mode = PlaybackMode::PlayerWaitsForGrab;
                }
            }
            Command::Stop => {
                if let PlaybackMode::Play = &self.playback_mode {}
                if !matches!(
                    self.current_listener_mode,
                    global_event::listener::Mode::Disabled
                ) {
                    self.global_event_listener_command_sender
                        .try_send(global_event::listener::Command::ChangeMode(
                            global_event::listener::Mode::Disabled,
                        ))
                        .unwrap();
                }

                if !matches!(self.playback_mode, PlaybackMode::Idle) {
                    self.global_event_player_command_sender
                        .try_send(global_event::player::Command::StopPlayback)
                        .unwrap();
                }

                self.playback_mode = PlaybackMode::Idle;

                // std::fs::write("macro.json", serde_json::to_string(&self.items).unwrap()).unwrap();
            }
            Command::SetAlwaysOnTop(always_on_top) => {
                if let Some(window_id) = self.window_id {
                    self.always_on_top = always_on_top;
                    return iced::window::change_level(
                        window_id,
                        if always_on_top {
                            Level::AlwaysOnTop
                        } else {
                            Level::Normal
                        },
                    );
                } else {
                    return Task::done(Message::Command(Command::TriggerWindowId)).chain(
                        Task::done(Message::Command(Command::SetAlwaysOnTop(always_on_top))),
                    );
                }
            }
            Command::TriggerWindowId => {
                return iced::window::get_oldest()
                    .and_then(|id| Task::done(Message::Trigger(Trigger::WindowId(id))));
            }
            Command::UpdateModifiers(modifiers) => self.modifiers = modifiers,
            Command::AddYieldEventAfterSelected => {
                let yield_event = PrintableEvent(global_event::Event::new(
                    SystemTime::now(),
                    global_event::EventKind::YieldFocus,
                ));
                if let Some(last_selected_index) = self.selected_items_state.get_last_selected() {
                    self.items.insert(last_selected_index + 1, yield_event);
                } else {
                    self.items.push(yield_event);
                }
            }
            Command::SetWindowId(id) => self.window_id = Some(id),
            Command::ItemList(command) => return self.handle_list_command(command),
        }
        Task::none()
    }

    fn handle_list_command(&mut self, command: ListCommand) -> Task<Message> {
        match command {
            ListCommand::SelectItem(index) => {
                if self.modifiers.control() {
                    self.selected_items_state.add_item_to_selection(index);
                } else if self.modifiers.shift() {
                    self.selected_items_state.expand_to(index);
                } else {
                    self.selected_items_state.select(index);
                }
            }
            ListCommand::SelectNext => {
                if let Some(last_item_selected) = self.selected_items_state.get_last_selected() {
                    let next_index = last_item_selected + 1;
                    let next_index = next_index.clamp(0, self.items.len() - 1);
                    self.selected_items_state.select(next_index);
                    return self.scroll_to_item_task();
                }
            }
            ListCommand::SelectPrevious => {
                if let Some(last_item_selected) = self.selected_items_state.get_first_selected() {
                    let next_index = last_item_selected.saturating_sub(1);
                    self.selected_items_state.select(next_index);
                    return self.scroll_to_item_task();
                }
            }
            ListCommand::DeleteItem => {
                if let Some(first_item_selected) = self.selected_items_state.get_first_selected() {
                    for (index_index, index_to_delete) in
                        self.selected_items_state.iter().enumerate()
                    {
                        self.items.remove(index_to_delete - index_index);
                    }
                    if self.items.is_empty() {
                        self.selected_items_state.unselect()
                    } else {
                        self.selected_items_state
                            .select(first_item_selected.clamp(0, self.items.len() - 1));
                    }
                    return Task::done(Message::Command(Command::Stop));
                }
            }
            ListCommand::SetScrollableViewport(viewport) => {
                self.item_list_scroll_viewport = Some(viewport);
            }
        }
        Task::none()
    }

    fn handle_trigger(&mut self, trigger: Trigger) -> Task<Message> {
        match trigger {
            Trigger::RecordButton => Task::done(Message::Command(Command::StartRecording)),
            Trigger::PlayButton => Task::done(Message::Command(Command::StartPlayback)),
            Trigger::StopButton => Task::done(Message::Command(Command::Stop)),
            Trigger::AlwaysOnTopCheckbox(checked) => {
                Task::done(Message::Command(Command::SetAlwaysOnTop(checked)))
            }
            Trigger::WindowId(id) => Task::done(Message::Command(Command::SetWindowId(id))),
            Trigger::GlobalEvent(global_event_message) => {
                self.handle_global_event_message(global_event_message)
            }
            Trigger::AddYieldButton => {
                Task::done(Message::Command(Command::AddYieldEventAfterSelected))
            }
        }
    }

    fn handle_global_event_message(
        &mut self,
        global_event_message: GlobalEventTrigger,
    ) -> Task<Message> {
        match global_event_message {
            GlobalEventTrigger::ListenerReady(sender) => {
                self.global_event_listener_command_sender = Some(sender);
            }
            GlobalEventTrigger::ListenerModeJustChanged(mode) => {
                if matches!(self.playback_mode, PlaybackMode::PlayerWaitsForGrab)
                    && matches!(mode, global_event::listener::Mode::Grab { .. })
                {
                    self.global_event_player_command_sender
                        .try_send(global_event::player::Command::NotifyGrabReady)
                        .unwrap();
                }
                self.current_listener_mode = mode;
            }
            GlobalEventTrigger::PlayerReady(sender) => {
                self.global_event_player_command_sender = Some(sender);
            }
            GlobalEventTrigger::PlayerPlaybackJustEnded => {
                return Task::done(Message::Command(Command::Stop));
            }
            GlobalEventTrigger::PlayerJustPlayed(index) => {
                self.selected_items_state.select(index);
            }
            GlobalEventTrigger::Event(event) => self.handle_global_event(event),
            GlobalEventTrigger::PlayerPlaybackJustStarted => {
                self.playback_mode = PlaybackMode::Play;
            }
            GlobalEventTrigger::ListenerAddGrabIgnoreListDone => {
                self.global_event_player_command_sender
                    .try_send(player::Command::NotifyMissedEventsAddedToGrabber)
                    .unwrap();
            }
        }
        Task::none()
    }

    fn handle_global_event(&mut self, event: global_event::Event) {
        match (&self.current_listener_mode, &mut self.playback_mode) {
            (global_event::listener::Mode::Listen, PlaybackMode::Record) => {
                if let Some(previous_event) = self.items.last() {
                    if let Ok(delay) = event.time.duration_since(previous_event.0.time) {
                        self.items.push(PrintableEvent(global_event::Event::new(
                            SystemTime::now(),
                            global_event::EventKind::Delay(delay),
                        )));
                    }
                }
                self.items.push(PrintableEvent(event));
            }
            (global_event::listener::Mode::Grab { .. }, PlaybackMode::Play) => {
                if let global_event::Event {
                    kind: global_event::EventKind::Input(Input(event)),
                    time,
                } = event
                {
                    self.global_event_player_command_sender
                        .try_send(global_event::player::Command::StoreMissedEvent(
                            global_event::player::MissedEvent { event, time },
                        ))
                        .unwrap()
                }
            }
            _ => {}
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        trace!("Main window update: {message:?}");
        match message {
            Message::Command(command) => self.handle_command(command),
            Message::Trigger(trigger) => self.handle_trigger(trigger),
        }
    }

    pub fn view(&self) -> Element<Message> {
        let items = column(
            #[allow(unstable_name_collisions)]
            self.items
                .iter()
                .enumerate()
                .map(|(index, event)| list_item(index, event, &self.selected_items_state))
                .intersperse_with(|| separator().into()),
        );

        column![
            row![
                column![
                    text(format!("{:?}", self.current_listener_mode)),
                    text(format!("{:?}", self.playback_mode)),
                ],
                checkbox("Always on top", self.always_on_top)
                    .on_toggle(|value| Message::Trigger(Trigger::AlwaysOnTopCheckbox(value)))
            ]
            .spacing(8.0)
            .height(Length::Shrink),
            row![
                button(text!("Record")).on_press(Message::Trigger(Trigger::RecordButton)),
                button(text!("Play")).on_press(Message::Trigger(Trigger::PlayButton)),
                button(text!("Stop")).on_press(Message::Trigger(Trigger::StopButton)),
                button(text!("Add yield")).on_press(Message::Trigger(Trigger::AddYieldButton)),
            ]
            .spacing(4.0),
            if self.items.is_empty() {
                Element::new(container(text("Press record !").size(24.0)).center(Length::Fill))
            } else {
                Element::new(
                    widget::scrollable(items)
                        .spacing(8.0)
                        .id(self.item_list_scroll_id.clone())
                        .on_scroll(|viewport| {
                            Message::Command(Command::ItemList(ListCommand::SetScrollableViewport(
                                viewport,
                            )))
                        }),
                )
            },
        ]
        .spacing(4.0)
        .into()
    }
}

pub fn theme(_state: &State) -> iced::Theme {
    Theme::Ferra
}

fn list_item<'a, 'b: 'a>(
    index: usize,
    event: &'b PrintableEvent,
    selected_items_state: &'a ItemSelectionState,
) -> Element<'a, Message> {
    let selected = selected_items_state.is_selected(index);
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
    .on_press(Message::Command(Command::ItemList(
        ListCommand::SelectItem(index),
    )))
    .into()
}

pub fn subscription(_state: &State) -> Subscription<Message> {
    let global_event_listener = Subscription::run(global_event::listener::subscription).map_into();
    let global_event_player = Subscription::run(global_event::player::subscription).map_into();

    let local_keyevent_listener = iced::keyboard::on_key_press(on_key_press);
    let local_event_listener = iced::event::listen_with(on_event);

    Subscription::batch([
        global_event_listener,
        local_keyevent_listener,
        local_event_listener,
        global_event_player,
    ])
}

fn on_key_press(key: Key, _modifiers: Modifiers) -> Option<Message> {
    match key {
        Key::Named(Named::Delete) => {
            Some(Message::Command(Command::ItemList(ListCommand::DeleteItem)))
        }
        Key::Named(Named::ArrowUp) => Some(Message::Command(Command::ItemList(
            ListCommand::SelectPrevious,
        ))),
        Key::Named(Named::ArrowDown) => {
            Some(Message::Command(Command::ItemList(ListCommand::SelectNext)))
        }
        _ => None,
    }
}

fn on_event(event: iced::Event, _status: Status, _window: iced::window::Id) -> Option<Message> {
    if let iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(modifiers)) = event {
        return Some(Message::Command(Command::UpdateModifiers(modifiers)));
    }
    None
}
