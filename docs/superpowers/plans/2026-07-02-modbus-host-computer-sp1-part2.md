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
