use std::{
    collections::VecDeque,
    ptr::null_mut,
    time::{Duration, SystemTime},
};

use crate::{
    subscription::global_event::{Event, EventKind, Input},
    utils::get_window_title_from_hwnd,
};
use iced::{
    futures::{
        SinkExt, Stream, StreamExt,
        channel::mpsc::{Sender, channel},
    },
    stream,
};
use log::{error, info};
use windows::Win32::{
    Foundation,
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook},
        WindowsAndMessaging::{
            EVENT_OBJECT_FOCUS, GetMessageA, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
        },
    },
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

#[derive(Debug)]
struct State {
    mode: Mode,
    current_window_title: Option<String>,
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
    fn new() -> Self {
        Self {
            mode: Mode::Disabled,
            current_window_title: None,
        }
    }

    async fn handle_command(&mut self, command: Command, mut message_sender: Sender<Message>) {
        match command {
            Command::ChangeMode(mode) => {
                message_sender
                    .send(Message::ModeJustSet(mode.clone()))
                    .await // TODO: Use lightweight message instead of copying vec in grab
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
                    .send(Message::SetNextEventsToBeIgnoredByGrabDone)
                    .await
                    .unwrap();
            }
        }
    }

    async fn on_focus_event(&mut self, window_title: String, mut message_sender: Sender<Message>) {
        if self
            .current_window_title
            .as_ref()
            .is_none_or(|title| *title != window_title)
        {
            self.current_window_title = Some(window_title.clone());
            message_sender
                .send(Message::Event(Event {
                    time: SystemTime::now(),
                    kind: EventKind::FocusChange { window_title },
                }))
                .await
                .unwrap();
        }
    }

    async fn on_key_event(
        &mut self,
        event: rdev::Event,
        mut message_sender: Sender<Message>,
    ) -> Option<rdev::Event> {
        // We don't care about mouse events
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
                    .send(Message::Event(Event::new(
                        event.time,
                        EventKind::Input(Input(event.event_type)),
                    )))
                    .await
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
                    .send(Message::Event(Event::new(
                        event.time,
                        EventKind::Input(Input(event.event_type)),
                    )))
                    .await
                    .unwrap();
                None
            }
        }
    }
}

pub fn subscription() -> impl Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        struct GrabMessage {
            event: rdev::Event,
            response_sender: oneshot::Sender<Option<rdev::Event>>,
        }

        enum AllEvent {
            GrabMessage(GrabMessage),
            Command(Command),
            Focus(String),
        }

        let mut listener = State::new();
        let (command_tx, command_rx) = channel(100);
        let (mut grab_event_tx, grab_event_rx) = channel(100);
        std::thread::spawn(move || {
            rdev::grab(move |event| {
                let (response_sender, response_rx) = oneshot::channel();
                grab_event_tx
                    .try_send(GrabMessage {
                        event: event.clone(),
                        response_sender,
                    })
                    .unwrap();
                response_rx
                    .recv_timeout(Duration::from_millis(200))
                    .unwrap_or(Some(event))
            })
            .unwrap()
        });

        let (focus_event_tx, focus_event_rx) = channel(100);
        std::thread::spawn(move || unsafe {
            static mut FOCUS_EVENT_TX: Option<Sender<String>> = None;
            FOCUS_EVENT_TX = Some(focus_event_tx);
            unsafe extern "system" fn callback(
                _hwineventhook: HWINEVENTHOOK,
                event: u32,
                hwnd: Foundation::HWND,
                _idobject: i32,
                _idchild: i32,
                _ideventthread: u32,
                _dwmseventtime: u32,
            ) {
                unsafe {
                    fn is_window_title_ok<S: AsRef<str>>(title: S) -> bool {
                        #[allow(
                            clippy::match_like_matches_macro,
                            reason = "More will be added later"
                        )]
                        match title.as_ref() {
                            "" => false,
                            _ => true,
                        }
                    }

                    if event == EVENT_OBJECT_FOCUS {
                        if let Ok(window_title) = get_window_title_from_hwnd(hwnd) {
                            if is_window_title_ok(&window_title) {
                                let sender_ptr = &raw mut FOCUS_EVENT_TX;
                                if let Some(sender) = &mut *sender_ptr {
                                    // const CLASS_MAX_LEN: usize = 256;
                                    // let mut title = vec![0u16; CLASS_MAX_LEN];
                                    // GetClassNameW(hwnd, title.as_mut_slice());
                                    // let class_name =
                                    //     windows_strings::PWSTR::from_raw(title.as_mut_ptr())
                                    //         .to_string();
                                    // info!("{window_title}: class name: {class_name:?}");
                                    sender.try_send(window_title).unwrap();
                                }
                            }
                        }
                    }
                }
            }

            let hook = SetWinEventHook(
                EVENT_OBJECT_FOCUS,
                EVENT_OBJECT_FOCUS,
                None,
                Some(callback),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            if hook.is_invalid() {
                panic!("Could not start window focus listener");
            }
            info!("Focus hook setup");
            GetMessageA(null_mut(), None, 0, 0).unwrap();
        });

        output.send(Message::Ready(command_tx)).await.unwrap();

        let mut all_event = futures::stream::select(
            command_rx.map(AllEvent::Command),
            futures::stream::select(
                focus_event_rx.map(AllEvent::Focus),
                grab_event_rx.map(AllEvent::GrabMessage),
            ),
        );

        while let Some(event) = all_event.next().await {
            match event {
                AllEvent::GrabMessage(GrabMessage {
                    event,
                    response_sender,
                }) => {
                    let response = listener.on_key_event(event, output.clone()).await;
                    response_sender.send(response).unwrap();
                }
                AllEvent::Command(command) => {
                    listener.handle_command(command, output.clone()).await;
                }
                AllEvent::Focus(window_title) => {
                    listener.on_focus_event(window_title, output.clone()).await;
                }
            }
        }
    })
}
