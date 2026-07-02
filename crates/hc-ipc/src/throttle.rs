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

impl Default for UpdateThrottle {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateThrottle {
    pub fn new() -> Self {
        UpdateThrottle {
            pending: HashMap::new(),
            last_emit: Instant::now(),
            last_values: HashMap::new(),
            dropped: 0,
        }
    }

    pub fn push(&mut self, u: TagUpdate) {
        self.pending.insert(u.tag_id.clone(), u);
    }

    pub fn tick(&mut self) -> Option<Vec<TagUpdate>> {
        if (self.last_emit.elapsed().as_millis() as u64) < BATCH_MS || self.pending.is_empty() {
            return None;
        }
        if self.last_values.len() > 8192 {
            self.last_values.clear();
        }
        let n = self.pending.len();
        if n > CHANNEL_MAX {
            self.dropped += (n - CHANNEL_MAX) as u64;
            let keep: HashMap<_, _> = self
                .pending
                .drain()
                .skip(n.saturating_sub(CHANNEL_MAX))
                .collect();
            self.pending = keep;
        }
        let mut batch: Vec<TagUpdate> = self.pending.drain().map(|(_, v)| v).collect();
        batch.retain(|u| {
            let key = u.tag_id.clone();
            let cur = (
                serde_json::to_value(&u.value).unwrap_or_default(),
                u.quality.to_string(),
            );
            let prev = self.last_values.get(&key);
            if Some(&cur) == prev {
                return false;
            }
            self.last_values.insert(key, cur);
            true
        });
        self.last_emit = Instant::now();
        if batch.is_empty() {
            None
        } else {
            Some(batch)
        }
    }

    pub fn dropped_count(&mut self) -> u64 {
        let count = self.dropped;
        self.dropped = 0;
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hc_core::model::*;

    fn make(tag_id: &str, v: f32) -> TagUpdate {
        TagUpdate {
            tag_id: tag_id.into(),
            ts: chrono::Utc::now(),
            value: Value::F32(v),
            unit: "C".into(),
            quality: Quality::Good,
        }
    }

    #[test]
    fn test_batch() {
        let mut t = UpdateThrottle::new();
        t.push(make("d/t", 25.0));
        t.push(make("d/t2", 30.0));
        std::thread::sleep(std::time::Duration::from_millis(150));
        let b = t.tick();
        assert!(b.is_some());
        assert_eq!(b.unwrap().len(), 2);
    }

    #[test]
    fn test_dedupe() {
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

    #[test]
    fn test_overlimit() {
        let mut t = UpdateThrottle::new();
        for i in 0..100 {
            t.push(make(&format!("d/t{i}"), i as f32));
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        let b = t.tick();
        assert!(b.unwrap().len() <= CHANNEL_MAX);
        assert!(t.dropped_count() > 0);
    }
}
