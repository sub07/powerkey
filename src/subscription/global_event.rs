use std::{collections::VecDeque, time::Duration};

use iced::{
    futures::{
        SinkExt, Stream,
        channel::mpsc::{Receiver, Sender, channel},
        select,
    },
    stream,
};
use rdev::{Event, EventType};
use smol::Timer;

#[derive(Default, Clone, Debug)]
pub enum ListenerMode {
    #[default]
    Disabled,
    Listen,
    Grab,
}

struct GlobalEventListener {
    mode: ListenerMode,
    command_rx: Receiver<ListenerCommand>,
}

#[derive(Debug)]
pub enum ListenerCommand {
    SetMode(ListenerMode),
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
            ListenerCommand::SetMode(mode) => match mode {
                ListenerMode::Disabled => {
                    self.mode = ListenerMode::Disabled;
                    message_sender
                        .send_blocking(Message::ModeJustSet(ListenerMode::Disabled))
                        .unwrap();
                }
                ListenerMode::Listen => {
                    self.mode = ListenerMode::Listen;
                    message_sender
                        .send_blocking(Message::ModeJustSet(ListenerMode::Listen))
                        .unwrap();
                }
                ListenerMode::Grab => {
                    self.mode = ListenerMode::Grab;
                    message_sender
                        .send_blocking(Message::ModeJustSet(ListenerMode::Grab))
                        .unwrap();
                }
            },
        }
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

        // We don't care about mouse events
        // Keep this after command pumping to allow mouse event to trigger it. Alternative would be to wake this callback on a regular basis if no event is shot
        if let Event {
            event_type:
                EventType::Wheel { .. }
                | EventType::MouseMove { .. }
                | EventType::ButtonPress(_)
                | EventType::ButtonRelease(_),
            ..
        } = event
        {
            return Some(event);
        }

        match &mut self.mode {
            ListenerMode::Disabled => Some(event),
            ListenerMode::Listen => {
                message_sender
                    .send_blocking(Message::Event(event.clone()))
                    .unwrap();
                Some(event)
            }
            ListenerMode::Grab => {
                message_sender
                    .send_blocking(Message::Event(event.clone()))
                    .unwrap();
                None
            }
        }
    }
}

pub enum Message {
    Ready(Sender<ListenerCommand>),
    ModeJustSet(ListenerMode),
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
            output.send(message).await.unwrap();
        }
    })
}
