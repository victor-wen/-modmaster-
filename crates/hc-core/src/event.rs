use crate::model::*;

#[derive(Debug, Clone)]
pub enum Event {
    PollSucceeded(DeviceId, Vec<Sample>),
    PollFailed(DeviceId, String),
    ConnectionStateChanged(DeviceId, bool),
    Log(LogEntry),
    DeviceStatus(DeviceState),
}
