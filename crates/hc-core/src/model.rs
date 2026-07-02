use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

pub type DeviceId = String;
pub type TagId = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    F32(f32),
    Bool(bool),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::U16(v) => write!(f, "{v}"),
            Value::I16(v) => write!(f, "{v}"),
            Value::U32(v) => write!(f, "{v}"),
            Value::I32(v) => write!(f, "{v}"),
            Value::F32(v) => write!(f, "{v}"),
            Value::Bool(v) => write!(f, "{v}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    Good = 0,
    Bad = 1,
    Stale = 2,
}

impl fmt::Display for Quality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Quality::Good => write!(f, "Good"),
            Quality::Bad => write!(f, "Bad"),
            Quality::Stale => write!(f, "Stale"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub tag_id: TagId,
    pub ts: DateTime<Utc>,
    pub value: Value,
    pub quality: Quality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagUpdate {
    pub tag_id: TagId,
    pub ts: DateTime<Utc>,
    pub value: Value,
    pub unit: String,
    pub quality: Quality,
}

impl From<&Sample> for TagUpdate {
    fn from(s: &Sample) -> Self {
        TagUpdate {
            tag_id: s.tag_id.clone(),
            ts: s.ts,
            value: s.value.clone(),
            unit: String::new(),
            quality: s.quality,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    U16,
    I16,
    U32,
    I32,
    F32,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ByteOrder {
    Abcd,
    Badc,
    Cdab,
    Dcba,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    pub enabled: bool,
    pub protocol: String,
    pub transport: TransportSpec,
    pub protocol_params: serde_json::Value,
    pub poll_interval_ms: u64,
    pub timeout_ms: u64,
}

impl Default for Device {
    fn default() -> Self {
        Device {
            id: "default".into(),
            name: "Default".into(),
            enabled: true,
            protocol: "modbus".into(),
            transport: TransportSpec::Tcp {
                host: "127.0.0.1".into(),
                port: 502,
            },
            protocol_params: serde_json::json!({}),
            poll_interval_ms: 1000,
            timeout_ms: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TransportSpec {
    Tcp {
        host: String,
        port: u16,
    },
    Rtu {
        port: String,
        baud: u32,
        data_bits: u8,
        parity: String,
        stop_bits: u8,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: TagId,
    pub device_id: DeviceId,
    pub name: String,
    pub enabled: bool,
    pub data_type: DataType,
    pub byte_order: ByteOrder,
    pub scale: f64,
    pub offset: f64,
    pub unit: String,
    pub writable: bool,
    pub protocol_params: serde_json::Value,
}

impl Default for Tag {
    fn default() -> Self {
        Tag {
            id: "default/t".into(),
            device_id: "default".into(),
            name: "Default".into(),
            enabled: true,
            data_type: DataType::U16,
            byte_order: ByteOrder::Abcd,
            scale: 1.0,
            offset: 0.0,
            unit: String::new(),
            writable: false,
            protocol_params: serde_json::json!({}),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceState {
    pub device_id: DeviceId,
    pub online: bool,
    pub error_count: u64,
    pub last_error: Option<String>,
    pub last_poll_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub running: bool,
    pub devices: Vec<DeviceState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub version: u32,
    #[serde(skip)]
    pub path: Option<PathBuf>,
    pub runtime: ProjectRuntime,
    pub storage: ProjectStorage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRuntime {
    pub default_poll_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStorage {
    pub history_sampling_ms: u64,
    pub trend_max_points: u32,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            name: "新工程".into(),
            version: 1,
            path: None,
            runtime: ProjectRuntime {
                default_poll_interval_ms: 1000,
            },
            storage: ProjectStorage {
                history_sampling_ms: 1000,
                trend_max_points: 2000,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub ts: DateTime<Utc>,
    pub level: String,
    pub message: String,
}
