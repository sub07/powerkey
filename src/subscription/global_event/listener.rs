use std::{collections::VecDeque, time::SystemTime};

use crate::{
    subscription::global_event::{Event, EventKind, Input},
    utils::get_focused_window_title,
};
use iced::{
    futures::{
        SinkExt, Stream, StreamExt,
        channel::mpsc::{Receiver, Sender, channel},
    },
    stream,
};
use log::{debug, error, info, trace};

#[derive(Default, Clone, Debug)]
pub enum Mode {
    #[default]
    Disabled,
    Listen,
    Grab {
        simulated_events: VecDeque<rdev::EventType>,
    },
}

#[derive(Debug)]
struct State {
    mode: Mode,
    command_rx: Receiver<Command>,
    current_window_title: String,
}

#[derive(Debug)]
pub enum Command {
    ChangeMode(Mode),
    SetNextEventsToBeIgnoredByGrab(Vec<rdev::EventType>),
}

#[derive(Debug)]
pub enum Message {
    Ready(Sender<Command>),
    ModeJustSet(Mode),
    SetNextEventsToBeIgnoredByGrabDone,
    Event(Event),
}

impl State {
    fn new() -> (Self, Sender<Command>) {
        let (tx, rx) = channel(100);
        (
            Self {
                mode: Mode::Disabled,
                command_rx: rx,
                current_window_title: get_focused_window_title()
                    .unwrap_or("Could not get window".into()),
            },
            tx,
        )
    }

    fn handle_command(&mut self, command: Command, mut message_sender: Sender<Message>) {
        match command {
            Command::ChangeMode(mode) => {
                message_sender
                    .try_send(Message::ModeJustSet(mode.clone())) // TODO: Use lightweight message instead of copying vec in grab
                    .unwrap();
                self.mode = mode;
                info!("Listener: mode set to {:#?}", self.mode);
            }
            Command::SetNextEventsToBeIgnoredByGrab(events) => {
                let Mode::Grab { simulated_events } = &mut self.mode else {
                    error!(
                        "Trying to add more to grabber ignore list while being in {:?} mode",
                        self.mode
                    );
                    return;
                };
                for event in events.into_iter().rev() {
                    simulated_events.push_front(event);
                }
                message_sender
                    .try_send(Message::SetNextEventsToBeIgnoredByGrabDone)
                    .unwrap();
            }
        }
    }

    fn on_event(
        &mut self,
        event: rdev::Event,
        mut message_sender: Sender<Message>,
    ) -> Option<rdev::Event> {
        // Handle commands
        while let Ok(Some(command)) = self.command_rx.try_next() {
            trace!("Listener command: {command:#?}");
            self.handle_command(command, message_sender.clone());
        }

        if let Mode::Disabled = self.mode {
            return Some(event);
        }

        if let Mode::Listen = self.mode {
            if let Some(current_window_title) = get_focused_window_title()
                .ok()
                .filter(|title| filter_window_title(title))
            {
                if current_window_title != self.current_window_title {
                    self.current_window_title = current_window_title.to_owned();
                    message_sender
                        .try_send(Message::Event(Event::new(
                            SystemTime::now(),
                            EventKind::FocusChange {
                                window_title: self.current_window_title.clone(),
                            },
                        )))
                        .unwrap();
                }
            }
        }

        // We don't care about mouse events
        // Keep this after command pumping to allow mouse event to trigger it.
        // Alternative would be to wake this callback on a regular basis if no event is triggered
        if let rdev::Event {
            event_type:
                rdev::EventType::Wheel { .. }
                | rdev::EventType::MouseMove { .. }
                | rdev::EventType::ButtonPress(_)
                | rdev::EventType::ButtonRelease(_),
            ..
        } = event
        {
            return Some(event);
        }

        match &mut self.mode {
            Mode::Disabled => unreachable!("Disabled is short circuited"),
            Mode::Listen => {
                message_sender
                    .try_send(Message::Event(Event::new(
                        event.time,
                        EventKind::Input(Input(event.event_type)),
                    )))
                    .unwrap();
                Some(event)
            }
            Mode::Grab { simulated_events } => {
                if let Some(simulated_event) = simulated_events.front() {
                    if event.event_type == *simulated_event {
                        return Some(event);
                    }
                }
                message_sender
                    .try_send(Message::Event(Event::new(
                        event.time,
                        EventKind::Input(Input(event.event_type)),
                    )))
                    .unwrap();
                None
            }
        }
    }
}

fn filter_window_title<S: AsRef<str>>(title: S) -> bool {
    #[allow(clippy::match_like_matches_macro, reason = "More will be added later")]
    match title.as_ref() {
        "" => false,
        _ => true,
    }
}

pub fn subscription() -> impl Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let (stream_tx, mut stream_rx) = channel(100);
        let (mut event_listener, simulated_tx) = State::new();
        std::thread::spawn(move || {
            rdev::grab(move |event| event_listener.on_event(event, stream_tx.clone())).unwrap()
        });

        output.send(Message::Ready(simulated_tx)).await.unwrap();

        loop {
            debug!("Listener: listening grab thread relay...");
            let message = stream_rx.next().await.unwrap();
            debug!("Listener: relaying message to subscription: {message:?}");
            output.send(message).await.unwrap();
        }
    })
}
