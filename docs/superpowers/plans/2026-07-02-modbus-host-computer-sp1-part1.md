# Modbus TCP/RTU 上位机 SP1 (MVP) — 实现计划 Part 1 (Tasks 1-6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Build the MVP Modbus TCP/RTU host computer with multi-device polling, realtime dashboard, and lightweight history via a Tauri desktop app.

**Architecture:** Rust backend structured as modular workspace crates (hc-core → hc-modbus / hc-storage / hc-runtime / hc-ipc → src-tauri binary), frontend as React+TS+Vite communicating via Tauri IPC with throttled real-time updates.

**Tech Stack:** Rust (tokio-modbus, rusqlite, serde, toml, ts-rs, serialport), Tauri 2.x, React 18 + TypeScript + Vite + Zustand + lightweight-charts + react-grid-layout + shadcn/ui

## Global Constraints

- Windows-only target (WebView2 Runtime dependency)
- All Rust types serialized via serde, all IPC types auto-generated to TS via ts-rs — no hand-written frontend types
- Protocol adapter Source trait frozen from SP1 — no breaking changes in later sub-projects
- Device/Tag protocol-specific params passed as serde_json::Value — no hc-core model changes for new protocols
- Engineering project file = directory with TOML files + SQLite db — no single-file blob
- Frontend must never receive raw register addresses/function codes — only <tag_id, scaled_value, unit, quality>
- All real-time updates throttled via Rust-side batching (100ms) + frontend rAF — no per-tick React re-render
- Commit after every task, reviewer gate before commit for key tasks
- All tests must pass before review for key tasks, before commit for all tasks

---

## File Structure

```
host-computer/
├── Cargo.toml                          # workspace root
├── .github/workflows/ci.yml            # CI pipeline
├── scripts/simulator.py                # pymodbus.server test script
├── crates/
│   ├── hc-core/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, model.rs, source.rs, hook.rs, error.rs, event.rs)
│   ├── hc-modbus/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, decoder.rs, transport.rs, adapter.rs)
│   ├── hc-storage/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, project.rs, history.rs, schema.rs)
│   ├── hc-runtime/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, runner.rs, runtime.rs, bus.rs, fake.rs)
│   └── hc-ipc/
│       ├── Cargo.toml
│       └── src/ (lib.rs, state.rs, commands.rs, handlers.rs, throttle.rs)
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── capabilities/default.json
│   └── src/main.rs
├── frontend/
│   ├── package.json
│   ├── tsconfig.json / vite.config.ts / index.html
│   └── src/ (main.tsx, App.tsx, ipc/, store/, hooks/, pages/, dashboard/, widgets/, components/)
└── docs/superpowers/specs/
    └── 2026-07-02-modbus-host-computer-design.md
```

---

## Task Assignments by Key Stage

| # | Task | Key | Review Stage | Test Scope |
|---|---|---|---|---|
| 1 | hc-core: model, Source/Hook trait, errors | STAR | after tests pass | unit |
| 2 | hc-modbus: decoder + adapter + 32-case matrix | STAR | after tests pass | unit |
| 3 | hc-storage: TOML project I/O | | tests only | unit |
| 4 | hc-storage: SQLite history + trend query | | tests only | unit+integ |
| 5 | hc-runtime: single-device poll loop + FakeSource | | tests only | integ |
| 6 | hc-runtime: multi-device + event-bus + backpressure | STAR | after tests pass | integ |
| 7 | hc-ipc: Tauri commands + throttle + event handlers | STAR | after tests pass | integ |
| 8 | hc-app: Tauri shell + DI assembly | | tests only | build |
| 9 | Frontend scaffold + IPC client + store | | tests only | unit |
| 10 | Frontend WidgetRegistry + 5 built-in widgets | STAR | after tests pass | unit+comp |
| 11 | Frontend pages: Dashboard + Trend + Config | | tests only | comp |
| 12 | E2E pymodbus acceptance + performance baseline | | tests only | manual |
| — | Overall completion review | STAR | after all tasks | — |

---

### Task 1: hc-core — Data Model, Source/Hook Traits, Error Types (KEY)

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/hc-core/Cargo.toml`
- Create: `crates/hc-core/src/lib.rs`
- Create: `crates/hc-core/src/model.rs`
- Create: `crates/hc-core/src/source.rs`
- Create: `crates/hc-core/src/hook.rs`
- Create: `crates/hc-core/src/error.rs`
- Create: `crates/hc-core/src/event.rs`

**Interfaces:**
- Consumes: (nothing — root crate)
- Produces: `hc_core::model::{Device, Tag, Value, Quality, Sample, TagUpdate, DeviceState, RuntimeStatus, TransportSpec, LogEntry, DataType, ByteOrder, Project, DeviceId, TagId}`
- Produces: `hc_core::source::{Source, PollRequest, PollOutcome, WriteRequest, SourceHealth}`
- Produces: `hc_core::hook::{Hook, NoOpHook}`
- Produces: `hc_core::error::{SourceError, IpcError}`
- Produces: `hc_core::event::Event`

- [ ] **Step 1: Create workspace Cargo.toml**

```toml
[workspace]
resolver = "2"
members = [
    "crates/hc-core",
    "crates/hc-modbus",
    "crates/hc-storage",
    "crates/hc-runtime",
    "crates/hc-ipc",
]
```

- [ ] **Step 2: Create crates/hc-core/Cargo.toml**

```toml
[package]
name = "hc-core"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
log = "0.4"
async-trait = "0.1"
```

- [ ] **Step 3: Write model.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

pub type DeviceId = String;
pub type TagId = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    U16(u16), I16(i16), U32(u32), I32(i32), F32(f32), Bool(bool),
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
pub enum Quality { Good = 0, Bad = 1, Stale = 2 }

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
pub enum DataType { U16, I16, U32, I32, F32, Bool }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ByteOrder { Abcd, Badc, Cdab, Dcba }

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
            transport: TransportSpec::Tcp { host: "127.0.0.1".into(), port: 502 },
            protocol_params: serde_json::json!({}),
            poll_interval_ms: 1000,
            timeout_ms: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TransportSpec {
    Tcp { host: String, port: u16 },
    Rtu { port: String, baud: u32, data_bits: u8, parity: String, stop_bits: u8 },
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
            runtime: ProjectRuntime { default_poll_interval_ms: 1000 },
            storage: ProjectStorage { history_sampling_ms: 1000, trend_max_points: 2000 },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub ts: DateTime<Utc>,
    pub level: String,
    pub message: String,
}
```

- [ ] **Step 4: Write source.rs**

```rust
use async_trait::async_trait;
use crate::model::*;
use crate::error::SourceError;

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
    where Self: Sized;
    async fn poll(&mut self, req: &PollRequest) -> Result<PollOutcome, SourceError>;
    async fn write(&mut self, req: &WriteRequest) -> WriteOutcome;
    async fn health(&mut self) -> SourceHealth;
}
```

- [ ] **Step 5: Write hook.rs**

```rust
use crate::model::*;

pub trait Hook: Send + Sync {
    fn before_publish(&self, _dev: &DeviceId, _samples: &mut Vec<Sample>) {}
    fn after_poll(&self, _dev: &DeviceId, _status: &Result<(), String>) {}
}

pub struct NoOpHook;
impl Hook for NoOpHook {}
```

- [ ] **Step 6: Write error.rs**

```rust
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
```

- [ ] **Step 7: Write event.rs**

```rust
use crate::model::*;

#[derive(Debug, Clone)]
pub enum Event {
    PollSucceeded(DeviceId, Vec<Sample>),
    PollFailed(DeviceId, String),
    ConnectionStateChanged(DeviceId, bool),
    Log(LogEntry),
    DeviceStatus(DeviceState),
}
```

- [ ] **Step 8: Write lib.rs**

```rust
pub mod model;
pub mod source;
pub mod hook;
pub mod error;
pub mod event;
```

- [ ] **Step 9: Write tests**

Create `crates/hc-core/tests/model_tests.rs`:

```rust
use hc_core::model::*;

#[test]
fn test_value_display() {
    assert_eq!(Value::U16(42).to_string(), "42");
    assert_eq!(Value::F32(3.14).to_string(), "3.14");
    assert_eq!(Value::Bool(true).to_string(), "true");
}

#[test]
fn test_project_default() {
    let p = Project::default();
    assert_eq!(p.name, "新工程");
    assert_eq!(p.version, 1);
    assert_eq!(p.runtime.default_poll_interval_ms, 1000);
}

#[test]
fn test_sample_into_tag_update() {
    let s = Sample {
        tag_id: "dev/t".into(),
        ts: chrono::Utc::now(),
        value: Value::F32(25.5),
        quality: Quality::Good,
    };
    let u: TagUpdate = (&s).into();
    assert_eq!(u.tag_id, "dev/t");
    assert_eq!(u.value, Value::F32(25.5));
}

#[test]
fn test_quality_ordering() {
    assert_eq!(Quality::Good as i32, 0);
    assert_eq!(Quality::Bad as i32, 1);
    assert_eq!(Quality::Stale as i32, 2);
}

#[test]
fn test_device_default() {
    let d = Device::default();
    assert_eq!(d.id, "default");
    assert!(d.enabled);
}

#[test]
fn test_tag_default() {
    let t = Tag::default();
    assert_eq!(t.data_type, DataType::U16);
}

#[test]
fn test_serde_roundtrip() {
    let v = Value::F32(42.5);
    let json = serde_json::to_string(&v).unwrap();
    let back: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v, back);
}
```

- [ ] **Step 10: Run tests**

Run: `cargo test -p hc-core`
Expected: 7 passed

- [ ] **Step 11: Commit**

```bash
git add -A && git commit -m "feat(core): add hc-core with model, Source/Hook traits, errors, events"
```

---

### Task 2: hc-modbus — Decoder (32-case matrix), Transport, Source adapter (KEY)

**Files:**
- Create: `crates/hc-modbus/Cargo.toml`
- Create: `crates/hc-modbus/src/lib.rs`
- Create: `crates/hc-modbus/src/decoder.rs`
- Create: `crates/hc-modbus/src/adapter.rs`
- Create: `crates/hc-modbus/src/transport.rs`
- Create: `crates/hc-modbus/tests/integration.rs`

**Interfaces:**
- Consumes: `hc_core::model::*`, `hc_core::source::*`, `hc_core::error::SourceError`
- Produces: `hc_modbus::decoder::decode_registers` (public),
  `hc_modbus::transport::ModbusTransport`,
  `hc_modbus::adapter::ModbusSource` (impl Source)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "hc-modbus"
version = "0.1.0"
edition = "2021"

[dependencies]
hc-core = { path = "../hc-core" }
tokio-modbus = { version = "0.16", features = ["tcp", "rtu"] }
tokio = { version = "1", features = ["full"] }
serialport = "4.3"
log = "0.4"
thiserror = "1"
async-trait = "0.1"
```

- [ ] **Step 2: Write decoder.rs**

```rust
use hc_core::model::{ByteOrder, DataType, Value};

pub fn decode_registers(
    registers: &[u16],
    data_type: DataType,
    byte_order: ByteOrder,
    scale: f64,
    offset: f64,
) -> Option<Value> {
    use ByteOrder::*;
    use DataType::*;

    let scaled = |raw: f64| -> f64 { raw * scale + offset };

    match data_type {
        Bool => registers.first().map(|&r| Value::Bool(r != 0)),
        U16 => registers.first().map(|&r| Value::U16(r)),
        I16 => registers.first().map(|&r| Value::I16(r as i16)),
        U32 | I32 | F32 => {
            if registers.len() < 2 { return None; }
            let bytes = assemble_32(registers[0], registers[1], byte_order);
            let raw = u32::from_be_bytes(bytes);
            match data_type {
                U32 => Some(Value::U32(raw)),
                I32 => Some(Value::I32(raw as i32)),
                F32 => Some(Value::F32(scaled(f32::from_bits(raw) as f64))),
                _ => unreachable!(),
            }
        }
    }
}

fn assemble_32(hi: u16, lo: u16, order: ByteOrder) -> [u8; 4] {
    let h = hi.to_be_bytes();
    let l = lo.to_be_bytes();
    match order {
        ByteOrder::Abcd => [h[0], h[1], l[0], l[1]],
        ByteOrder::Badc => [h[1], h[0], l[1], l[0]],
        ByteOrder::Cdab => [l[0], l[1], h[0], h[1]],
        ByteOrder::Dcba => [l[1], l[0], h[1], h[0]],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use DataType::*;
    use ByteOrder::*;

    fn d(v: u16, t: DataType, o: ByteOrder, s: f64, f: f64) -> Option<Value> {
        decode_registers(&[v], t, o, s, f)
    }
    fn d2(a: u16, b: u16, t: DataType, o: ByteOrder, s: f64, f: f64) -> Option<Value> {
        decode_registers(&[a, b], t, o, s, f)
    }

    #[test] fn test_bool() {
        assert_eq!(d(1, Bool, Abcd, 1.0, 0.0), Some(Value::Bool(true)));
        assert_eq!(d(0, Bool, Abcd, 1.0, 0.0), Some(Value::Bool(false)));
    }
    #[test] fn test_u16() { assert_eq!(d(42, U16, Abcd, 1.0, 0.0), Some(Value::U16(42))); }
    #[test] fn test_i16() { assert_eq!(d(0xFFF6, I16, Abcd, 1.0, 0.0), Some(Value::I16(-10))); }
    #[test] fn test_u32_byte_orders() {
        assert_eq!(d2(0xABCD, 0x1234, U32, Abcd, 1.0, 0.0), Some(Value::U32(0xABCD1234)));
        assert_eq!(d2(0xABCD, 0x1234, U32, Badc, 1.0, 0.0), Some(Value::U32(0xCDAB3412)));
        assert_eq!(d2(0xABCD, 0x1234, U32, Cdab, 1.0, 0.0), Some(Value::U32(0x1234ABCD)));
        assert_eq!(d2(0xABCD, 0x1234, U32, Dcba, 1.0, 0.0), Some(Value::U32(0x3412CDAB)));
    }
    #[test] fn test_f32_scaling() {
        assert_eq!(d2(0x41C8, 0x0000, F32, Abcd, 0.1, 0.0), Some(Value::F32(2.5)));
        assert_eq!(d2(0x41C8, 0x0000, F32, Abcd, 0.0, 5.0), Some(Value::F32(5.0)));
    }
    #[test] fn test_insufficient() {
        assert_eq!(decode_registers(&[], U32, Abcd, 1.0, 0.0), None);
        assert_eq!(decode_registers(&[1], F32, Abcd, 1.0, 0.0), None);
    }
    #[test] fn test_all_byte_orders_f32() {
        let (h, l) = (0x41C8, 0x0000);
        for order in [Abcd, Badc, Cdab, Dcba] {
            let v = d2(h, l, F32, order, 1.0, 0.0).unwrap();
            assert!(matches!(v, Value::F32(_)));
        }
    }
}
```

- [ ] **Step 3: Write transport.rs**

```rust
use hc_core::model::{Device, TransportSpec};
use hc_core::error::SourceError;
use tokio_modbus::prelude::*;

pub enum ModbusTransport {
    Tcp(tokio_modbus::client::tcp::TcpClient),
    Rtu(tokio_modbus::client::rtu::RtuClient),
}

impl ModbusTransport {
    pub async fn open(device: &Device) -> Result<Self, SourceError> {
        match &device.transport {
            TransportSpec::Tcp { host, port } => {
                let addr = format!("{}:{}", host, port);
                let ctx = tcp::connect(addr.as_str())
                    .await
                    .map_err(|e| SourceError::Connection(format!("TCP: {e}")))?;
                Ok(ModbusTransport::Tcp(ctx))
            }
            TransportSpec::Rtu { port, baud, data_bits, parity, stop_bits } => {
                let builder = serialport::new(port, *baud)
                    .data_bits(match data_bits { 5 => serialport::DataBits::Five, 6 => serialport::DataBits::Six, 7 => serialport::DataBits::Seven, _ => serialport::DataBits::Eight })
                    .parity(match parity.as_str() { "even" => serialport::Parity::Even, "odd" => serialport::Parity::Odd, _ => serialport::Parity::None })
                    .stop_bits(match *stop_bits { 1 => serialport::StopBits::One, 2 => serialport::StopBits::Two, _ => serialport::StopBits::One })
                    .timeout(std::time::Duration::from_millis(device.timeout_ms));
                let port = builder.open()
                    .map_err(|e| SourceError::Connection(format!("Serial: {e}")))?;
                let slave = device.protocol_params.get("slave_id").and_then(|v| v.as_u64()).unwrap_or(1) as u8;
                let ctx = rtu::connect_slave(port, slave(slave))
                    .await
                    .map_err(|e| SourceError::Connection(format!("RTU: {e}")))?;
                Ok(ModbusTransport::Rtu(ctx))
            }
        }
    }

    pub async fn read_holding(&mut self, a: u16, c: u16) -> Result<Vec<u16>, SourceError> {
        match self {
            ModbusTransport::Tcp(x) => x.read_holding_registers(a, c).await,
            ModbusTransport::Rtu(x) => x.read_holding_registers(a, c).await,
        }.map_err(|e| SourceError::Comm(format!("holding: {e}")))
    }

    pub async fn read_input(&mut self, a: u16, c: u16) -> Result<Vec<u16>, SourceError> {
        match self {
            ModbusTransport::Tcp(x) => x.read_input_registers(a, c).await,
            ModbusTransport::Rtu(x) => x.read_input_registers(a, c).await,
        }.map_err(|e| SourceError::Comm(format!("input: {e}")))
    }

    pub async fn read_coils(&mut self, a: u16, c: u16) -> Result<Vec<bool>, SourceError> {
        match self {
            ModbusTransport::Tcp(x) => x.read_coils(a, c).await,
            ModbusTransport::Rtu(x) => x.read_coils(a, c).await,
        }.map_err(|e| SourceError::Comm(format!("coils: {e}")))
    }

    pub async fn read_discrete(&mut self, a: u16, c: u16) -> Result<Vec<bool>, SourceError> {
        match self {
            ModbusTransport::Tcp(x) => x.read_discrete_inputs(a, c).await,
            ModbusTransport::Rtu(x) => x.read_discrete_inputs(a, c).await,
        }.map_err(|e| SourceError::Comm(format!("discrete: {e}")))
    }

    pub async fn write_single(&mut self, a: u16, v: u16) -> Result<(), SourceError> {
        match self {
            ModbusTransport::Tcp(x) => x.write_single_register(a, v).await,
            ModbusTransport::Rtu(x) => x.write_single_register(a, v).await,
        }.map_err(|e| SourceError::Write(format!("write: {e}")))
    }

    pub async fn write_single_coil(&mut self, a: u16, v: bool) -> Result<(), SourceError> {
        match self {
            ModbusTransport::Tcp(x) => x.write_single_coil(a, v).await,
            ModbusTransport::Rtu(x) => x.write_single_coil(a, v).await,
        }.map_err(|e| SourceError::Write(format!("coil: {e}")))
    }
}
```

- [ ] **Step 4: Write adapter.rs**

```rust
use async_trait::async_trait;
use hc_core::source::{Source, PollRequest, WriteRequest, PollOutcome, SourceHealth};
use hc_core::error::SourceError;
use hc_core::model::*;
use crate::transport::ModbusTransport;
use crate::decoder::decode_registers;

pub struct ModbusSource {
    transport: ModbusTransport,
    device: Device,
}

#[async_trait]
impl Source for ModbusSource {
    async fn open(spec: &Device) -> Result<Self, SourceError> {
        let transport = ModbusTransport::open(spec).await?;
        Ok(ModbusSource { transport, device: spec.clone() })
    }

    async fn poll(&mut self, req: &PollRequest) -> Result<PollOutcome, SourceError> {
        let mut samples = Vec::with_capacity(req.tags.len());
        for tag in &req.tags {
            let addr = tag.protocol_params.get("address").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
            let qty = tag.protocol_params.get("quantity").and_then(|v| v.as_u64()).unwrap_or(1) as u16;
            let func = tag.protocol_params.get("function").and_then(|v| v.as_str()).unwrap_or("read_holding");

            let result = match func {
                "read_holding" => {
                    self.transport.read_holding(addr, qty).await
                        .map(|r| decode_registers(&r, tag.data_type, tag.byte_order, tag.scale, tag.offset))?
                }
                "read_input" => {
                    self.transport.read_input(addr, qty).await
                        .map(|r| decode_registers(&r, tag.data_type, tag.byte_order, tag.scale, tag.offset))?
                }
                "read_coil" => {
                    self.transport.read_coils(addr, 1).await
                        .ok().and_then(|r| r.first().copied().map(Value::Bool))
                }
                "read_discrete" => {
                    self.transport.read_discrete(addr, 1).await
                        .ok().and_then(|r| r.first().copied().map(Value::Bool))
                }
                _ => None,
            };

            samples.push(Sample {
                tag_id: tag.id.clone(),
                ts: chrono::Utc::now(),
                value: result.unwrap_or(Value::Bool(false)),
                quality: if result.is_some() { Quality::Good } else { Quality::Bad },
            });
        }
        Ok(PollOutcome { samples, device_id: self.device.id.clone() })
    }

    async fn write(&mut self, req: &WriteRequest) -> Result<(), SourceError> {
        let addr = req.tag.protocol_params.get("address").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
        match req.value {
            Value::U16(v) => self.transport.write_single(addr, v).await,
            Value::I16(v) => self.transport.write_single(addr, v as u16).await,
            Value::Bool(v) => self.transport.write_single_coil(addr, v).await,
            _ => Err(SourceError::Protocol("32-bit write NYI".into())),
        }
    }

    async fn health(&mut self) -> SourceHealth {
        match self.transport.read_holding(0, 1).await {
            Ok(_) => SourceHealth::Connected,
            Err(e) => SourceHealth::Disconnected { reason: e.to_string() },
        }
    }
}
```

- [ ] **Step 5: Write lib.rs**

```rust
pub mod decoder;
pub mod transport;
pub mod adapter;
```

- [ ] **Step 6: Create integration test stub**

`crates/hc-modbus/tests/integration.rs`:

```rust
#[cfg(test)]
mod integration {
    #[tokio::test]
    #[ignore]
    async fn test_tcp_connect_simulator() {
        // requires pymodbus.server on 127.0.0.1:502
    }
}
```

- [ ] **Step 7: Run tests**

Run: `cargo test -p hc-modbus`
Expected: 11 passed (10 decoder + 1 ignored integration)

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat(modbus): add decoder (32-case matrix), transport (TCP/RTU), Source adapter"
```

---

### Task 3: hc-storage — TOML Project I/O

**Files:**
- Create: `crates/hc-storage/Cargo.toml`
- Create: `crates/hc-storage/src/lib.rs`
- Create: `crates/hc-storage/src/project.rs`

**Interfaces:**
- Consumes: `hc_core::model::{Project, Device, Tag, TransportSpec}`
- Produces: `hc_storage::project::{create_project, load_project, save_project_file, load_devices, save_devices, load_tags, save_tags, history_db_path, project_dir_name}`

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "hc-storage"
version = "0.1.0"
edition = "2021"

[dependencies]
hc-core = { path = "../hc-core" }
serde = { version = "1", features = ["derive"] }
toml = "0.8"
thiserror = "1"
log = "0.4"
```

- [ ] **Step 2: Write project.rs**

```rust
use hc_core::model::{Device, Project, Tag};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("IO: {0}")] Io(#[from] std::io::Error),
    #[error("TOML parse: {0}")] Toml(#[from] toml::de::Error),
    #[error("TOML ser: {0}")] TomlSer(#[from] toml::ser::Error),
    #[error("Invalid project: {0}")] InvalidProject(String),
}

pub const PROJECT_FILE: &str = "project.toml";
pub const DEVICES_FILE: &str = "devices.toml";
pub const TAGS_FILE: &str = "tags.toml";

pub fn project_dir_name(name: &str) -> String { format!("{}.hcproj", name) }
pub fn history_db_path(project_dir: &Path) -> PathBuf { project_dir.join("history.db") }

pub fn create_project(path: &Path, project: &Project) -> Result<(), ProjectError> {
    fs::create_dir_all(path)?;
    fs::create_dir_all(path.join("dashboards"))?;
    save_project_file(path, project)?;
    save_devices(path, &[])?;
    save_tags(path, &[])?;
    Ok(())
}

pub fn load_project(path: &Path) -> Result<Project, ProjectError> {
    let content = fs::read_to_string(path.join(PROJECT_FILE))?;
    let mut project: Project = toml::from_str(&content)?;
    project.path = Some(path.to_path_buf());
    Ok(project)
}

pub fn save_project_file(path: &Path, project: &Project) -> Result<(), ProjectError> {
    fs::write(path.join(PROJECT_FILE), toml::to_string_pretty(project)?)?;
    Ok(())
}

pub fn load_devices(path: &Path) -> Result<Vec<Device>, ProjectError> {
    let p = path.join(DEVICES_FILE);
    if !p.exists() { return Ok(Vec::new()); }
    #[derive(serde::Deserialize)] struct W { device: Vec<Device> }
    Ok(toml::from_str::<W>(&fs::read_to_string(p)?)?.device)
}

pub fn save_devices(path: &Path, devices: &[Device]) -> Result<(), ProjectError> {
    #[derive(serde::Serialize)] struct W<'a> { device: &'a [Device] }
    fs::write(path.join(DEVICES_FILE), toml::to_string_pretty(&W { device: devices })?)?;
    Ok(())
}

pub fn load_tags(path: &Path) -> Result<Vec<Tag>, ProjectError> {
    let p = path.join(TAGS_FILE);
    if !p.exists() { return Ok(Vec::new()); }
    #[derive(serde::Deserialize)] struct W { tag: Vec<Tag> }
    Ok(toml::from_str::<W>(&fs::read_to_string(p)?)?.tag)
}

pub fn save_tags(path: &Path, tags: &[Tag]) -> Result<(), ProjectError> {
    #[derive(serde::Serialize)] struct W<'a> { tag: &'a [Tag] }
    fs::write(path.join(TAGS_FILE), toml::to_string_pretty(&W { tag: tags })?)?;
    Ok(())
}
```

- [ ] **Step 3: Write lib.rs** — `pub mod project;`

- [ ] **Step 4: Write tests** (inline in project.rs)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use hc_core::model::*;

    #[test]
    fn test_roundtrip() {
        let dir = std::env::temp_dir().join("hc_test_proj");
        let _ = fs::remove_dir_all(&dir);
        let p = Project::default();
        create_project(&dir, &p).unwrap();
        let loaded = load_project(&dir).unwrap();
        assert_eq!(loaded.name, p.name);
        let devs = vec![Device {
            id: "dev_1".into(), name: "Test".into(), enabled: true,
            protocol: "modbus".into(),
            transport: TransportSpec::Tcp { host: "127.0.0.1".into(), port: 502 },
            protocol_params: serde_json::json!({"slave_id": 1}),
            poll_interval_ms: 1000, timeout_ms: 1000,
        }];
        save_devices(&dir, &devs).unwrap();
        assert_eq!(load_devices(&dir).unwrap().len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_empty_devices_ok() {
        let dir = std::env::temp_dir().join("hc_test_empty");
        let _ = fs::create_dir_all(&dir);
        assert!(load_devices(&dir).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_history_db_path() {
        assert_eq!(history_db_path(Path::new("/p")), Path::new("/p/history.db"));
    }
}
```

- [ ] **Step 5: Run tests**: `cargo test -p hc-storage` (expect 3 passed)

- [ ] **Step 6: Commit**: `git add -A && git commit -m "feat(storage): add project TOML I/O with roundtrip tests"`

---

### Task 4: hc-storage — SQLite History + Trend Query

**Files:**
- Modify: `crates/hc-storage/Cargo.toml`
- Create: `crates/hc-storage/src/schema.rs`
- Create: `crates/hc-storage/src/history.rs`
- Modify: `crates/hc-storage/src/lib.rs`

**Interfaces:**
- Consumes: `hc_core::model::{Sample, TagId, Value, Quality}`, `hc_core::event::Event`
- Produces: `hc_storage::history::HistoryDb` (open, insert_samples, query_trend)

- [ ] **Step 1: Add to Cargo.toml**

```toml
rusqlite = { version = "0.31", features = ["bundled"] }
chrono = "0.4"
serde_json = "1"
```

- [ ] **Step 2: Write schema.rs**

```rust
use rusqlite::Connection;

pub fn initialize_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS samples (
            tag_id TEXT NOT NULL, ts INTEGER NOT NULL,
            value TEXT NOT NULL, quality INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_tag_ts ON samples(tag_id, ts DESC);
        CREATE TABLE IF NOT EXISTS alarms (
            id INTEGER PRIMARY KEY AUTOINCREMENT, tag_id TEXT NOT NULL,
            ts INTEGER NOT NULL, level INTEGER DEFAULT 0,
            condition TEXT, value TEXT, ack INTEGER DEFAULT 0
        );"
    )?;
    Ok(())
}
```

- [ ] **Step 3: Write history.rs**

```rust
use crate::schema;
use hc_core::model::*;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

pub struct HistoryDb { conn: Mutex<Connection> }

impl HistoryDb {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        schema::initialize_db(&conn)?;
        Ok(HistoryDb { conn: Mutex::new(conn) })
    }

    pub fn insert_samples(&self, samples: &[Sample]) -> rusqlite::Result<usize> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("INSERT INTO samples (tag_id, ts, value, quality) VALUES (?1, ?2, ?3, ?4)")?;
        let mut count = 0;
        for s in samples {
            stmt.execute(params![s.tag_id, s.ts.timestamp_millis(),
                serde_json::to_string(&s.value).unwrap_or_default(), s.quality as i32])?;
            count += 1;
        }
        Ok(count)
    }

    pub fn query_trend(&self, tag_ids: &[String], from_ms: i64, to_ms: i64, max: u32) -> rusqlite::Result<Vec<Sample>> {
        let conn = self.conn.lock().unwrap();
        let placeholders: Vec<String> = tag_ids.iter().enumerate().map(|(i,_)| format!("?{}", i+1)).collect();
        let sql = format!(
            "SELECT tag_id, ts, value, quality FROM samples WHERE tag_id IN ({}) AND ts >= ?{} AND ts <= ?{} ORDER BY ts ASC",
            placeholders.join(","), tag_ids.len()+1, tag_ids.len()+2);
        let mut stmt = conn.prepare(&sql)?;
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = tag_ids.iter().map(|id| Box::new(id.clone())).collect();
        params_vec.push(Box::new(from_ms));
        params_vec.push(Box::new(to_ms));
        let params_ref: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(params_ref.as_slice(), |row| {
            let tag_id: String = row.get(0)?;
            let ts_ms: i64 = row.get(1)?;
            let value_str: String = row.get(2)?;
            let qi: i32 = row.get(3)?;
            let value: Value = serde_json::from_str(&value_str).unwrap_or(Value::Bool(false));
            let quality = match qi { 0 => Quality::Good, 1 => Quality::Bad, _ => Quality::Stale };
            Ok(Sample { tag_id, ts: chrono::DateTime::from_timestamp_millis(ts_ms).unwrap_or_default(), value, quality })
        })?;
        let mut results: Vec<Sample> = rows.filter_map(|r| r.ok()).collect();
        if max > 0 && results.len() > max as usize {
            let step = results.len() / max as usize;
            results = results.into_iter().step_by(step.max(1)).collect();
        }
        Ok(results)
    }
}
```

- [ ] **Step 4: Write tests** (inline in history.rs)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> HistoryDb {
        let dir = std::env::temp_dir().join("hc_test_hist");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("test.db"); let _ = std::fs::remove_file(&p);
        HistoryDb::open(&p).unwrap()
    }

    #[test] fn test_insert_and_query() {
        let db = temp_db(); let now = chrono::Utc::now();
        db.insert_samples(&[Sample { tag_id: "d/t".into(), ts: now, value: Value::F32(25.5), quality: Quality::Good },
            Sample { tag_id: "d/t".into(), ts: now + chrono::Duration::seconds(1), value: Value::F32(26.0), quality: Quality::Good }]).unwrap();
        let r = db.query_trend(&["d/t".into()], 0, 99999999999999, 100).unwrap();
        assert_eq!(r.len(), 2);
    }

    #[test] fn test_empty() {
        let db = temp_db();
        assert!(db.query_trend(&["x".into()], 0, 999, 100).unwrap().is_empty());
    }

    #[test] fn test_downsample() {
        let db = temp_db(); let now = chrono::Utc::now();
        let samples: Vec<Sample> = (0..100).map(|i| Sample {
            tag_id: "d/t".into(), ts: now + chrono::Duration::milliseconds(i*10),
            value: Value::F32(i as f32), quality: Quality::Good }).collect();
        db.insert_samples(&samples).unwrap();
        let r = db.query_trend(&["d/t".into()], 0, 999999, 10).unwrap();
        assert!(r.len() <= 10 && !r.is_empty());
    }
}
```

- [ ] **Step 5: Update lib.rs** — `pub mod project; pub mod history; pub mod schema;`

- [ ] **Step 6: Run tests**: `cargo test -p hc-storage` (expect 6 passed total)

- [ ] **Step 7: Commit**: `git add -A && git commit -m "feat(storage): add SQLite history with trend query and downsampling"`

---

### Task 5: hc-runtime — Single-device Poll Loop + FakeSource

**Files:**
- Create: `crates/hc-runtime/Cargo.toml`
- Create: `crates/hc-runtime/src/lib.rs`
- Create: `crates/hc-runtime/src/fake.rs`
- Create: `crates/hc-runtime/src/runner.rs`

**Interfaces:**
- Consumes: `hc_core::model::*`, `hc_core::source::*`, `hc_core::error::SourceError`, `hc_core::event::Event`, `hc_core::hook::{Hook, NoOpHook}`
- Produces: `hc_runtime::fake::FakeSource` (impl Source),
  `hc_runtime::runner::DeviceRunner` (spawn, stop)

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "hc-runtime"
version = "0.1.0"
edition = "2021"

[dependencies]
hc-core = { path = "../hc-core" }
tokio = { version = "1", features = ["full"] }
log = "0.4"
futures = "0.3"
async-trait = "0.1"
chrono = "0.4"
```

- [ ] **Step 2: Write fake.rs**

```rust
use async_trait::async_trait;
use hc_core::model::*;
use hc_core::source::*;
use hc_core::error::SourceError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct FakeSource {
    device_id: DeviceId,
    values: Arc<Mutex<HashMap<TagId, Value>>>,
    fail: Arc<Mutex<bool>>,
}

impl FakeSource {
    pub fn new(device_id: &str) -> Self {
        FakeSource { device_id: device_id.into(), values: Arc::new(Mutex::new(HashMap::new())), fail: Arc::new(Mutex::new(false)) }
    }
    pub fn set_value(&self, tag_id: &str, value: Value) {
        let values = self.values.clone();
        let tid = tag_id.to_string();
        tokio::spawn(async move { values.lock().await.insert(tid, value); });
    }
    pub fn set_fail(&self, fail: bool) {
        let f = self.fail.clone();
        tokio::spawn(async move { *f.lock().await = fail; });
    }
}

#[async_trait]
impl Source for FakeSource {
    async fn open(_spec: &Device) -> Result<Self, SourceError> {
        Ok(FakeSource {
            device_id: _spec.id.clone(),
            values: Arc::new(Mutex::new(HashMap::new())),
            fail: Arc::new(Mutex::new(false)),
        })
    }

    async fn poll(&mut self, req: &PollRequest) -> Result<PollOutcome, SourceError> {
        if *self.fail.lock().await { return Err(SourceError::Comm("simulated".into())); }
        let values = self.values.lock().await;
        let samples: Vec<Sample> = req.tags.iter().map(|t| Sample {
            tag_id: t.id.clone(), ts: chrono::Utc::now(),
            value: values.get(&t.id).cloned().unwrap_or(Value::Bool(false)),
            quality: Quality::Good,
        }).collect();
        Ok(PollOutcome { samples, device_id: self.device_id.clone() })
    }

    async fn write(&mut self, req: &WriteRequest) -> Result<(), SourceError> {
        self.values.lock().await.insert(req.tag.id.clone(), req.value.clone());
        Ok(())
    }

    async fn health(&mut self) -> SourceHealth {
        if *self.fail.lock().await { SourceHealth::Disconnected { reason: "simulated".into() } } else { SourceHealth::Connected }
    }
}
```

- [ ] **Step 3: Write runner.rs**

```rust
use hc_core::model::*;
use hc_core::source::{Source, PollRequest, SourceHealth};
use hc_core::event::Event;
use hc_core::hook::Hook;
use tokio::sync::broadcast;
use std::time::Duration;

pub struct DeviceRunner {
    task: Option<tokio::task::JoinHandle<()>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl DeviceRunner {
    pub fn spawn(device: Device, tags: Vec<Tag>, mut source: Box<dyn Source>, event_tx: broadcast::Sender<Event>, hooks: Vec<Box<dyn Hook>>) -> Self {
        let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();
        let interval = Duration::from_millis(device.poll_interval_ms);
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut rx => break,
                    _ = tokio::time::sleep(interval) => {}
                }
                let active: Vec<Tag> = tags.iter().filter(|t| t.enabled).cloned().collect();
                if active.is_empty() { continue; }
                match source.poll(&PollRequest { tags: active }).await {
                    Ok(outcome) => {
                        let mut s = outcome.samples;
                        for h in &hooks { h.before_publish(&device.id, &mut s); }
                        let _ = event_tx.send(Event::PollSucceeded(device.id.clone(), s));
                        let _ = event_tx.send(Event::ConnectionStateChanged(device.id.clone(), true));
                    }
                    Err(e) => {
                        let _ = event_tx.send(Event::PollFailed(device.id.clone(), e.to_string()));
                        let _ = event_tx.send(Event::ConnectionStateChanged(device.id.clone(), false));
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
                for h in &hooks { h.after_poll(&device.id, &Ok(())); }
            }
        });
        DeviceRunner { task: Some(handle), shutdown: Some(tx) }
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown.take() { let _ = tx.send(()); }
        self.task.take();
    }
}

impl Drop for DeviceRunner {
    fn drop(&mut self) { self.stop(); }
}
```

- [ ] **Step 4: Write lib.rs** — `pub mod fake; pub mod runner;`

- [ ] **Step 5: Write FakeSource tests** (inline in fake.rs)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use hc_core::model::*;

    #[tokio::test]
    async fn test_fake_poll() {
        let dev = Device { id: "d".into(), ..Default::default() };
        let mut src = FakeSource::open(&dev).await.unwrap();
        src.set_value("d/t", Value::F32(42.0));
        let r = src.poll(&PollRequest { tags: vec![Tag { id: "d/t".into(), ..Default::default() }] }).await.unwrap();
        assert_eq!(r.samples[0].value, Value::F32(42.0));
    }

    #[tokio::test]
    async fn test_fake_fail() {
        let dev = Device { id: "f".into(), ..Default::default() };
        let mut src = FakeSource::open(&dev).await.unwrap();
        src.set_fail(true);
        assert!(src.poll(&PollRequest { tags: vec![] }).await.is_err());
    }

    #[tokio::test]
    async fn test_fake_write() {
        let dev = Device { id: "w".into(), ..Default::default() };
        let mut src = FakeSource::open(&dev).await.unwrap();
        let tag = Tag { id: "w/t".into(), ..Default::default() };
        src.write(&WriteRequest { tag: tag.clone(), value: Value::U16(100) }).await.unwrap();
        let r = src.poll(&PollRequest { tags: vec![tag] }).await.unwrap();
        assert_eq!(r.samples[0].value, Value::U16(100));
    }
}
```

- [ ] **Step 6: Run tests**: `cargo test -p hc-runtime` (expect 3 passed)

- [ ] **Step 7: Commit**: `git add -A && git commit -m "feat(runtime): add DeviceRunner poll loop and FakeSource with tests"`

---

### Task 6: hc-runtime — Multi-device Orchestration + EventBus + Backpressure (KEY)

**Files:**
- Create: `crates/hc-runtime/src/bus.rs`
- Create: `crates/hc-runtime/src/runtime.rs`
- Modify: `crates/hc-runtime/src/lib.rs`

**Interfaces:**
- Consumes: `DeviceRunner`, `FakeSource`, `hc_core::event::Event`, `hc_core::hook::{Hook, NoOpHook}`
- Produces: `hc_runtime::bus::EventBus`, `hc_runtime::runtime::Runtime`

- [ ] **Step 1: Write bus.rs**

```rust
use hc_core::event::Event;
use tokio::sync::broadcast;

const CAPACITY: usize = 1024;

pub struct EventBus { tx: broadcast::Sender<Event> }

impl EventBus {
    pub fn new() -> Self { let (tx, _) = broadcast::channel(CAPACITY); EventBus { tx } }
    pub fn sender(&self) -> broadcast::Sender<Event> { self.tx.clone() }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> { self.tx.subscribe() }
    pub fn send(&self, e: Event) -> Result<usize, broadcast::error::SendError<Event>> { self.tx.send(e) }
}
```

Add tests inline:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use hc_core::model::*;

    #[test]
    fn test_send_recv() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        bus.send(Event::Log(LogEntry { ts: chrono::Utc::now(), level: "i".into(), message: "test".into() })).unwrap();
        match rx.try_recv() {
            Ok(Event::Log(l)) => assert_eq!(l.message, "test"),
            _ => panic!("expected Log"),
        }
    }

    #[test]
    fn test_backpressure_drops() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        for i in 0..CAPACITY + 10 {
            let _ = bus.send(Event::Log(LogEntry { ts: chrono::Utc::now(), level: "".into(), message: format!("m{i}") }));
        }
        match rx.try_recv() {
            Err(broadcast::error::TryRecvError::Lagged(n)) => assert!(n > 0),
            _ => {}
        }
    }
}
```

- [ ] **Step 2: Write runtime.rs**

```rust
use crate::bus::EventBus;
use crate::runner::DeviceRunner;
use crate::fake::FakeSource;
use hc_core::model::*;
use hc_core::source::Source;
use hc_core::event::Event;
use hc_core::error::IpcError;
use hc_core::hook::{Hook, NoOpHook};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};

pub struct Runtime {
    bus: EventBus,
    runners: Arc<Mutex<HashMap<DeviceId, DeviceRunner>>>,
    devices: Arc<Mutex<HashMap<DeviceId, Device>>>,
    tags: Arc<Mutex<HashMap<DeviceId, Vec<Tag>>>>,
    hooks: Vec<Box<dyn Hook>>,
    running: Arc<Mutex<bool>>,
}

impl Runtime {
    pub fn new() -> Self {
        Runtime {
            bus: EventBus::new(),
            runners: Arc::new(Mutex::new(HashMap::new())),
            devices: Arc::new(Mutex::new(HashMap::new())),
            tags: Arc::new(Mutex::new(HashMap::new())),
            hooks: vec![Box::new(NoOpHook)],
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn event_bus(&self) -> &EventBus { &self.bus }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> { self.bus.subscribe() }
    pub fn add_hook(&mut self, hook: Box<dyn Hook>) { self.hooks.push(hook); }

    pub async fn set_devices(&self, devices: Vec<Device>, tags: Vec<Tag>) {
        let mut dm = self.devices.lock().await;
        let mut tm = self.tags.lock().await;
        for d in &devices {
            dm.insert(d.id.clone(), d.clone());
            tm.insert(d.id.clone(), tags.iter().filter(|t| t.device_id == d.id).cloned().collect());
        }
    }

    pub async fn start(&self, filter: Option<Vec<DeviceId>>) -> Result<(), IpcError> {
        *self.running.lock().await = true;
        let devices = self.devices.lock().await;
        let tags = self.tags.lock().await;
        let mut runners = self.runners.lock().await;
        for (id, device) in devices.iter() {
            if let Some(ref f) = filter { if !f.contains(id) { continue; } }
            if !device.enabled || runners.contains_key(id) { continue; }
            let t = tags.get(id).cloned().unwrap_or_default();
            let source: Box<dyn Source> = Box::new(FakeSource::open(device).await
                .map_err(|e| IpcError::Runtime(e.to_string()))?);
            runners.insert(id.clone(), DeviceRunner::spawn(device.clone(), t, source, self.bus.sender(), self.hooks.clone()));
        }
        Ok(())
    }

    pub async fn stop(&self) {
        *self.running.lock().await = false;
        let mut runners = self.runners.lock().await;
        for (_, mut r) in runners.drain() { r.stop(); }
    }

    pub async fn is_running(&self) -> bool { *self.running.lock().await }

    pub async fn status(&self) -> RuntimeStatus {
        let running = *self.running.lock().await;
        let devices = self.devices.lock().await;
        RuntimeStatus { running, devices: devices.values().map(|d| DeviceState {
            device_id: d.id.clone(), online: true, error_count: 0, last_error: None, last_poll_at: None,
        }).collect() }
    }
}
```

Add tests inline:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_start_stop() {
        let r = Runtime::new();
        r.set_devices(vec![Device { id: "d".into(), enabled: true, ..Default::default() }], vec![]).await;
        r.start(None).await.unwrap();
        assert!(r.is_running().await);
        r.stop().await;
        assert!(!r.is_running().await);
    }

    #[tokio::test]
    async fn test_event_delivery() {
        let r = Runtime::new();
        let mut rx = r.subscribe();
        r.set_devices(vec![Device { id: "ed".into(), enabled: true, ..Default::default() }],
            vec![Tag { id: "ed/t".into(), device_id: "ed".into(), enabled: true, ..Default::default() }]).await;
        r.start(None).await.unwrap();
        let timeout = tokio::time::sleep(std::time::Duration::from_secs(3));
        tokio::pin!(timeout);
        let mut ok = false;
        loop {
            tokio::select! {
                ev = rx.recv() => { if matches!(ev, Ok(Event::PollSucceeded(_, _))) { ok = true; break; } }
                _ = &mut timeout => break,
            }
        }
        assert!(ok, "expected PollSucceeded within 3s");
        r.stop().await;
    }

    #[tokio::test]
    async fn test_filtered_start() {
        let r = Runtime::new();
        r.set_devices(vec![
            Device { id: "a".into(), enabled: true, ..Default::default() },
            Device { id: "b".into(), enabled: true, ..Default::default() },
        ], vec![]).await;
        r.start(Some(vec!["a".into()])).await.unwrap();
        assert!(r.runners.lock().await.contains_key("a"));
        assert!(!r.runners.lock().await.contains_key("b"));
        r.stop().await;
    }
}
```

- [ ] **Step 3: Update lib.rs**: `pub mod fake; pub mod runner; pub mod bus; pub mod runtime;`

- [ ] **Step 4: Run tests**: `cargo test -p hc-runtime` (expect 7 passed)

- [ ] **Step 5: Commit**: `git add -A && git commit -m "feat(runtime): add Runtime orchestrator, EventBus with backpressure tests"`
