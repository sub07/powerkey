use std::collections::VecDeque;

use iced::{
    futures::{
        SinkExt, Stream,
        channel::mpsc::{Receiver, Sender, channel},
    },
    stream,
};
use rdev::{Event, EventType};

struct EventGrabber {
    simulated: VecDeque<EventType>,
    simulated_rx: Receiver<EventType>,
}

impl EventGrabber {
    fn new() -> (Self, Sender<EventType>) {
        let (tx, rx) = channel(100);
        (
            Self {
                simulated: Default::default(),
                simulated_rx: rx,
            },
            tx,
        )
    }

    fn on_event(
        &mut self,
        event: Event,
        stream_tx: &smol::channel::Sender<Event>,
    ) -> Option<Event> {
        // Drain simulated channel
        while let Ok(Some(simulated_event)) = self.simulated_rx.try_next() {
            self.simulated.push_back(simulated_event);
        }

        // If event is on simulated queue don't grab it to let it reach user apps
        if let Some(front) = self.simulated.front() {
            if front == &event.event_type {
                self.simulated.pop_front();
                return Some(event);
            }
        }

        // If not simulated we grab it
        match event.event_type {
            EventType::KeyPress(_) | EventType::KeyRelease(_) => {
                stream_tx.send_blocking(event).unwrap();
                None
            }
            _ => Some(event),
        }
    }
}

pub enum Message {
    Ready(Sender<EventType>),
    Event(rdev::Event),
}

pub fn stream() -> impl Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let (stream_tx, stream_rx) = smol::channel::unbounded::<Event>();
        let (mut grabber, simulated_tx) = EventGrabber::new();
        std::thread::spawn(move || {
            rdev::_grab(move |event| grabber.on_event(event, &stream_tx)).unwrap()
        });

        output.send(Message::Ready(simulated_tx)).await.unwrap();

        loop {
            let event = stream_rx.recv().await.unwrap();
            output.send(Message::Event(event)).await.unwrap();
        }
    })
}
