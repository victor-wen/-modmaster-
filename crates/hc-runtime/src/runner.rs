use hc_core::model::*;
use hc_core::source::{Source, PollRequest};
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
