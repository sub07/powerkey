use std::{collections::VecDeque, time::SystemTime};

use iced::{
    futures::{
        SinkExt, Stream,
        channel::mpsc::{Receiver, Sender, channel},
    },
    stream,
};
use log::info;

use crate::{
    subscription::global_event::{Event, EventKind, Input},
    utils::get_focused_window_title,
};

#[derive(Default, Clone, Debug)]
pub enum Mode {
    #[default]
    Disabled,
    Listen,
    Grab {
        simulated_events: VecDeque<rdev::EventType>,
    },
}

struct State {
    mode: Mode,
    command_rx: Receiver<Command>,
    current_window_title: String,
}

#[derive(Debug)]
pub enum Command {
    ChangeMode(Mode),
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

    fn handle_command(
        &mut self,
        command: Command,
        message_sender: &smol::channel::Sender<Message>,
    ) {
        match command {
            Command::ChangeMode(mode) => {
                message_sender
                    .send_blocking(Message::ModeJustSet(mode.clone())) // Use lightweight message instead of copying vec in grab
                    .unwrap();
                self.mode = mode;
            }
        }
    }

    fn on_event(
        &mut self,
        event: rdev::Event,
        message_sender: &smol::channel::Sender<Message>,
    ) -> Option<rdev::Event> {
        // TODO: Try to offload the event processing away from the hook callback

        // Handle commands
        while let Ok(Some(command)) = self.command_rx.try_next() {
            info!("Handle command {command:?} in global event listener");
            self.handle_command(command, message_sender);
        }

        fn filter_window_title<S: AsRef<str>>(title: S) -> bool {
            match title.as_ref() {
                "" => false,
                _ => true,
            }
        }

        if let Some(current_window_title) = get_focused_window_title()
            .ok()
            .filter(|title| filter_window_title(title))
        {
            if current_window_title != self.current_window_title {
                self.current_window_title = current_window_title.to_owned();
                message_sender
                    .send_blocking(Message::Event(Event::new(
                        SystemTime::now(),
                        EventKind::FocusChange {
                            window_title: self.current_window_title.clone(),
                        },
                    )))
                    .unwrap();
            }
        }

        // We don't care about mouse events
        // Keep this after command pumping to allow mouse event to trigger it. Alternative would be to wake this callback on a regular basis if no event is shot
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
            Mode::Disabled => Some(event),
            Mode::Listen => {
                message_sender
                    .send_blocking(Message::Event(Event::new(
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
                    .send_blocking(Message::Event(Event::new(
                        event.time,
                        EventKind::Input(Input(event.event_type)),
                    )))
                    .unwrap();
                None
            }
        }
    }
}

pub enum Message {
    Ready(Sender<Command>),
    ModeJustSet(Mode),
    Event(Event),
}

pub fn subscription() -> impl Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let (stream_tx, stream_rx) = smol::channel::unbounded::<Message>();
        let (mut event_listener, simulated_tx) = State::new();
        std::thread::spawn(move || {
            rdev::grab(move |event| event_listener.on_event(event, &stream_tx)).unwrap()
        });

        output.send(Message::Ready(simulated_tx)).await.unwrap();

        loop {
            let message = stream_rx.recv().await.unwrap();
            output.send(message).await.unwrap();
        }
    })
}
