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
    hooks: Vec<Arc<dyn Hook>>,
    running: Arc<Mutex<bool>>,
}

impl Runtime {
    pub fn new() -> Self {
        Runtime {
            bus: EventBus::new(),
            runners: Arc::new(Mutex::new(HashMap::new())),
            devices: Arc::new(Mutex::new(HashMap::new())),
            tags: Arc::new(Mutex::new(HashMap::new())),
            hooks: vec![Arc::new(NoOpHook)],
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn event_bus(&self) -> &EventBus { &self.bus }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> { self.bus.subscribe() }
    pub fn add_hook(&mut self, hook: Box<dyn Hook>) { self.hooks.push(Arc::from(hook)); }

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
