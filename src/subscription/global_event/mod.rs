use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

pub mod listener;
pub mod player;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Event {
    pub time: SystemTime,
    pub kind: EventKind,
}

impl Event {
    pub fn new(time: SystemTime, kind: EventKind) -> Self {
        Self { time, kind }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum EventKind {
    Input(rdev::EventType),
    FocusChange { window_title: String },
    Delay(Duration),
    YieldFocus,
}
