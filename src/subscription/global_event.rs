use std::collections::VecDeque;

use iced::{
    futures::{
        SinkExt, Stream,
        channel::mpsc::{Receiver, Sender, channel},
    },
    stream,
};
use rdev::{Event, EventType};

enum ListenerMode {
    Disabled,
    Listen,
    Grab { simulated: VecDeque<EventType> },
}

struct GlobalEventListener {
    mode: ListenerMode,
    command_rx: Receiver<ListenerCommand>,
}

#[derive(Debug)]
pub enum ListenerCommand {
    SetDisabledMode,
    SetListenMode,
    SetGrabMode,
    AddSimulatedInput(EventType),
}

impl GlobalEventListener {
    fn new() -> (Self, Sender<ListenerCommand>) {
        let (tx, rx) = channel(100);
        (
            Self {
                mode: ListenerMode::Disabled,
                command_rx: rx,
            },
            tx,
        )
    }

    fn handle_command(
        &mut self,
        command: ListenerCommand,
        message_sender: &smol::channel::Sender<Message>,
    ) {
        match command {
            ListenerCommand::SetDisabledMode => {
                self.mode = ListenerMode::Disabled;
                message_sender
                    .send_blocking(Message::DisabledModeSet)
                    .unwrap();
            }
            ListenerCommand::SetListenMode => {
                self.mode = ListenerMode::Listen;
                message_sender
                    .send_blocking(Message::ListenModeSet)
                    .unwrap();
            }
            ListenerCommand::SetGrabMode => {
                self.mode = ListenerMode::Grab {
                    simulated: Default::default(),
                };
                message_sender.send_blocking(Message::GrabModeSet).unwrap();
            }
            ListenerCommand::AddSimulatedInput(simulated_event) => {
                if let ListenerMode::Grab { simulated } = &mut self.mode {
                    simulated.push_back(simulated_event);
                } else {
                    debug_assert!(false); // TODO: panic on debug, log on release
                }
            }
        }
    }

    fn handle_grab(
        &mut self,
        event: Event,
        message_sender: &smol::channel::Sender<Message>,
    ) -> Option<Event> {
        let ListenerMode::Grab { simulated } = &mut self.mode else {
            debug_assert!(false, "handle_grab called without being in grab mode");
            return Some(event);
        };

        // If event is on simulated queue don't grab it to allow it reach user apps
        if let Some(front) = simulated.front() {
            if front == &event.event_type {
                simulated.pop_front();
                return Some(event);
            }
        }

        // If not simulated we grab it
        match event.event_type {
            EventType::KeyPress(_) | EventType::KeyRelease(_) => {
                message_sender.send_blocking(Message::Event(event)).unwrap();
                None
            }
            _ => Some(event),
        }
    }

    fn handle_listen(
        &mut self,
        event: Event,
        message_sender: &smol::channel::Sender<Message>,
    ) -> Option<Event> {
        message_sender
            .send_blocking(Message::Event(event.clone()))
            .unwrap();
        Some(event)
    }

    fn on_event(
        &mut self,
        event: Event,
        message_sender: &smol::channel::Sender<Message>,
    ) -> Option<Event> {
        // Handle commands
        while let Ok(Some(command)) = self.command_rx.try_next() {
            self.handle_command(command, message_sender);
        }

        match &mut self.mode {
            ListenerMode::Disabled => Some(event),
            ListenerMode::Listen => self.handle_listen(event, message_sender),
            ListenerMode::Grab { .. } => self.handle_grab(event, message_sender),
        }
    }
}

pub enum Message {
    Ready(Sender<ListenerCommand>),
    DisabledModeSet,
    ListenModeSet,
    GrabModeSet,
    Event(rdev::Event),
}

pub fn stream() -> impl Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let (stream_tx, stream_rx) = smol::channel::unbounded::<Message>();
        let (mut grabber, simulated_tx) = GlobalEventListener::new();
        std::thread::spawn(move || {
            rdev::_grab(move |event| grabber.on_event(event, &stream_tx)).unwrap()
        });

        output.send(Message::Ready(simulated_tx)).await.unwrap();

        loop {
            let message = stream_rx.recv().await.unwrap();
            match message {
                Message::Event(Event {
                    event_type:
                        EventType::Wheel { .. }
                        | EventType::MouseMove { .. }
                        | EventType::ButtonPress(_)
                        | EventType::ButtonRelease(_),
                    ..
                }) => {}
                _ => output.send(message).await.unwrap(),
            }
        }
    })
}
