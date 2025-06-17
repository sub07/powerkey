use std::{
    collections::VecDeque,
    time::{Duration, SystemTime},
};

use iced::{
    futures::{
        SinkExt, Stream,
        channel::mpsc::{Sender, channel},
    },
    stream,
};
use log::{error, info};
use smol::{Timer, stream::StreamExt};

use crate::{
    subscription::global_event::{Event, EventKind, Input, listener},
    utils::{get_focused_window_title, set_focused_window_by_title},
};

pub enum Message {
    SenderReady(Sender<Command>),
    JustPlayed { index: usize },
    PlaybackDone,
}

#[derive(Debug)]
pub enum Command {
    StartPlaybackWith(Vec<Event>, Sender<listener::Command>),
    StoreMissedEvent(MissedEvent),
    StopPlayback,
}

struct PlayingState {
    event_index: usize,
    listener_sender: Sender<listener::Command>,
    events: Vec<Event>,
}

impl PlayingState {
    pub fn build_simulated_event_for_grab_mode(&self) -> VecDeque<rdev::EventType> {
        self.events[self.event_index..]
            .iter()
            .take_while(|event| !matches!(event.kind, EventKind::YieldFocus))
            .filter_map(|event| match event.kind {
                EventKind::Input(Input(event_type)) => Some(event_type),
                _ => None,
            })
            .collect()
    }
}

enum PlayerState {
    Playing(PlayingState),
    Idle,
}

#[derive(Debug)]
pub struct MissedEvent {
    pub event: rdev::EventType,
    pub time: SystemTime,
}

struct YieldContext {
    window_title_before_last_focus: String,
    missed_events: Vec<MissedEvent>,
}

struct Player {
    state: PlayerState,
    yield_context: Option<YieldContext>,
}

impl Player {
    fn new() -> Self {
        Self {
            state: PlayerState::Idle,
            yield_context: None,
        }
    }

    fn start_playback(&mut self, events: Vec<Event>, listener_sender: Sender<listener::Command>) {
        self.state = PlayerState::Playing(PlayingState {
            event_index: 0,
            listener_sender,
            events,
        });
    }

    async fn perform_playback(&mut self) -> Message {
        let PlayerState::Playing(playing_state) = &mut self.state else {
            error!("Tried performing playback while being idle");
            return Message::PlaybackDone;
        };

        if playing_state.event_index >= playing_state.events.len() {
            info!("Playback done");
            self.stop_playback();
            return Message::PlaybackDone;
        }

        let event = &playing_state.events[playing_state.event_index];

        match &event.kind {
            EventKind::Input(Input(event)) => {
                rdev::simulate(event).unwrap();
                Timer::after(Duration::from_millis(16)).await;
            }
            EventKind::FocusChange { window_title } => {
                if let Ok(window_title) = get_focused_window_title() {
                    playing_state
                        .listener_sender
                        .send(listener::Command::ChangeMode(listener::Mode::Grab {
                            simulated_events: playing_state.build_simulated_event_for_grab_mode(),
                        }))
                        .await
                        .unwrap();
                    self.yield_context = Some(YieldContext {
                        window_title_before_last_focus: window_title,
                        missed_events: Default::default(),
                    });
                }
                set_focused_window_by_title(window_title);
            }
            EventKind::Delay(duration) => {
                Timer::after(*duration).await;
            }
            EventKind::YieldFocus => {
                if let Some(mut yield_context) = self.yield_context.take() {
                    playing_state
                        .listener_sender
                        .send(listener::Command::ChangeMode(listener::Mode::Disabled))
                        .await
                        .unwrap();

                    set_focused_window_by_title(yield_context.window_title_before_last_focus);
                    yield_context.missed_events.sort_by_key(|e| e.time);

                    for missed_event in yield_context.missed_events {
                        rdev::simulate(&missed_event.event).unwrap();
                        Timer::after(Duration::from_millis(16)).await;
                    }
                }
            }
        }

        playing_state.event_index += 1;

        Message::JustPlayed {
            index: playing_state.event_index - 1,
        }
    }

    fn stop_playback(&mut self) {
        self.state = PlayerState::Idle;
    }

    fn store_missed_event(&mut self, event: MissedEvent) {
        if let Some(yield_context) = &mut self.yield_context {
            yield_context.missed_events.push(event);
        }
    }
}

pub fn subscription() -> impl Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let mut player = Player::new();
        let (command_tx, mut command_rx) = channel(100);
        output.send(Message::SenderReady(command_tx)).await.unwrap();

        loop {
            match player.state {
                PlayerState::Playing(PlayingState { .. }) => {
                    if let Ok(Some(command)) = command_rx.try_next() {
                        match command {
                            Command::StartPlaybackWith(events, listener_sender) => {
                                player.start_playback(events, listener_sender);
                            }
                            Command::StopPlayback => player.stop_playback(),
                            Command::StoreMissedEvent(event) => player.store_missed_event(event),
                        }
                    }
                    let message = player.perform_playback().await;
                    output.send(message).await.unwrap();
                }
                PlayerState::Idle => match command_rx.next().await.unwrap() {
                    Command::StartPlaybackWith(events, listener_sender) => {
                        player.start_playback(events, listener_sender);
                    }
                    Command::StopPlayback => output.send(Message::PlaybackDone).await.unwrap(),
                    Command::StoreMissedEvent(_) => {
                        error!("Should not happen");
                    }
                },
            }
        }
    })
}
