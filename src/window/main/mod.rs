use iced::{Element, Subscription, futures::channel::mpsc::Sender, widget::text};
use rdev::EventType;

use crate::{subscription, utils::SubscriptionExt};

mod grab_adapter;

#[derive(Default)]
pub struct State {
    simulated_sender: Option<Sender<EventType>>,
}

#[derive(Debug)]
pub enum Message {
    GrabbedEvent(rdev::Event),
    GrabSimulatedSender(Sender<EventType>),
}

pub fn title(_state: &State) -> String {
    "Powerkey".into()
}

pub fn update(state: &mut State, message: Message) {
    match message {
        Message::GrabbedEvent(event) => {
            println!("replaying {event:?}");
            if let Some(simulated_sender) = state.simulated_sender.as_mut() {
                simulated_sender.try_send(event.event_type).unwrap();
                rdev::simulate(&event.event_type).unwrap();
            }
        }
        Message::GrabSimulatedSender(sender) => state.simulated_sender = Some(sender),
    }
}

pub fn view(state: &State) -> Element<Message> {
    text("hello").into()
}

pub fn subscription(_state: &State) -> Subscription<Message> {
    Subscription::run(subscription::grab::stream).map_into()
}
