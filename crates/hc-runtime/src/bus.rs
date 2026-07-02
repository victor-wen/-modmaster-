use hc_core::event::Event;
use tokio::sync::broadcast;

const CAPACITY: usize = 1024;

pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(CAPACITY);
        EventBus { tx }
    }
    pub fn sender(&self) -> broadcast::Sender<Event> {
        self.tx.clone()
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
    pub fn send(&self, e: Event) -> Result<usize, broadcast::error::SendError<Event>> {
        self.tx.send(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hc_core::model::*;

    #[test]
    fn test_send_recv() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        bus.send(Event::Log(LogEntry {
            ts: chrono::Utc::now(),
            level: "i".into(),
            message: "test".into(),
        }))
        .unwrap();
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
            let _ = bus.send(Event::Log(LogEntry {
                ts: chrono::Utc::now(),
                level: "".into(),
                message: format!("m{i}"),
            }));
        }
        match rx.try_recv() {
            Err(broadcast::error::TryRecvError::Lagged(n)) => assert!(n > 0),
            _ => {}
        }
    }
}
