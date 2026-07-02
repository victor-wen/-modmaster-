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
# Modbus TCP/RTU 上位机 SP1 (MVP) — 实现计划 Part 2 (Tasks 7-12 + CI)

### Task 7: hc-ipc — Tauri Commands + Throttle + Event Handlers (KEY)

**Files:**
- Create: `crates/hc-ipc/Cargo.toml`
- Create: `crates/hc-ipc/src/lib.rs`
- Create: `crates/hc-ipc/src/state.rs`
- Create: `crates/hc-ipc/src/commands.rs`
- Create: `crates/hc-ipc/src/throttle.rs`
- Create: `crates/hc-ipc/src/handlers.rs`

**Interfaces:**
- Consumes: `hc_core::model::*`, `hc_core::event::Event`, `hc_core::error::IpcError`,
  `hc_runtime::runtime::Runtime`, `hc_storage::history::HistoryDb`, `hc_storage::project::*`
- Produces: Tauri commands (new_project, open_project, save_project, list_devices, start_runtime, stop_runtime, runtime_status, write_tag, query_trend),
  event handlers (tag-update, device-state, log emissions),
  UpdateThrottle

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "hc-ipc"
version = "0.1.0"
edition = "2021"

[dependencies]
hc-core = { path = "../hc-core" }
hc-runtime = { path = "../hc-runtime" }
hc-storage = { path = "../hc-storage" }
tauri = { version = "2", features = ["protocol-asset"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["full"] }
log = "0.4"
thiserror = "1"
```

- [ ] **Step 2: Write state.rs**

```rust
use hc_runtime::runtime::Runtime;
use hc_storage::history::HistoryDb;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    pub runtime: Runtime,
    pub history: Option<HistoryDb>,
    pub tauri_app: Option<tauri::AppHandle>,
}

impl AppState {
    pub fn new() -> Self {
        AppState { runtime: Runtime::new(), history: None, tauri_app: None }
    }
}

pub type SharedState = Arc<Mutex<AppState>>;
```

- [ ] **Step 3: Write throttle.rs**

```rust
use hc_core::model::TagUpdate;
use std::collections::HashMap;
use std::time::Instant;

const BATCH_MS: u64 = 100;
const CHANNEL_MAX: usize = 64;

pub struct UpdateThrottle {
    pending: HashMap<String, TagUpdate>,
    last_emit: Instant,
    last_values: HashMap<String, (serde_json::Value, String)>,
    dropped: u64,
}

impl UpdateThrottle {
    pub fn new() -> Self {
        UpdateThrottle { pending: HashMap::new(), last_emit: Instant::now(), last_values: HashMap::new(), dropped: 0 }
    }

    pub fn push(&mut self, u: TagUpdate) {
        self.pending.insert(u.tag_id.clone(), u);
    }

    pub fn tick(&mut self) -> Option<Vec<TagUpdate>> {
        if self.last_emit.elapsed().as_millis() as u64 < BATCH_MS || self.pending.is_empty() {
            return None;
        }
        if self.pending.len() > CHANNEL_MAX {
            self.dropped += (self.pending.len() - CHANNEL_MAX) as u64;
            let keep: HashMap<_, _> = self.pending.drain().skip(self.pending.len().saturating_sub(CHANNEL_MAX)).collect();
            self.pending = keep;
        }
        let mut batch: Vec<TagUpdate> = self.pending.drain().map(|(_, v)| v).collect();
        batch.retain(|u| {
            let key = u.tag_id.clone();
            let cur = (serde_json::to_value(&u.value).unwrap_or_default(), u.quality.to_string());
            let prev = self.last_values.get(&key);
            if Some(&cur) == prev { return false; }
            self.last_values.insert(key, cur);
            true
        });
        self.last_emit = Instant::now();
        if batch.is_empty() { None } else { Some(batch) }
    }

    pub fn dropped_count(&self) -> u64 { self.dropped }
}
```

- [ ] **Step 4: Write handlers.rs**

```rust
use crate::state::SharedState;
use crate::throttle::UpdateThrottle;
use hc_core::event::Event;
use hc_core::model::*;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn spawn_event_handler(state: SharedState) {
    let throttle = Arc::new(Mutex::new(UpdateThrottle::new()));
    let t1 = throttle.clone();
    let s1 = state.clone();

    // Emit loop
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Some(batch) = t1.lock().await.tick() {
                if let Some(app) = s1.lock().await.tauri_app.as_ref() {
                    let _ = app.emit("tag-update", &batch);
                }
            }
        }
    });

    // Event listener
    let t2 = throttle.clone();
    let s2 = state.clone();
    tokio::spawn(async move {
        let rx = { s2.lock().await.runtime.subscribe() };
        let mut rx = rx;
        loop {
            match rx.recv().await {
                Ok(Event::PollSucceeded(_, samples)) => {
                    let mut t = t2.lock().await;
                    for s in &samples { t.push(TagUpdate::from(s)); }
                    let dropped = t.dropped_count();
                    if dropped > 0 {
                        if let Some(app) = s2.lock().await.tauri_app.as_ref() {
                            let _ = app.emit("log", &LogEntry {
                                ts: chrono::Utc::now(), level: "warn".into(),
                                message: format!("IPC throttle dropped {dropped} updates"),
                            });
                        }
                    }
                }
                Ok(Event::PollFailed(id, err)) => {
                    if let Some(app) = s2.lock().await.tauri_app.as_ref() {
                        let _ = app.emit("device-state", &DeviceState {
                            device_id: id, online: false, error_count: 1,
                            last_error: Some(err), last_poll_at: None,
                        });
                    }
                }
                Ok(Event::ConnectionStateChanged(id, online)) => {
                    if let Some(app) = s2.lock().await.tauri_app.as_ref() {
                        let _ = app.emit("device-state", &DeviceState {
                            device_id: id, online, error_count: 0,
                            last_error: None, last_poll_at: None,
                        });
                    }
                }
                Ok(Event::Log(entry)) => {
                    if let Some(app) = s2.lock().await.tauri_app.as_ref() {
                        let _ = app.emit("log", &entry);
                    }
                }
                Ok(Event::DeviceStatus(ds)) => {
                    if let Some(app) = s2.lock().await.tauri_app.as_ref() {
                        let _ = app.emit("device-state", &ds);
                    }
                }
                Err(_) => break,
            }
        }
    });
}
```

- [ ] **Step 5: Write commands.rs**

```rust
use crate::state::SharedState;
use hc_core::model::*;
use hc_core::error::IpcError;
use hc_storage::project;

#[tauri::command]
pub async fn new_project(state: tauri::State<'_, SharedState>, name: String) -> Result<Project, String> {
    let mut project = Project::default();
    project.name = name.clone();
    let base = std::env::temp_dir().join("hc_projects").join(project::project_dir_name(&name));
    project::create_project(&base, &project).map_err(|e| e.to_string())?;
    project.path = Some(base);
    Ok(project)
}

#[tauri::command]
pub async fn open_project(state: tauri::State<'_, SharedState>, path: String) -> Result<Project, String> {
    project::load_project(std::path::Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_project(state: tauri::State<'_, SharedState>, project: Project) -> Result<(), String> {
    if let Some(ref p) = project.path {
        project::save_project_file(p, &project).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_devices(state: tauri::State<'_, SharedState>) -> Result<Vec<Device>, String> {
    Ok(Vec::new())
}

#[tauri::command]
pub async fn start_runtime(state: tauri::State<'_, SharedState>) -> Result<(), String> {
    state.lock().await.runtime.start(None).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_runtime(state: tauri::State<'_, SharedState>) -> Result<(), String> {
    state.lock().await.runtime.stop().await;
    Ok(())
}

#[tauri::command]
pub async fn runtime_status(state: tauri::State<'_, SharedState>) -> Result<RuntimeStatus, String> {
    Ok(state.lock().await.runtime.status().await)
}

#[tauri::command]
pub async fn write_tag(state: tauri::State<'_, SharedState>, tag_id: String, value: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn query_trend(state: tauri::State<'_, SharedState>, tag_ids: Vec<String>, from_ms: i64, to_ms: i64, max_points: u32) -> Result<Vec<Sample>, String> {
    let s = state.lock().await;
    if let Some(ref db) = s.history {
        db.query_trend(&tag_ids, from_ms, to_ms, max_points).map_err(|e| e.to_string())
    } else {
        Ok(Vec::new())
    }
}
```

- [ ] **Step 6: Write lib.rs**

```rust
pub mod state;
pub mod commands;
pub mod handlers;
pub mod throttle;
```

- [ ] **Step 7: Write throttle tests** (inline)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use hc_core::model::*;

    fn make(tag_id: &str, v: f32) -> TagUpdate {
        TagUpdate { tag_id: tag_id.into(), ts: chrono::Utc::now(), value: Value::F32(v), unit: "C".into(), quality: Quality::Good }
    }

    #[test] fn test_batch() {
        let mut t = UpdateThrottle::new();
        t.push(make("d/t", 25.0)); t.push(make("d/t2", 30.0));
        std::thread::sleep(std::time::Duration::from_millis(150));
        let b = t.tick();
        assert!(b.is_some());
        assert_eq!(b.unwrap().len(), 2);
    }

    #[test] fn test_dedupe() {
        let mut t = UpdateThrottle::new();
        t.push(make("d/t", 25.0));
        std::thread::sleep(std::time::Duration::from_millis(150));
        t.tick();
        t.push(make("d/t", 25.0));
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(t.tick().is_none());
        t.push(make("d/t", 26.0));
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(t.tick().is_some());
    }

    #[test] fn test_overlimit() {
        let mut t = UpdateThrottle::new();
        for i in 0..100 { t.push(make(&format!("d/t{i}"), i as f32)); }
        std::thread::sleep(std::time::Duration::from_millis(150));
        let b = t.tick();
        assert!(b.unwrap().len() <= CHANNEL_MAX);
        assert!(t.dropped_count() > 0);
    }
}
```

- [ ] **Step 8: Run tests**: `cargo test -p hc-ipc` (expect 3 passed)

- [ ] **Step 9: Commit**: `git add -A && git commit -m "feat(ipc): add Tauri command stubs, throttled event handlers"`

---

### Task 8: hc-app — Tauri Shell + Entrypoint + Assembly

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/capabilities/default.json`
- Create: `src-tauri/src/main.rs`
- Create: `.gitignore`
- Create (placeholder): `src-tauri/icons/` (empty directory)

- [ ] **Step 1: Create src-tauri/Cargo.toml**

```toml
[package]
name = "host-computer"
version = "0.1.0"
edition = "2021"

[dependencies]
hc-core = { path = "../crates/hc-core" }
hc-runtime = { path = "../crates/hc-runtime" }
hc-ipc = { path = "../crates/hc-ipc" }
hc-storage = { path = "../crates/hc-storage" }
tauri = { version = "2", features = [] }
tauri-build = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
log = "0.4"
```

Note: Do NOT add `[workspace]` here. This crate is OUTSIDE the workspace (Tauri convention). The src-tauri/Cargo.toml is separate from the workspace root.

- [ ] **Step 2: Create tauri.conf.json**

```json
{
  "$schema": "https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-cli/schema.json",
  "productName": "HostComputer",
  "version": "0.1.0",
  "identifier": "com.host-computer.app",
  "build": {
    "frontendDist": "../frontend/dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "windows": [
      {
        "title": "上位机 - Host Computer",
        "width": 1280,
        "height": 800,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]
  }
}
```

- [ ] **Step 3: Create build.rs**

```rust
fn main() { tauri_build::build() }
```

- [ ] **Step 4: Create capabilities/default.json**

```json
{
  "identifier": "default",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:event:default",
    "core:event:allow-emit",
    "core:event:allow-listen"
  ]
}
```

- [ ] **Step 5: Create src/main.rs**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use hc_ipc::state::{AppState, SharedState};
use hc_ipc::commands;
use hc_ipc::handlers;
use std::sync::Arc;
use tokio::sync::Mutex;

fn main() {
    let state: SharedState = Arc::new(Mutex::new(AppState::new()));

    tauri::Builder::default()
        .manage(state.clone())
        .setup(move |app| {
            let handle = app.handle().clone();
            let s = state.clone();
            tokio::spawn(async move {
                s.lock().await.tauri_app = Some(handle);
                handlers::spawn_event_handler(s.clone()).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::new_project,
            commands::open_project,
            commands::save_project,
            commands::list_devices,
            commands::start_runtime,
            commands::stop_runtime,
            commands::runtime_status,
            commands::write_tag,
            commands::query_trend,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: Create .gitignore**

```
target/
frontend/dist/
frontend/node_modules/
*.db
*.db-wal
*.db-shm
.DS_Store
```

- [ ] **Step 7: Create icons placeholder**

```bash
mkdir -p src-tauri/icons
# Create minimal 1x1 PNG placeholders for development
```

- [ ] **Step 8: Verify compilation**

Run: `cargo check -p host-computer`
(Note: frontend directory must exist with at least index.html for tauri::generate_context! to work.
Create empty `frontend/dist/index.html` temporarily if needed:
```bash
mkdir -p frontend/dist && echo '<html><body></body></html>' > frontend/dist/index.html
```
)

Expected: Compilation succeeds

- [ ] **Step 9: Commit**: `git add -A && git commit -m "feat(app): add Tauri shell with command registration and event handlers"`

---

### Task 9: Frontend Scaffold — IPC Client + Store + Bindings

**Files:**
- Create: `frontend/package.json`
- Create: `frontend/tsconfig.json`
- Create: `frontend/tsconfig.node.json`
- Create: `frontend/vite.config.ts`
- Create: `frontend/index.html`
- Create: `frontend/src/main.tsx`
- Create: `frontend/src/App.tsx`
- Create: `frontend/src/vite-env.d.ts`
- Create: `frontend/src/index.css`
- Create: `frontend/src/lib/utils.ts`
- Create: `frontend/src/ipc/client.ts`
- Create: `frontend/src/ipc/bindings.ts`
- Create: `frontend/src/store/index.ts`
- Create: `frontend/src/hooks/useTagSubscription.ts`
- Create: `frontend/src/components/Layout.tsx`
- Create: `frontend/src/components/RuntimeStatusBar.tsx`
- Create: `frontend/src/components/LogWindow.tsx`
- Create: `frontend/tailwind.config.js`
- Create: `frontend/postcss.config.js`

- [ ] **Step 1: Create package.json**

```json
{
  "name": "host-computer-frontend",
  "private": true, "version": "0.1.0", "type": "module",
  "scripts": {
    "dev": "vite", "build": "tsc && vite build",
    "preview": "vite preview", "typecheck": "tsc --noEmit",
    "lint": "eslint . --ext ts,tsx --max-warnings 0",
    "test": "vitest run"
  },
  "dependencies": {
    "react": "^18.3.1", "react-dom": "^18.3.1",
    "react-router-dom": "^6.26.0", "zustand": "^4.5.4",
    "react-grid-layout": "^1.4.4", "lightweight-charts": "^4.1.1",
    "@tauri-apps/api": "^2.0.1",
    "class-variance-authority": "^0.7.0", "clsx": "^2.1.1",
    "tailwind-merge": "^2.4.0", "lucide-react": "^0.428.0"
  },
  "devDependencies": {
    "@types/react": "^18.3.3", "@types/react-dom": "^18.3.0",
    "@types/react-grid-layout": "^1.3.5",
    "@tauri-apps/cli": "^2.0.1", "@vitejs/plugin-react": "^4.3.1",
    "typescript": "^5.5.3", "vite": "^5.4.0", "vitest": "^2.0.0",
    "@testing-library/react": "^16.0.0", "jsdom": "^24.0.0",
    "tailwindcss": "^3.4.7", "postcss": "^8.4.40", "autoprefixer": "^10.4.19"
  }
}
```

- [ ] **Step 2: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2020", "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext", "skipLibCheck": true,
    "moduleResolution": "bundler", "allowImportingTsExtensions": true,
    "resolveJsonModule": true, "isolatedModules": true, "noEmit": true,
    "jsx": "react-jsx",
    "strict": true, "noUnusedLocals": true, "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "baseUrl": ".", "paths": { "@/*": ["./src/*"] }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

- [ ] **Step 3: Create vite.config.ts**

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
const host = process.env.TAURI_DEV_HOST;
export default defineConfig(async () => ({
  plugins: [react()],
  resolve: { alias: { "@": path.resolve(__dirname, "./src") } },
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] } },
}));
```

- [ ] **Step 4: Create index.html**

```html
<!doctype html>
<html lang="zh-CN">
  <head><meta charset="UTF-8" /><meta name="viewport" content="width=device-width, initial-scale=1.0" /><title>上位机 - Host Computer</title></head>
  <body><div id="root"></div><script type="module" src="/src/main.tsx"></script></body>
</html>
```

- [ ] **Step 5: Create src/main.tsx**

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode><App /></React.StrictMode>
);
```

- [ ] **Step 6: Create src/App.tsx**

```tsx
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<div className="p-4"><h1 className="text-xl font-bold">上位机</h1><p>Modbus TCP/RTU Host Computer</p></div>} />
      </Routes>
    </BrowserRouter>
  );
}
export default App;
```

- [ ] **Step 7: Create index.css**: `@tailwind base; @tailwind components; @tailwind utilities;`

- [ ] **Step 8: Create tailwind.config.js and postcss.config.js**

tailwind.config.js:
```js
export default { content: ["./index.html", "./src/**/*.{ts,tsx}"], theme: { extend: {} }, plugins: [] }
```
postcss.config.js:
```js
export default { plugins: { tailwindcss: {}, autoprefixer: {} } }
```

- [ ] **Step 9: Create src/ipc/bindings.ts**

```ts
export interface Project { name: string; version: number; runtime: ProjectRuntime; storage: ProjectStorage; }
export interface ProjectRuntime { default_poll_interval_ms: number; }
export interface ProjectStorage { history_sampling_ms: number; trend_max_points: number; }
export interface Device {
  id: string; name: string; enabled: boolean; protocol: string;
  transport: TcpTransport | RtuTransport;
  protocol_params: Record<string, unknown>;
  poll_interval_ms: number; timeout_ms: number;
}
export interface TcpTransport { type: "Tcp"; host: string; port: number; }
export interface RtuTransport { type: "Rtu"; port: string; baud: number; data_bits: number; parity: string; stop_bits: number; }
export interface Tag {
  id: string; device_id: string; name: string; enabled: boolean;
  data_type: string; byte_order: string; scale: number; offset: number;
  unit: string; writable: boolean; protocol_params: Record<string, unknown>;
}
export interface TagUpdate { tag_id: string; ts: string; value: number | boolean; unit: string; quality: string; }
export interface DeviceState { device_id: string; online: boolean; error_count: number; last_error: string | null; last_poll_at: string | null; }
export interface RuntimeStatus { running: boolean; devices: DeviceState[]; }
export interface Sample { tag_id: string; ts: string; value: number | boolean; quality: string; }
export interface LogEntry { ts: string; level: string; message: string; }
```

- [ ] **Step 10: Create src/ipc/client.ts**

```ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Project, Device, Tag, TagUpdate, DeviceState, RuntimeStatus, Sample, LogEntry } from "./bindings";
export interface IpcClient {
  newProject(name: string): Promise<Project>;
  openProject(path: string): Promise<Project>;
  saveProject(p: Project): Promise<void>;
  listDevices(): Promise<Device[]>;
  startRuntime(): Promise<void>;
  stopRuntime(): Promise<void>;
  runtimeStatus(): Promise<RuntimeStatus>;
  writeTag(tagId: string, value: string): Promise<void>;
  queryTrend(tagIds: string[], fromMs: number, toMs: number, maxPoints: number): Promise<Sample[]>;
  onTagUpdate(cb: (u: TagUpdate[]) => void): Promise<UnlistenFn>;
  onDeviceState(cb: (s: DeviceState) => void): Promise<UnlistenFn>;
  onLog(cb: (e: LogEntry) => void): Promise<UnlistenFn>;
}
export function createClient(): IpcClient {
  return {
    newProject: (n) => invoke("new_project", { name: n }),
    openProject: (p) => invoke("open_project", { path: p }),
    saveProject: (p) => invoke("save_project", { project: p }),
    listDevices: () => invoke("list_devices"),
    startRuntime: () => invoke("start_runtime"),
    stopRuntime: () => invoke("stop_runtime"),
    runtimeStatus: () => invoke("runtime_status"),
    writeTag: (id, v) => invoke("write_tag", { tagId: id, value: v }),
    queryTrend: (ids, f, t, m) => invoke("query_trend", { tagIds: ids, fromMs: f, toMs: t, maxPoints: m }),
    onTagUpdate: (cb) => listen<TagUpdate[]>("tag-update", (e) => cb(e.payload)),
    onDeviceState: (cb) => listen<DeviceState>("device-state", (e) => cb(e.payload)),
    onLog: (cb) => listen<LogEntry>("log", (e) => cb(e.payload)),
  };
}
```

- [ ] **Step 11: Create src/store/index.ts**

```ts
import { create } from "zustand";
import type { TagUpdate, DeviceState } from "../ipc/bindings";
interface TagValueMap { [tagId: string]: TagUpdate }
interface RuntimeStore {
  running: boolean; devices: DeviceState[]; tagValues: TagValueMap; logs: string[];
  setRunning: (v: boolean) => void;
  setDevices: (d: DeviceState[]) => void;
  updateTagValues: (u: TagUpdate[]) => void;
  addLog: (msg: string) => void;
}
export const useStore = create<RuntimeStore>((set) => ({
  running: false, devices: [], tagValues: {}, logs: [],
  setRunning: (v) => set({ running: v }),
  setDevices: (d) => set({ devices: d }),
  updateTagValues: (u) => set((s) => {
    const next = { ...s.tagValues };
    for (const v of u) next[v.tag_id] = v;
    return { tagValues: next };
  }),
  addLog: (msg) => set((s) => ({ logs: [...s.logs.slice(-499), `[${new Date().toLocaleTimeString()}] ${msg}`] })),
}));
```

- [ ] **Step 12: Create hooks/useTagSubscription.ts**

```ts
import { useEffect, useRef } from "react";
import { createClient, type IpcClient } from "../ipc/client";
import { useStore } from "../store";
import type { TagUpdate, UnlistenFn } from "../ipc/bindings";
const client: IpcClient = createClient();
export function useTagSubscription() {
  const rafRef = useRef<number>(0);
  const pendingRef = useRef<TagUpdate[]>([]);
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    (async () => {
      unlisten = await client.onTagUpdate((updates) => {
        pendingRef.current.push(...updates);
        if (!rafRef.current) {
          rafRef.current = requestAnimationFrame(() => {
            const batch = pendingRef.current.splice(0);
            useStore.getState().updateTagValues(batch);
            rafRef.current = 0;
          });
        }
      });
    })();
    return () => { unlisten?.(); if (rafRef.current) cancelAnimationFrame(rafRef.current); };
  }, []);
}
```

- [ ] **Step 13: Create Layout and basic components**

Create `src/components/Layout.tsx`, `RuntimeStatusBar.tsx`, `LogWindow.tsx` (see spec design in Task 11 details). For now, simple stub:

```tsx
// Layout.tsx
import { Outlet, NavLink } from "react-router-dom";
export function Layout({ children }: { children?: React.ReactNode }) {
  return <div className="flex h-screen flex-col">
    <header className="flex items-center bg-slate-800 px-4 py-2 text-white">
      <h1 className="mr-8 text-lg font-bold">上位机</h1>
      <nav className="flex gap-4">
        {["/dashboard","/devices","/trend","/settings"].map(p => (
          <NavLink key={p} to={p} className={({isActive}) => isActive ? "text-blue-300 underline" : "hover:text-blue-200"}>{p.slice(1)}</NavLink>
        ))}
      </nav>
    </header>
    <main className="flex-1 overflow-auto p-4">{children || <Outlet />}</main>
  </div>;
}
```

- [ ] **Step 14: Verify frontend compiles**

```bash
cd frontend && npm install && npx tsc --noEmit
```

Expected: Compiles without errors (some imports may need adjustment)

- [ ] **Step 15: Commit**: `git add -A frontend/ && git commit -m "feat(frontend): add Vite+React scaffold, IPC client, Zustand store"`

---

### Task 10: Frontend — WidgetRegistry + 5 Built-in Widgets (KEY)

**Files:**
- Create: `frontend/src/widgets/types.ts`
- Create: `frontend/src/widgets/registry.ts`
- Create: `frontend/src/widgets/built-in/NumericTable.tsx`
- Create: `frontend/src/widgets/built-in/RealtimeChart.tsx`
- Create: `frontend/src/widgets/built-in/Gauge.tsx`
- Create: `frontend/src/widgets/built-in/StatusLight.tsx`
- Create: `frontend/src/widgets/built-in/BarChart.tsx`
- Create: `frontend/src/app/init.ts`
- Create: `frontend/src/widgets/__tests__/registry.test.ts`

- [ ] **Step 1: Create types.ts**

```ts
import type { ComponentType } from "react";
import type { TagUpdate } from "../ipc/bindings";
export interface WidgetManifest {
  id: string; name: string;
  category: "indicator" | "chart" | "table" | "control";
  configSchema: Record<string, unknown>;
  dataBinding: { minSources: number; maxSources: number; sourceRoleNames: string[] };
  defaultProps: { w: number; h: number };
  runtime: ComponentType<WidgetRuntimeProps>;
  editor?: ComponentType<WidgetEditorProps>;
}
export interface WidgetRuntimeProps {
  instanceId: string; config: Record<string, unknown>;
  bindings: TagBinding[]; values: Record<string, TagUpdate>;
}
export interface WidgetEditorProps {
  config: Record<string, unknown>; bindings: TagBinding[];
  onChange: (c: Record<string, unknown>, b: TagBinding[]) => void;
}
export interface TagBinding { role: number; tag_id: string; }
export interface DashboardWidget {
  id: string; widget_id: string;
  layout: { x: number; y: number; w: number; h: number };
  config: Record<string, unknown>; bindings: TagBinding[];
}
export interface Dashboard { name: string; widgets: DashboardWidget[]; }
```

- [ ] **Step 2: Create registry.ts**

```ts
import type { WidgetManifest } from "./types";
class WidgetRegistry {
  private manifests = new Map<string, WidgetManifest>();
  register(m: WidgetManifest) {
    if (this.manifests.has(m.id)) console.warn(`Widget ${m.id} overwritten`);
    this.manifests.set(m.id, m);
  }
  get(id: string) { return this.manifests.get(id); }
  list(category?: string) {
    const all = Array.from(this.manifests.values());
    return category ? all.filter(m => m.category === category) : all;
  }
  unregister(id: string) { this.manifests.delete(id); }
}
export const registry = new WidgetRegistry();
```

- [ ] **Step 3-7: Create 5 built-in widgets** (exact code in spec design — see Task 10 details in design doc)

For brevity, implement as per design spec. Each widget:
- NumericTable: `<table>` with tag_id, value, unit columns
- RealtimeChart: lightweight-charts canvas, LineSeries per binding
- Gauge: SVG circle with percentage arc + center label
- StatusLight: colored circle (green/gray) + ON/OFF label
- BarChart: dynamic bars per binding, proportional height

- [ ] **Step 8: Create app/init.ts** — registers all 5 widgets with registry

```ts
import { registry } from "../widgets/registry";
import { NumericTable } from "../widgets/built-in/NumericTable";
// ... import other widgets
export function initializeWidgets() {
  registry.register({ id: "hcs.builtin.numeric-table", name: "数值表", category: "table", configSchema: { type: "object", properties: { title: { type: "string" } } }, dataBinding: { minSources: 1, maxSources: 20, sourceRoleNames: ["值"] }, defaultProps: { w: 4, h: 3 }, runtime: NumericTable });
  // ... register other 4 widgets
}
```

- [ ] **Step 9: Write registration test**

```ts
import { describe, it, expect } from "vitest";
import { registry } from "../registry";
describe("WidgetRegistry", () => {
  it("registers and lists", () => {
    registry.register({ id: "test.w", name: "T", category: "indicator", configSchema: {}, dataBinding: { minSources: 1, maxSources: 1, sourceRoleNames: ["v"] }, defaultProps: { w: 2, h: 2 }, runtime: () => null });
    expect(registry.get("test.w")).toBeDefined();
    registry.unregister("test.w");
    expect(registry.get("test.w")).toBeUndefined();
  });
});
```

- [ ] **Step 10: Run tests**: `cd frontend && npx vitest run` (expect 1 passed)

- [ ] **Step 11: Commit**: `git add -A frontend/src/widgets/ frontend/src/app/ && git commit -m "feat(widgets): add WidgetRegistry and 5 built-in widgets"`

---

### Task 11: Frontend Pages — Dashboard + Trend + Device Config + Log

**Files:**
- Create: `frontend/src/pages/DashboardPage.tsx`
- Create: `frontend/src/pages/DevicePage.tsx`
- Create: `frontend/src/pages/TrendPage.tsx`
- Create: `frontend/src/pages/SettingsPage.tsx`
- Create: `frontend/src/dashboard/DashboardRuntime.tsx`
- Create: `frontend/src/dashboard/LayoutEngine.tsx`
- Create: `frontend/src/components/DeviceConfigPanel.tsx`
- Create: `frontend/src/components/TagConfigPanel.tsx`
- Create: `frontend/src/components/ProjectManager.tsx`
- Modify: `frontend/src/App.tsx` (add routes)

- [ ] **Step 1: Create LayoutEngine.tsx** (wrapper around react-grid-layout)
- [ ] **Step 2: Create DashboardRuntime.tsx** (renders widgets from registry with values)
- [ ] **Step 3: Create DashboardPage.tsx** (editing mode toggle, add widget button)
- [ ] **Step 4: Create TrendPage.tsx** (query form + lightweight-charts canvas)
- [ ] **Step 5: Create DevicePage.tsx** (CRUD for devices and tags — simple forms)
- [ ] **Step 6: Create SettingsPage.tsx** (project open/save buttons)
- [ ] **Step 7: Update App.tsx** routes: `/` → Dashboard, `/devices`, `/trend`, `/settings`
- [ ] **Step 8: Verify frontend typecheck**: `cd frontend && npx tsc --noEmit`
- [ ] **Step 9: Commit**: `git add -A frontend/src/pages/ frontend/src/dashboard/ frontend/src/components/ && git commit -m "feat(pages): add Dashboard, Device, Trend, Settings pages"`

---

### Task 12: E2E Acceptance + CI Pipeline

**Files:**
- Create: `scripts/simulator.py`
- Create: `.github/workflows/ci.yml`
- Modify: `Cargo.toml` (add workspace exclude for src-tauri if needed)

- [ ] **Step 1: Create simulator.py**

```python
#!/usr/bin/env python3
"""Simple Modbus TCP simulator for acceptance testing."""
import struct, asyncio
from pymodbus.server import StartAsyncTcpServer
from pymodbus.datastore import ModbusSlaveContext, ModbusServerContext

store = ModbusSlaveContext(zero_mode=True)
# Holding 100-101: float 25.0 (0x41C80000)
store.setValues(3, 100, [0x41C8, 0x0000])
# Coil 200: True
store.setValues(1, 200, [True])

async def main():
    context = ModbusServerContext(slaves=store, single=True)
    await StartAsyncTcpServer(context, address=("127.0.0.1", 502))

if __name__ == "__main__":
    asyncio.run(main())
```

- [ ] **Step 2: Create CI workflow**

`.github/workflows/ci.yml`:

```yaml
name: CI
on: [push, pull_request]
jobs:
  rust:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo fmt --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test --workspace
  frontend:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 20 }
      - run: cd frontend && npm ci
      - run: cd frontend && npm run typecheck
      - run: cd frontend && npm run lint
      - run: cd frontend && npm run test
  build:
    needs: [rust, frontend]
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/setup-node@v4
        with: { node-version: 20 }
      - run: cd frontend && npm ci && npm run build
      - run: cargo tauri build
```

- [ ] **Step 3: Acceptance test (manual)**

Per spec §8 acceptance script:
1. Start simulator: `python scripts/simulator.py`
2. Build Tauri app: `cargo tauri build`
3. Run acceptance steps:
   - Launch app → create project → add TCP device
   - Add two tags (f32 holding at 100, bool coil at 200)
   - Build dashboard with gauge + status-light
   - Start runtime → confirm real-time updates within 1s
   - Write tag → confirm simulator reflects write
   - Query trend → confirm curve renders
   - Kill simulator → confirm connection error state + auto-reconnect

- [ ] **Step 4: Verify CI config**: Ensure Rust workspace exclude src-tauri (it's not a workspace member but a Tauri binary). If needed, add `exclude = ["src-tauri"]` to root Cargo.toml.

- [ ] **Step 5: Commit**: `git add -A .github/ scripts/ && git commit -m "ci: add GitHub Actions workflow and acceptance simulator"`
