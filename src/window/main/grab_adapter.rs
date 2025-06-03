use crate::{subscription, window::main};

impl From<subscription::grab::Message> for main::Message {
    fn from(event: subscription::grab::Message) -> Self {
        match event {
            subscription::grab::Message::Ready(sender) => {
                main::Message::GrabSimulatedSender(sender)
            }
            subscription::grab::Message::Event(event) => main::Message::GrabbedEvent(event),
        }
    }
}
