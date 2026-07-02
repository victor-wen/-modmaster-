use crate::error::SourceError;
use crate::model::*;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct PollRequest {
    pub tags: Vec<Tag>,
}

#[derive(Debug, Clone)]
pub struct PollOutcome {
    pub samples: Vec<Sample>,
    pub device_id: DeviceId,
}

#[derive(Debug, Clone)]
pub struct WriteRequest {
    pub tag: Tag,
    pub value: Value,
}

pub type WriteOutcome = Result<(), SourceError>;

#[derive(Debug, Clone)]
pub enum SourceHealth {
    Connected,
    Disconnected { reason: String },
}

#[async_trait]
pub trait Source: Send {
    async fn open(spec: &Device) -> Result<Self, SourceError>
    where
        Self: Sized;
    async fn poll(&mut self, req: &PollRequest) -> Result<PollOutcome, SourceError>;
    async fn write(&mut self, req: &WriteRequest) -> WriteOutcome;
    async fn health(&mut self) -> SourceHealth;
}
