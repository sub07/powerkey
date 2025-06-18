use std::{
    collections::{BTreeSet, VecDeque},
    time::{Duration, SystemTime},
};

use iced::{
    futures::{
        SinkExt, Stream,
        channel::mpsc::{Sender, channel},
    },
    stream,
};
use itertools::Itertools;
use log::{error, info, trace, warn};
use smol::{Timer, stream::StreamExt};

use crate::{
    subscription::global_event::{Event, EventKind, Input, listener},
    utils::{get_focused_window_title, set_focused_window_by_title},
};

pub enum Message {
    SenderReady(Sender<Command>),
    PlaybackJustStarted,
    JustPlayed { index: usize },
    PlaybackDone,
}

#[derive(Debug)]
pub enum Command {
    InitializePlayback(Vec<Event>, Sender<listener::Command>),
    NotifyGrabReady,
    StoreMissedEvent(MissedEvent),
    NotifyMissedEventsAddedToGrabber,
    StopPlayback,
}

#[derive(Debug)]
enum PlayingState {
    WaitingForGrabMode,
    Running,
    WaitingForMissedEventsAddedToGrabber { yield_end_time: SystemTime },
}

#[derive(Debug)]
struct YieldContext {
    start_time: SystemTime,
    previous_window_title: String,
}

#[derive(Debug)]
struct Playing {
    event_index: usize,
    listener_command_sender: Sender<listener::Command>,
    events: Vec<Event>,
    state: PlayingState,
    missed_events: BTreeSet<MissedEvent>,
    yield_context: Option<YieldContext>,
}

impl Playing {
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

    pub fn filtered_missed_events(
        &self,
        start: SystemTime,
        end: SystemTime,
    ) -> impl Iterator<Item = rdev::EventType> {
        self.missed_events
            .iter()
            .skip_while(move |missed_event| missed_event.time < start)
            .take_while(move |missed_event| missed_event.time < end)
            .map(|missed_event| missed_event.event)
    }
}

#[derive(Debug)]
enum PlayerState {
    Playing(Playing),
    Idle,
}

#[derive(Debug, Clone)]
pub struct MissedEvent {
    pub event: rdev::EventType,
    pub time: SystemTime,
}

impl Ord for MissedEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time.cmp(&other.time)
    }
}

impl PartialOrd for MissedEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for MissedEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time.eq(&other.time)
    }
}

impl Eq for MissedEvent {}

#[derive(Debug)]
struct Player {
    state: PlayerState,
}

impl Player {
    fn new() -> Self {
        Self {
            state: PlayerState::Idle,
        }
    }

    fn initialize_playback(
        &mut self,
        events: Vec<Event>,
        listener_command_sender: Sender<listener::Command>,
    ) {
        let mut playing = Playing {
            event_index: 0,
            listener_command_sender,
            events,
            state: PlayingState::WaitingForGrabMode,
            missed_events: Default::default(),
            yield_context: None,
        };

        let simulated_events = playing.build_simulated_event_for_grab_mode();

        playing
            .listener_command_sender
            .try_send(listener::Command::ChangeMode(listener::Mode::Grab {
                simulated_events,
            }))
            .unwrap();

        self.state = PlayerState::Playing(playing);
        info!("Player playback initialized: {:#?}", self);
    }

    fn notify_grab_ready(&mut self, mut message_sender: Sender<Message>) {
        let PlayerState::Playing(playing_state) = &mut self.state else {
            error!(
                "Invalid player state when receiving NotifyGrabReady command. Expected Playing state, got {:?}",
                self.state
            );
            return;
        };
        let PlayingState::WaitingForGrabMode = playing_state.state else {
            error!(
                "Invalid player state when receiving NotifyGrabReady command. Expected WaitingForGrabMode state, got {:?}",
                playing_state.state
            );
            return;
        };
        playing_state.state = PlayingState::Running;
        message_sender
            .try_send(Message::PlaybackJustStarted)
            .unwrap();
    }

    async fn perform_playback(&mut self, mut output: Sender<Message>) {
        let PlayerState::Playing(playing_state) = &mut self.state else {
            return;
        };

        if !matches!(playing_state.state, PlayingState::Running) {
            return;
        }

        if playing_state.event_index >= playing_state.events.len() {
            info!("Playback done");
            self.stop_playback();
            output.send(Message::PlaybackDone).await.unwrap();
            return;
        }

        let event = &playing_state.events[playing_state.event_index];

        match &event.kind {
            EventKind::Input(Input(event)) => {
                rdev::simulate(event).unwrap();
                Timer::after(Duration::from_millis(16)).await;
            }
            EventKind::FocusChange { window_title } => {
                if let Ok(window_title) = get_focused_window_title() {
                    playing_state.yield_context = Some(YieldContext {
                        previous_window_title: window_title,
                        start_time: SystemTime::now(),
                    });
                }
                set_focused_window_by_title(window_title);
            }
            EventKind::Delay(duration) => {
                Timer::after(*duration).await;
            }
            EventKind::YieldFocus => {
                if let Some(yield_context) = &playing_state.yield_context {
                    let end_time = SystemTime::now();
                    playing_state.state = PlayingState::WaitingForMissedEventsAddedToGrabber {
                        yield_end_time: end_time,
                    };
                    let yield_time_missed_events = playing_state
                        .filtered_missed_events(yield_context.start_time, end_time)
                        .collect_vec();
                    playing_state
                        .listener_command_sender
                        .send(listener::Command::SetNextEventsToBeIgnoredByGrab(
                            yield_time_missed_events,
                        ))
                        .await
                        .unwrap();
                } else {
                    warn!(
                        "No yield context for yield focus at index {}: Make sure to focus a window before yielding context",
                        playing_state.event_index
                    );
                }
            }
        }

        playing_state.event_index += 1;

        output
            .send(Message::JustPlayed {
                index: playing_state.event_index - 1,
            })
            .await
            .unwrap();
    }

    fn stop_playback(&mut self) {
        self.state = PlayerState::Idle;
    }

    fn store_missed_event(&mut self, event: MissedEvent) {
        let PlayerState::Playing(Playing {
            state: PlayingState::Running,
            missed_events,
            ..
        }) = &mut self.state
        else {
            error!("Expected running player while storing missed event");
            return;
        };
        missed_events.insert(event);
    }

    async fn notify_missed_events_added_to_grabber(&mut self) {
        let PlayerState::Playing(playing_state) = &mut self.state else {
            error!("notify_missed_events_added_to_grabber should not be called if not playing");
            return;
        };

        let PlayingState::WaitingForMissedEventsAddedToGrabber { yield_end_time } =
            playing_state.state
        else {
            error!(
                "notify_missed_events_added_to_grabber should not be called if the player is not waiting for missed events to be added to grabber"
            );
            return;
        };

        let Some(yield_context) = playing_state.yield_context.take() else {
            error!(
                "notify_missed_events_added_to_grabber should not be called without yield context"
            );
            return;
        };

        set_focused_window_by_title(yield_context.previous_window_title);

        for missed_event in
            playing_state.filtered_missed_events(yield_context.start_time, yield_end_time)
        {
            rdev::simulate(&missed_event).unwrap();
            Timer::after(Duration::from_millis(20)).await;
        }

        playing_state.missed_events = playing_state
            .missed_events
            .iter()
            .rev()
            .take_while(|missed_event| missed_event.time > yield_end_time)
            .cloned()
            .collect();

        playing_state.state = PlayingState::Running;
    }
}

pub fn subscription() -> impl Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let mut player = Player::new();
        let (command_tx, mut command_rx) = channel(100);
        output.send(Message::SenderReady(command_tx)).await.unwrap();

        loop {
            let command = if matches!(player.state, PlayerState::Playing(Playing { .. })) {
                command_rx.try_next()
            } else {
                Ok(command_rx.next().await)
            };
            if let Ok(Some(command)) = command {
                trace!("Player command: {command:#?}");
                match command {
                    Command::InitializePlayback(events, sender) => {
                        player.initialize_playback(events, sender)
                    }
                    Command::NotifyGrabReady => player.notify_grab_ready(output.clone()),
                    Command::StoreMissedEvent(missed_event) => {
                        player.store_missed_event(missed_event)
                    }
                    Command::NotifyMissedEventsAddedToGrabber => {
                        player.notify_missed_events_added_to_grabber().await;
                    }
                    Command::StopPlayback => {
                        player.stop_playback();
                        output.send(Message::PlaybackDone).await.unwrap();
                    }
                }
            }
            player.perform_playback(output.clone()).await;
        }
    })
}
