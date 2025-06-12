use std::time::Duration;

use iced::{
    futures::{
        SinkExt, Stream,
        channel::mpsc::{Receiver, Sender, channel},
    },
    stream,
};
use log::{debug, error, info};
use smol::{Timer, stream::StreamExt};

use crate::{
    subscription::global_event_listener::{Event, EventKind},
    utils::set_focused_window_by_title,
};

pub enum Message {
    SenderReady(Sender<Command>),
    JustPlayed(Event),
    PlaybackDone,
}

#[derive(Debug)]
pub enum Command {
    StartPlaybackWith(Vec<Event>),
    StopPlayback,
}

enum PlayerState {
    Playing { event_index: usize },
    Idle,
}

struct Player {
    events: Vec<Event>,
    state: PlayerState,
}

impl Player {
    fn new() -> Self {
        Self {
            events: Vec::default(),
            state: PlayerState::Idle,
        }
    }

    fn start_playback(&mut self, events: Vec<Event>) {
        self.events = events;
        self.state = PlayerState::Playing { event_index: 0 };
    }

    async fn perform_playback(&mut self) -> Message {
        let PlayerState::Playing { event_index } = &mut self.state else {
            error!("Tried performing playback while being idle");
            return Message::PlaybackDone;
        };

        if *event_index >= self.events.len() {
            info!("Playback done");
            self.stop_playback();
            return Message::PlaybackDone;
        }

        let event = &self.events[*event_index];

        match &event.kind {
            EventKind::Input(event) => {
                rdev::simulate(&event).unwrap();
                Timer::after(Duration::from_millis(32)).await;
            }
            EventKind::FocusChange { window_title } => {
                set_focused_window_by_title(window_title);
            }
            EventKind::Delay(duration) => {
                Timer::after(*duration).await;
            }
        }

        *event_index += 1;

        Message::JustPlayed(event.clone())
    }

    fn stop_playback(&mut self) {
        self.events.clear();
        self.state = PlayerState::Idle;
    }
}

pub fn stream() -> impl Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let mut player = Player::new();
        let (command_tx, mut command_rx) = channel(100);
        output.send(Message::SenderReady(command_tx)).await.unwrap();

        loop {
            match player.state {
                PlayerState::Playing { .. } => {
                    if let Ok(Some(command)) = command_rx.try_next() {
                        match command {
                            Command::StartPlaybackWith(events) => {
                                player.start_playback(events);
                            }
                            Command::StopPlayback => player.stop_playback(),
                        }
                    }
                    let message = player.perform_playback().await;
                    output.send(message).await.unwrap();
                }
                PlayerState::Idle => match command_rx.next().await.unwrap() {
                    Command::StartPlaybackWith(events) => {
                        player.start_playback(events);
                    }
                    Command::StopPlayback => output.send(Message::PlaybackDone).await.unwrap(),
                },
            }
        }
    })
}
