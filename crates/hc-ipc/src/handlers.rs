use crate::state::SharedState;
use crate::throttle::UpdateThrottle;
use hc_core::event::Event;
use hc_core::model::*;
use std::sync::Arc;
use tauri::Emitter;
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
                    for s in &samples {
                        t.push(TagUpdate::from(s));
                    }
                    let dropped = t.dropped_count();
                    drop(t);
                    if dropped > 0 {
                        log::warn!("IPC throttle dropped {dropped} updates");
                        if let Some(app) = s2.lock().await.tauri_app.as_ref() {
                            let _ = app.emit(
                                "log",
                                &LogEntry {
                                    ts: chrono::Utc::now(),
                                    level: "warn".into(),
                                    message: format!("IPC throttle dropped {dropped} updates"),
                                },
                            );
                        }
                    }
                }
                Ok(Event::PollFailed(id, err)) => {
                    if let Some(app) = s2.lock().await.tauri_app.as_ref() {
                        let _ = app.emit(
                            "device-state",
                            &DeviceState {
                                device_id: id,
                                online: false,
                                error_count: 1,
                                last_error: Some(err),
                                last_poll_at: None,
                            },
                        );
                    }
                }
                Ok(Event::ConnectionStateChanged(id, online)) => {
                    if let Some(app) = s2.lock().await.tauri_app.as_ref() {
                        let _ = app.emit(
                            "device-state",
                            &DeviceState {
                                device_id: id,
                                online,
                                error_count: 0,
                                last_error: None,
                                last_poll_at: None,
                            },
                        );
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
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    log::warn!("Event bus lagged by {n} messages");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}
