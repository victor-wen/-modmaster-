use thiserror::Error;

#[derive(Error, Debug)]
pub enum SourceError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Communication error: {0}")]
    Comm(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Write failed: {0}")]
    Write(String),
    #[error("Not connected")]
    NotConnected,
}

#[derive(Error, Debug)]
pub enum IpcError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Runtime error: {0}")]
    Runtime(String),
    #[error("Internal error: {0}")]
    Internal(String),
}
